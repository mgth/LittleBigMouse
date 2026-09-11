//! `lbm-agent`: the resident process of LittleBigMouse v6 — it watches the system,
//! loads the profiles and drives the hook (`docs/v6-architecture-plan.md` on the `v6`
//! branch, phase 3; `docs/v6-agent.md`).
//!
//! ```text
//! lbm-agent [--fake-hook | --hook PATH] [--config-dir DIR] [--data-dir DIR]
//! lbm-agent --dump-displays
//! ```
//!
//! - `--fake-hook`: drive a hook that hooks nothing (`fake_hook`), on a private
//!   endpoint, instead of the real one — to develop without capturing the mice.
//! - `--hook PATH`: the hook to launch when none answers (default: the one beside this
//!   executable). It is launched detached, so it outlives the agent (D5).
//! - `--config-dir` / `--data-dir`: where the profiles (`options.json`, `layouts/`)
//!   and `Excluded.txt` live, instead of the user's own — a load can write there
//!   (the excluded-list top-up), so anything but a real session should pass both.
//! - `--dump-displays`: the display discovery as JSON (the outputs, and the monitor
//!   ids and layout id the model gives them); its C# twin is `DisplayDump` in the C#
//!   test project, and on the same machine both must print the same values.
//!
//! Without `--fake-hook` the agent drives the hook at the usual endpoint (or
//! `LBM_HOOK_ENDPOINT`), launching it when none answers.

use std::path::PathBuf;
use std::process::ExitCode;

use lbm_agent::fake_hook::FakeHook;
use lbm_agent::gap_guard::GapGuard;
use lbm_agent::hook::HookClient;
use lbm_agent::instance::InstanceLock;
use lbm_agent::reconcile::Timings;
use lbm_agent::runtime::Agent;
use lbm_agent::supervise::HookLauncher;
use lbm_agent::world::{Platform, SystemWorld};
use lbm_display::linux::{display_json, display_signature, drm, Backend};
use lbm_layout::linux::add_monitor;
use lbm_layout::model::{Layout, LayoutOptions};
use lbm_store::{lbm_paths, JsonLayoutStore, LayoutPersistence};
use serde_json::json;

const USAGE: &str = "usage: lbm-agent [--fake-hook | --hook PATH] [--config-dir DIR] [--data-dir DIR]\n       lbm-agent --dump-displays";

#[derive(Default)]
struct Options {
    dump_displays: bool,
    fake_hook: bool,
    hook: Option<PathBuf>,
    /// Hidden: run a fake hook at this endpoint until a client sends Quit (the
    /// supervision tests launch the agent binary this way instead of a real hook).
    serve_fake_hook: Option<String>,
    config_dir: Option<PathBuf>,
    data_dir: Option<PathBuf>,
}

fn options() -> Option<Options> {
    let mut options = Options::default();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--dump-displays" => options.dump_displays = true,
            "--fake-hook" => options.fake_hook = true,
            "--hook" => options.hook = Some(args.next()?.into()),
            "--serve-fake-hook" => options.serve_fake_hook = Some(args.next()?),
            "--config-dir" => options.config_dir = Some(args.next()?.into()),
            "--data-dir" => options.data_dir = Some(args.next()?.into()),
            _ => return None,
        }
    }
    Some(options)
}

fn main() -> ExitCode {
    let Some(options) = options() else {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    };
    if options.dump_displays {
        return dump_displays();
    }
    if let Some(endpoint) = options.serve_fake_hook {
        return serve_fake_hook(endpoint);
    }
    run(options)
}

/// A private endpoint for the fake hook: never the real hook's.
fn fake_endpoint() -> String {
    let name = format!("lbm-agent-fake-hook-{}", std::process::id());
    if cfg!(windows) {
        format!(r"\\.\pipe\{name}")
    } else {
        let dir = std::env::var_os("XDG_RUNTIME_DIR")
            .map(PathBuf::from)
            .filter(|d| !d.as_os_str().is_empty())
            .unwrap_or_else(std::env::temp_dir);
        dir.join(format!("{name}.sock"))
            .to_string_lossy()
            .into_owned()
    }
}

/// Where the real hook listens.
fn hook_endpoint() -> Option<String> {
    if let Some(endpoint) = lbm_ipc::endpoint::from_environment() {
        return Some(endpoint);
    }
    if cfg!(windows) {
        // The per-session pipe name needs the session id: comes with the Windows agent.
        return None;
    }
    let runtime = std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from);
    lbm_ipc::endpoint::socket_path(runtime.as_deref(), Some(&lbm_paths::data_dir()))
        .map(|p| p.to_string_lossy().into_owned())
}

/// `yyyy-mm-dd hh:mm:ss` UTC, for the log header.
fn utc_now() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let (days, rest) = (secs / 86_400, secs % 86_400);
    // Howard Hinnant's civil_from_days.
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02} {:02}:{:02}:{:02}",
        rest / 3600,
        rest % 3600 / 60,
        rest % 60
    )
}

