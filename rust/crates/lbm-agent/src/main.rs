//! `lbm-agent`: the resident process of LittleBigMouse v6 — it watches the system,
//! loads the profiles and drives the hook (`docs/v6-architecture-plan.md` on the `v6`
//! branch, phase 3; `docs/v6-agent.md`).
//!
//! ```text
//! lbm-agent [--fake-hook | --hook PATH] [--config-dir DIR] [--data-dir DIR]
//!           [--no-tray] [--ui PATH]
//! lbm-agent --dump-displays
//! ```
//!
//! - `--fake-hook`: drive a hook that hooks nothing (`fake_hook`), on a private
//!   endpoint, instead of the real one — to develop without capturing the mice.
//! - `--hook PATH`: the hook to launch when none answers (default: the one beside this
//!   executable). It is launched detached, so it outlives the agent (D5).
//! - `--config-dir` / `--data-dir`: where the profiles (`options.json`, `layouts/`)
//!   and `Excluded.txt` live, instead of the user's own — a load can write there
//!   (the excluded-list top-up), so anything but a real session should pass both. The
//!   session autostart entry goes under `--config-dir` too (`autostart/`), never into the
//!   user's.
//! - `--no-tray`: no tray icon (a headless run).
//! - `--ui PATH`: the frontend the tray opens. None by default until the UI is a frontend
//!   of the agent (phase 4): today's UI still drives the hook itself.
//! - `--dump-displays`: the display discovery as JSON (the outputs, and the monitor
//!   ids and layout id the model gives them); its C# twin is `DisplayDump` in the C#
//!   test project, and on the same machine both must print the same values.
//!
//! Without `--fake-hook` the agent drives the hook at the usual endpoint (or
//! `LBM_HOOK_ENDPOINT`), launching it when none answers.

use std::path::PathBuf;
use std::process::ExitCode;

#[cfg(not(windows))]
use lbm_agent::autostart::XdgAutostart;
use lbm_agent::discovery::Discovery;
use lbm_agent::fake_hook::FakeHook;
use lbm_agent::gap_guard::GapGuard;
use lbm_agent::hook::HookClient;
use lbm_agent::instance::InstanceLock;
use lbm_agent::reconcile::Timings;
use lbm_agent::runtime::Agent;
use lbm_agent::supervise::HookLauncher;
use lbm_agent::world::{Platform, SystemWorld};
use lbm_store::{lbm_paths, JsonLayoutStore, LayoutPersistence};
use serde_json::Value;

