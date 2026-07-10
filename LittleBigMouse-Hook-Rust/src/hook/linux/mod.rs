//! Linux input hooking — backend selection and control surface.
//!
//! Runtime selection, best first:
//!   1. evdev/uinput — grab the physical mice and re-inject a corrected stream.
//!      LBM is the sole router (like the Windows hook): no portal, no capture
//!      notification, no round-trip. Needs read access to /dev/input and write
//!      to /dev/uinput (the `input` group, or a udev rule). Works under Wayland
//!      AND X11.
//!   2. Wayland InputCapture portal — degraded fallback when evdev is not
//!      permitted: the compositor's barrier validator forbids interior-edge
//!      barriers and flags every crossing with a capture notification.
//!   3. X11 (XInput2) — native fallback on an X session without evdev access.
//! All implement the same reconcile contract as the Windows pump: watch
//! `shared.want_hook`, install/remove their capture, feed the engine, exit on
//! `want_quit`.

pub mod evdev;
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
    // Preferred everywhere it is permitted: the sole-router model has none of the
    // portal's limitations. Returns false only if initial setup failed (fall back).
    if evdev::available() {
        if evdev::run(shared) {
            return;
        }
        eprintln!("[LittleBigMouse.Hook] evdev backend unavailable, trying portal/X11");
    } else {
        eprintln!("[LittleBigMouse.Hook] evdev not available (no /dev/uinput access or no mouse); trying portal/X11");
    }

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

    eprintln!("[LittleBigMouse.Hook] no usable input backend (no evdev, no Wayland portal, no X11 display)");
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
