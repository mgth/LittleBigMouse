//! Entry point for the Little Big Mouse hook daemon.
//!
//! Platform-neutral: `platform::init()` does the per-OS process setup (Windows:
//! per-monitor DPI awareness, which must precede every other Win32 call), then
//! the IPC server comes up, then `hook::run` blocks on the platform's event
//! loop until a `Quit` command.

use std::sync::atomic::Ordering;

use littlebigmouse_hook::shared::{Shared, SHARED};
use littlebigmouse_hook::{daemon, hook, ipc, platform};

const DAEMON_PORT: u16 = 25196;

fn main() {
    platform::init();

    let shared: &'static Shared = SHARED.get_or_init(Shared::new);

    // main() is the event-loop thread; record it before the server can accept a
    // command that would signal it.
    hook::register_main_thread(shared);

    // The C++ daemon hardcodes 25196 while the C# side has a configurable
    // DaemonPort; honoring an override here removes that friction and enables
    // safe side-by-side testing next to a running daemon.
    let port = std::env::var("LBM_HOOK_PORT")
        .ok()
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(DAEMON_PORT);

    let (server, port) = ipc::server::start(shared, port);
    let _ = shared.server.set(server);

    eprintln!("[LittleBigMouse.Hook] listening on 127.0.0.1:{port}");

    // C++ Program.cpp: UI mode (wait for socket commands) when launched by the UI
    // (parent path contains "LittleBigMouse"); otherwise standalone — load the
    // last saved layout and start hooking. `LBM_HOOK_UI` forces UI mode for tests.
    let ui_mode = std::env::var_os("LBM_HOOK_UI").is_some()
        || platform::process::parent_process_path()
            .map(|p| p.contains("LittleBigMouse"))
            .unwrap_or(false);

    if !ui_mode {
        if let Some(path) = platform::paths::lbm_data_file("Current.xml") {
            eprintln!(
                "[LittleBigMouse.Hook] standalone mode: loading {}",
                path.display()
            );
            if let Some(path) = path.to_str() {
                daemon::load_from_file(shared, path);
            }
        }
    }

    // Optional debug heartbeat: prints the live mouse-event count so the hook can
    // be observed staying alive (and being called) under the timeout window.
    if std::env::var("LBM_HOOK_DEBUG").is_ok() {
        std::thread::spawn(move || loop {
            std::thread::sleep(std::time::Duration::from_millis(500));
            eprintln!(
                "[dbg] hooked={} mouse_events={} crossings={}",
                shared.hooked.load(Ordering::SeqCst),
                hook::MOUSE_EVENTS.load(Ordering::Relaxed),
                hook::CROSSINGS.load(Ordering::Relaxed),
            );
        });
    }

    // Heal silent OS removal of the low-level mouse hook (Windows only; no-op elsewhere).
    hook::spawn_watchdog(shared);

    // Run the platform's hook/event loop on this thread until a `Quit` command.
    hook::run(shared);
}
