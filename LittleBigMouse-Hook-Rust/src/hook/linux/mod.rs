//! Linux input hooking — backend selection and control surface.
//!
//! Runtime selection: the Wayland InputCapture portal backend when a Wayland
//! session is detected, the X11 (XInput2) backend otherwise. Both implement the
//! same reconcile loop contract as the Windows pump: watch `shared.want_hook`,
//! install/remove their capture, feed the engine, exit on `want_quit`.

use std::sync::atomic::Ordering;

use crate::shared::Shared;

/// Nothing to record on Linux (no message pump to post to): the backends poll
/// the shared flags.
pub fn register_main_thread(_shared: &Shared) {}

/// The Windows watchdog heals a silently-dropped WH_MOUSE_LL; no Linux backend
/// has that failure mode (capture loss is reported as an event/error).
pub fn spawn_watchdog(_shared: &'static Shared) {}

/// Run the platform event loop until `request_quit`.
pub fn run(shared: &'static Shared) {
    // Backend selection comes with the X11/portal implementations (phase 3.2);
    // until then reconcile the flags so the IPC contract stays observable.
    eprintln!("[LittleBigMouse.Hook] linux: no input backend yet (stub loop)");
    while !shared.want_quit.load(Ordering::SeqCst) {
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}

/// Ask the loop to install the input capture.
pub fn request_hook(shared: &Shared) {
    shared.want_hook.store(true, Ordering::SeqCst);
}

/// Ask the loop to remove the input capture.
pub fn request_unhook(shared: &Shared) {
    shared.want_hook.store(false, Ordering::SeqCst);
}

/// Ask the loop to exit.
pub fn request_quit(shared: &Shared) {
    shared.want_quit.store(true, Ordering::SeqCst);
}