fn run(options: Options) -> ExitCode {
    // One agent per session, decided before anything is touched — the log included:
    // a second launch must not rotate the running agent's log away.
    let _instance = match InstanceLock::acquire_for_session() {
        Ok(Some(lock)) => lock,
        Ok(None) => {
            eprintln!("lbm-agent: an agent already runs in this session");
            return ExitCode::SUCCESS;
        }
        Err(error) => {
            eprintln!("lbm-agent: cannot take the instance lock: {error}");
            return ExitCode::FAILURE;
        }
    };
    let data_dir = options.data_dir.clone().unwrap_or_else(lbm_paths::data_dir);
    let _ = lbm_agent::log::to_file_unless_terminal(&data_dir.join("agent.log"));
    eprintln!(
        "[{} UTC] lbm-agent {} starting (pid {}, {})",
        utc_now(),
        env!("CARGO_PKG_VERSION"),
        std::process::id(),
        std::env::consts::OS
    );

    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("lbm-agent: {error}");
            return ExitCode::FAILURE;
        }
    };
    runtime.block_on(async move {
        let store = JsonLayoutStore::new(options.config_dir.unwrap_or_else(lbm_paths::config_dir));
        let persistence = match options.data_dir {
            Some(dir) => LayoutPersistence::with_excluded_list_file(store, Platform, move || {
                dir.join("Excluded.txt")
            }),
            None => LayoutPersistence::new(store, Platform),
        };
        let mut world = SystemWorld::new(Backend::detect(), persistence);
        // The KWin gaps move the user's outputs: a real session only, never beside a
        // fake hook (which needs no barriers anyway).
        if !options.fake_hook && cfg!(target_os = "linux") {
            world =
                world.with_gap_guard(GapGuard::for_session(data_dir.join("kscreen-restore.json")));
        }

        // Kept alive for the whole run: dropping it closes its endpoint.
        let mut _fake = None;
        let endpoint = if options.fake_hook {
            let endpoint = fake_endpoint();
            match FakeHook::bind(&endpoint) {
                Ok(fake) => _fake = Some(fake),
                Err(error) => {
                    eprintln!("lbm-agent: cannot start the fake hook at {endpoint}: {error}");
                    return ExitCode::FAILURE;
                }
            }
            endpoint
        } else {
            match hook_endpoint() {
                Some(endpoint) => endpoint,
                None => {
                    eprintln!("lbm-agent: no hook endpoint (set LBM_HOOK_ENDPOINT)");
                    return ExitCode::FAILURE;
                }
            }
        };
        eprintln!("[lbm-agent] hook endpoint: {endpoint}");

        let (hook, signals) = HookClient::spawn(endpoint);
        let (inputs, inputs_rx) = tokio::sync::mpsc::unbounded_channel();
        #[cfg(target_os = "linux")]
        tokio::spawn(lbm_agent::watch::poll_displays(inputs.clone()));

        let mut agent = Agent::new(world, Timings::default(), hook, inputs);
        if !options.fake_hook {
            let log = Some(data_dir.join("hook.log"));
            let launcher = match options.hook {
                Some(program) => Some(HookLauncher::new(program, Vec::new(), log)),
                None => HookLauncher::beside_agent(log),
            };
            match launcher {
                Some(launcher) => agent = agent.with_launcher(launcher),
                None => eprintln!("[lbm-agent] no hook beside the agent: waiting for one"),
            }
        }
        agent
            .run(signals, inputs_rx, async {
                let _ = tokio::signal::ctrl_c().await;
            })
            .await;
        ExitCode::SUCCESS
    })
}

/// A fake hook as a process of its own, until a client sends `Quit`.
fn serve_fake_hook(endpoint: String) -> ExitCode {
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(_) => return ExitCode::FAILURE,
    };
    runtime.block_on(async move {
        match FakeHook::bind(&endpoint) {
            Ok(fake) => {
                fake.quit_requested().await;
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("lbm-agent: cannot serve a fake hook at {endpoint}: {error}");
                ExitCode::FAILURE
            }
        }
    })
}

/// The display discovery as JSON, member for member what the C# `DisplayDump` writes.
fn dump_displays() -> ExitCode {
    if cfg!(windows) {
        eprintln!("--dump-displays: the Windows discovery is not ported yet");
        return ExitCode::FAILURE;
    }

    let backend = Backend::detect();
    let monitors = match backend.map(Backend::query).transpose() {
        Ok(monitors) => monitors.unwrap_or_default(),
        Err(error) => {
            eprintln!("--dump-displays: {error}");
            return ExitCode::FAILURE;
        }
    };

    let mut layout = Layout::new(LayoutOptions::default());
    for monitor in &monitors {
        add_monitor(&mut layout, monitor);
    }

    let dump = json!({
        "Backend": backend.map(Backend::name),
        "Displays": monitors.iter().map(display_json).collect::<Vec<_>>(),
        "LayoutId": layout.compute_id(),
        "Monitors": layout.monitors().iter().map(|m| json!({
            "Id": m.id,
            "PnpCode": m.model,
            "DeviceId": m.device_id,
        })).collect::<Vec<_>>(),
        "PlugSignature": drm::plug_signature(),
        "DisplaySignature": display_signature(&monitors),
    });
    println!(
        "{}",
        serde_json::to_string_pretty(&dump).expect("a JSON value prints")
    );
    ExitCode::SUCCESS
}
