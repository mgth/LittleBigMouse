//! Entry point for the Little Big Mouse hook daemon.
//!
//! Mirrors the native `Program.cpp::wWinMain`. Phase 0 scope: bring up the IPC
//! server so the real C# UI can connect and drive daemon state. The DPI-awareness
//! call, parent-process detection (UI vs. standalone mode) and the message pump
//! arrive with the Win32 work in Phase 1+.

use littlebigmouse_hook::ipc;
use littlebigmouse_hook::shared::{Shared, SHARED};

const DAEMON_PORT: u16 = 25196;

fn main() {
    let shared: &'static Shared = SHARED.get_or_init(Shared::new);

    // The C++ daemon hardcodes 25196 while the C# side has a configurable
    // DaemonPort; honoring an override here removes that friction and enables
    // safe side-by-side testing next to a running daemon.
    let port = std::env::var("LBM_HOOK_PORT")
        .ok()
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(DAEMON_PORT);

    let (server, port) = ipc::server::start(shared, port);
    // Publish the server handle so later phases (WndProc / WinEvent callbacks on
    // the pump thread) can push events without threading it through every call.
    let _ = shared.server.set(server);

    eprintln!("[LittleBigMouse.Hook] listening on 127.0.0.1:{port}");

    // No message pump yet (Phase 1). Keep the process alive; a `Quit` command
    // terminates it from an IPC thread.
    loop {
        std::thread::park();
    }
}