const USAGE: &str = "usage: lbm-agent [--fake-hook | --hook PATH] [--config-dir DIR] [--data-dir DIR]\n                 [--no-tray] [--ui PATH]\n       lbm-agent --dump-displays";

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
    no_tray: bool,
    ui: Option<PathBuf>,
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
            "--no-tray" => options.no_tray = true,
            "--ui" => options.ui = Some(args.next()?.into()),
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
    #[cfg(windows)]
    return lbm_agent::winpipe::hook_pipe().ok();
    #[cfg(not(windows))]
    {
        let runtime = std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from);
        lbm_ipc::endpoint::socket_path(runtime.as_deref(), Some(&lbm_paths::data_dir()))
            .map(|p| p.to_string_lossy().into_owned())
    }
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
        // Every save aligns the session autostart on the options: a run on a scratch
        // configuration keeps its entry there too.
        #[cfg(not(windows))]
        let platform = match &options.config_dir {
            Some(dir) => std::env::current_exe()
                .ok()
                .map(|exe| XdgAutostart::new(dir.join("autostart"), Vec::new(), exe)),
            None => XdgAutostart::for_session(),
        }
        .map_or_else(Platform::default, Platform::with_autostart);
        // No scheduled task beside a scratch configuration: Windows has one Task
        // Scheduler, the user's.
        #[cfg(windows)]
        let platform = match &options.config_dir {
            Some(_) => None,
            None => lbm_agent::schtask::ScheduledTask::for_session(),
        }
        .map_or_else(Platform::default, Platform::with_autostart);
        let store = JsonLayoutStore::new(options.config_dir.unwrap_or_else(lbm_paths::config_dir));
        let persistence = match options.data_dir {
            Some(dir) => LayoutPersistence::with_excluded_list_file(store, platform, move || {
                dir.join("Excluded.txt")
            }),
            None => LayoutPersistence::new(store, platform),
        };
        // Windows virtualizes what the enumeration reads for a process that is not per-
        // monitor DPI aware (the C# UI's manifest makes it so).
        #[cfg(windows)]
        lbm_display::windows::set_process_per_monitor_dpi_aware();
        let mut world = SystemWorld::new(Discovery::detect(), persistence);
        // D2: the first launch imports the 5.x registry into the JSON store.
        #[cfg(windows)]
        {
            world = world.before_first_load(import_registry);
        }
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
        tokio::spawn(lbm_agent::watch::watch_displays(inputs.clone()));
        #[cfg(windows)]
        let _ = lbm_agent::winwatch::spawn(inputs.clone());
        #[cfg(target_os = "linux")]
        let (sleep, sleep_rx) = tokio::sync::mpsc::unbounded_channel();
        #[cfg(target_os = "linux")]
        tokio::spawn(lbm_agent::sleep::watch(sleep));

        let mut agent = Agent::new(world, Timings::default(), hook, inputs);
        #[cfg(target_os = "linux")]
        {
            agent = agent.with_sleep(sleep_rx);
        }

        // The frontends' requests: the socket's, and the tray's.
        let (calls, calls_rx) = tokio::sync::mpsc::unbounded_channel();
        agent = agent.with_api(calls_rx);

        // The frontends' endpoint (the instance lock is held: any socket there is stale).
        #[cfg(unix)]
        let _api = {
            let runtime = std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from);
            let endpoint =
                lbm_ipc::endpoint::agent_socket_path(runtime.as_deref(), Some(&data_dir));
            match endpoint.map(|p| p.to_string_lossy().into_owned()) {
                Some(endpoint) => match lbm_agent::api::listen_into(&endpoint, calls.clone()) {
                    Ok(listener) => {
                        eprintln!("[lbm-agent] frontends: {endpoint}");
                        Some(listener)
                    }
                    Err(error) => {
                        eprintln!("[lbm-agent] no frontend endpoint at {endpoint}: {error}");
                        None
                    }
                },
                None => None,
            }
        };
        #[cfg(windows)]
        let _api = match lbm_agent::winpipe::agent_pipe().and_then(|name| {
            lbm_agent::api::listen_pipe_into(&name, calls.clone()).map(|l| (name, l))
        }) {
            Ok((name, listener)) => {
                eprintln!("[lbm-agent] frontends: {name}");
                Some(listener)
            }
            Err(error) => {
                eprintln!("[lbm-agent] no frontend pipe: {error}");
                None
            }
        };
        #[cfg(any(target_os = "linux", windows))]
        if !options.no_tray {
            tokio::spawn(lbm_agent::tray::run(
                calls.clone(),
                opener(options.ui.clone()),
            ));
        }
        drop(calls);
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

/// What the tray's Open does: launch the frontend, if there is one to launch.
#[cfg(any(target_os = "linux", windows))]
fn opener(ui: Option<PathBuf>) -> std::sync::Arc<dyn Fn() + Send + Sync> {
    std::sync::Arc::new(move || {
        let Some(ui) = &ui else {
            eprintln!("[lbm-agent] no frontend to open (--ui)");
            return;
        };
        // Its own process, reaped when it leaves; it brings its window forward itself when
        // one already runs (its single-instance guard).
        match std::process::Command::new(ui)
            .stdin(std::process::Stdio::null())
            .spawn()
        {
            Ok(mut child) => {
                std::thread::spawn(move || child.wait());
            }
            Err(error) => eprintln!("[lbm-agent] cannot open {}: {error}", ui.display()),
        }
    })
}

/// D2: at the first launch — nothing in the JSON store yet — the 5.x registry is copied
/// into it, and left as it was (going back to 5.x loses nothing).
#[cfg(windows)]
fn import_registry(layout_id: &str, store: &JsonLayoutStore) {
    use lbm_store::registry_import::import_registry;
    use lbm_store::windows_registry::WindowsKey;

    if store.options_path().exists() {
        return;
    }
    let Some(root) = WindowsKey::open_current_user(r"SOFTWARE\Mgth\LittleBigMouse") else {
        return;
    };
    match import_registry(&root, layout_id, store) {
        Ok(report) => {
            eprintln!(
                "[lbm-agent] imported the 5.x registry: {} layouts, {} models",
                report.layouts.len(),
                report.models.len()
            );
            for (name, why) in &report.skipped {
                eprintln!("[lbm-agent] registry layout {name} not imported: {why}");
            }
        }
        Err(error) => eprintln!("[lbm-agent] the registry import is incomplete: {error}"),
    }
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
    #[cfg(windows)]
    let dump = windows_dump();
    #[cfg(not(windows))]
    let dump = linux_dump();

    match dump {
        Ok(dump) => {
            println!(
                "{}",
                serde_json::to_string_pretty(&dump).expect("a JSON value prints")
            );
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("--dump-displays: {error}");
            ExitCode::FAILURE
        }
    }
}

