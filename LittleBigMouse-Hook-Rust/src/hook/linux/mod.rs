//! Linux input hooking — backend selection and control surface.
//!
//! Runtime selection: the Wayland InputCapture portal backend in a Wayland
//! session (falling back to X11 if the portal is unusable), the X11 (XInput2)
//! backend otherwise. Both implement the same reconcile contract as the Windows
//! pump: watch `shared.want_hook`, install/remove their capture, feed the
//! engine, exit on `want_quit`.

pub mod portal;
pub mod x11;

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
    if portal::available() {
        if portal::run(shared) {
            return;
        }
        eprintln!("[LittleBigMouse.Hook] portal backend unavailable, trying X11");
    }

    if x11::available() {
        x11::run(shared);
        return;
    }

    eprintln!("[LittleBigMouse.Hook] no usable input backend (no Wayland portal, no X11 display)");
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
