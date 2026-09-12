//! Entry point for the Little Big Mouse hook daemon.
//!
//! Platform-neutral: `platform::init()` does the per-OS process setup (Windows:
//! per-monitor DPI awareness, which must precede every other Win32 call), then
//! the IPC server comes up, then `hook::run` blocks on the platform's event
//! loop until a `Quit` command.

use std::sync::atomic::Ordering;

use littlebigmouse_hook::shared::{Shared, SHARED};
use littlebigmouse_hook::{daemon, hook, ipc, platform};

fn main() {
    platform::init();

    let shared: &'static Shared = SHARED.get_or_init(Shared::new);

    // main() is the event-loop thread; record it before the server can accept a
    // command that would signal it.
    hook::register_main_thread(shared);

    // `LBM_HOOK_ENDPOINT` overrides the per-session pipe/socket path, enabling
    // side-by-side testing next to a running daemon (successor of the old
    // LBM_HOOK_PORT override).
    let side_by_side = std::env::var("LBM_HOOK_ENDPOINT");

    // One hook per session. Two of them is the worst state this program can reach:
    // both grab the mice, and the second takes the first one's socket with it — a
    // live socket and a stale one look the same from outside, so the second unlinks
    // and rebinds over a daemon that is still routing.
    //
    // Not when an endpoint was named: that is the deliberate side-by-side instance,
    // and refusing to start would take the way of testing next to a running daemon
    // with it.
    let _instance = if side_by_side.is_ok() {
        None
    } else {
        match lbm_ipc::instance::InstanceLock::acquire_for_session(lbm_ipc::instance::HOOK) {
            Ok(Some(lock)) => Some(lock),
            Ok(None) => {
                eprintln!("[LittleBigMouse.Hook] a hook already runs in this session");
                return;
            }
            // Not fatal: a runtime directory that cannot be written is no reason to
            // leave the mouse unrouted, and it was no guard at all until today.
            Err(error) => {
                eprintln!("[LittleBigMouse.Hook] no instance lock: {error}");
                None
            }
        }
    };

    let (server, endpoint) = match side_by_side {
        Ok(endpoint) => ipc::server::start_with_endpoint(shared, endpoint),
        Err(_) => ipc::server::start(shared),
    }
    .unwrap_or_else(|error| panic!("failed to start per-user local IPC: {error}"));
    let _ = shared.server.set(server);

    eprintln!("[LittleBigMouse.Hook] listening on {endpoint}");

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

    // The way out when the cursor is trapped where no click can reach the UI. Its
    // own thread by design — see hook::windows::rescue_key. Started before the pump
    // so it is already listening if the very first layout confines the cursor.
    hook::spawn_rescue_key(shared, daemon::rescue_fired);

    // Heal silent OS removal of the low-level mouse hook (Windows only; no-op elsewhere).
    hook::spawn_watchdog(shared);

    // Run the platform's hook/event loop on this thread until a `Quit` command.
    hook::run(shared);
}