/// `DisplayDump.Linux`: the outputs of the first backend that answers, in the domain
/// oracle's input shape, and the monitors `LinuxLayoutMapping.AddMonitor` makes of them.
#[cfg(not(windows))]
fn linux_dump() -> Result<Value, String> {
    use lbm_display::linux::{display_json, display_signature, drm, Backend};
    use lbm_layout::linux::add_monitor;
    use lbm_layout::model::{Layout, LayoutOptions};
    use serde_json::json;

    let backend = Backend::detect();
    let monitors = backend
        .map(Backend::query)
        .transpose()
        .map_err(|error| error.to_string())?
        .unwrap_or_default();

    let mut layout = Layout::new(LayoutOptions::default());
    for monitor in &monitors {
        add_monitor(&mut layout, monitor);
    }

    Ok(json!({
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
    }))
}

/// `DisplayDump.Windows`: the monitor devices of the Win32 tree in enumeration order,
/// and the layout `WindowsLayoutBuilder.UpdateFrom` builds from them over an empty
/// store — mapped, id computed, placed from the system, anchored on the primary.
///
/// The process is made per-monitor DPI aware first, as the C# UI's manifest makes it
/// (the C# twin sets its thread the same way): Windows virtualizes positions, modes
/// and DPIs for an unaware process.
#[cfg(windows)]
fn windows_dump() -> Result<Value, String> {
    use lbm_display::windows;
    use lbm_layout::model::{Layout, LayoutOptions};
    use serde_json::json;

    windows::set_process_per_monitor_dpi_aware();
    let dpi_awareness = windows::thread_dpi_awareness();
    let tree = windows::discover().map_err(|error| error.to_string())?;

    let mut layout = Layout::new(LayoutOptions::default());
    let loaded: Result<(), std::convert::Infallible> =
        lbm_layout::windows::populate(&mut layout, dpi_awareness, &tree.layout_input(), |_| Ok(()));
    let Ok(()) = loaded;

    Ok(json!({
        "DpiAwareness": format!("{:?}", layout.dpi_awareness),
        "Displays": tree.monitors().map(|m| windows::display_json(&tree, m)).collect::<Vec<_>>(),
        "LayoutId": layout.id,
        "Monitors": layout.monitors().iter().map(|m| monitor_json(&layout, m)).collect::<Vec<_>>(),
        "DisplaySignature": windows::current_display_signature(),
    }))
}

/// A monitor of the layout: its identity, its model's size (#507, #419), its place in
/// mm, and its sources.
#[cfg(windows)]
fn monitor_json(layout: &lbm_layout::model::Layout, m: &lbm_layout::model::Monitor) -> Value {
    use serde_json::json;

    let model = layout.model(&m.model);
    let projection = layout.depth_projection(m);
    json!({
        "Id": m.id,
        "PnpCode": m.model,
        "DeviceId": m.device_id,
        "SerialNumber": m.serial_number,
        "PnpDeviceName": model.and_then(|model| model.pnp_device_name.clone()),
        "Logo": model.and_then(|model| model.logo.clone()),
        "PhysicalWidth": model.map(|model| model.physical_size.width()),
        "PhysicalHeight": model.map(|model| model.physical_size.height()),
        "X": projection.map(|p| p.x),
        "Y": projection.map(|p| p.y),
        "Sources": m.sources.iter().filter_map(|id| layout.source(id)).map(|s| {
            let source = &s.source;
            json!({
                "Id": source.id,
                "DeviceId": s.device_id,
                "SourceNumber": source.source_number,
                "Primary": source.primary,
                "AttachedToDesktop": source.attached_to_desktop,
                "Orientation": source.orientation,
                "InPixel": {
                    "X": source.in_pixel.x,
                    "Y": source.in_pixel.y,
                    "Width": source.in_pixel.width,
                    "Height": source.in_pixel.height,
                },
                "EffectiveDpi": { "X": source.effective_dpi.x, "Y": source.effective_dpi.y },
                "RawDpi": { "X": source.raw_dpi.x, "Y": source.raw_dpi.y },
                "InterfaceName": source.interface_name,
            })
        }).collect::<Vec<_>>(),
    })
}
