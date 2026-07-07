//! Entry point for the Little Big Mouse hook daemon.
//!
//! Mirrors the native `Program.cpp::wWinMain`: set per-monitor DPI awareness
//! first, bring up the IPC server, then run the hook + message pump on the main
//! thread (the low-level hook must be installed and pumped on the same thread).
//!
//! Phase 1 always starts in "UI mode" (wait for socket commands). Parent-process
//! detection and standalone `Current.xml` loading arrive in Phase 4.

use std::sync::atomic::Ordering;

use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::HiDpi::{
    SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
};

use littlebigmouse_hook::shared::{Shared, SHARED};
use littlebigmouse_hook::{daemon, hook, ipc, platform};

const DAEMON_PORT: u16 = 25196;

fn main() {
    // Must be the very first Win32 call: otherwise every coordinate we read is
    // virtualized and wrong on multi-DPI setups.
    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }

    let shared: &'static Shared = SHARED.get_or_init(Shared::new);

    // main() is the pump thread; record its id before the server can accept a
    // command that would post to it.
    shared
        .pump_tid
        .store(unsafe { GetCurrentThreadId() }, Ordering::SeqCst);

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
            eprintln!("[LittleBigMouse.Hook] standalone mode: loading {}", path.display());
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
                hook::mouse::MOUSE_EVENTS.load(Ordering::Relaxed),
                hook::mouse::CROSSINGS.load(Ordering::Relaxed),
            );
        });
    }

    // Run the hook install/uninstall + message pump loop on this thread. Returns
    // when a `Quit` command posts WM_QUIT.
    let mut hooker = hook::Hooker::new();
    hooker.run(shared);
}
