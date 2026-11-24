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
//!
//! All implement the same reconcile contract as the Windows pump: watch
//! `shared.want_hook`, install/remove their capture, feed the engine, exit on
//! `want_quit`.

pub mod accel;
pub mod evdev;
pub mod focus;
pub mod keymap;
pub mod portal;
pub mod rescue;
pub mod wake;
pub mod x11;

use std::sync::atomic::Ordering;

use crate::shared::Shared;

/// Nothing to record on Linux (no message pump to post to): the requests below
/// signal [`Shared::wake`] instead.
pub fn register_main_thread(_shared: &Shared) {}

/// The Windows watchdog heals a silently-dropped WH_MOUSE_LL; no Linux backend
/// has that failure mode (capture loss is reported as an event/error).
pub fn spawn_watchdog(_shared: &'static Shared) {}

/// Run the platform event loop until `request_quit`.
pub fn run(shared: &'static Shared) {
    // Process exclusion (#515) is backend-independent: a dedicated thread watches
    // the active window and pauses/resumes whichever backend is routing.
    focus::spawn(shared);

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
    wait_for_quit(shared);
}

/// With no backend, all that is left to do is to leave when asked.
fn wait_for_quit(shared: &Shared) {
    while !shared.want_quit.load(Ordering::SeqCst) {
        shared.wake.wait();
    }
}

/// Ask the loop to install the input capture.
pub fn request_hook(shared: &Shared) {
    shared.want_hook.store(true, Ordering::SeqCst);
    shared.wake.notify();
}

/// Ask the loop to remove the input capture.
pub fn request_unhook(shared: &Shared) {
    shared.want_hook.store(false, Ordering::SeqCst);
    shared.wake.notify();
}

/// Ask the loop to exit.
pub fn request_quit(shared: &Shared) {
    shared.want_quit.store(true, Ordering::SeqCst);
    shared.wake.notify();
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::time::Duration;

    const PATIENCE: Duration = Duration::from_secs(5);

    /// How many times thread `tid` of this process has gone to sleep of its own accord.
    fn voluntary_switches(tid: libc::pid_t) -> u64 {
        std::fs::read_to_string(format!("/proc/self/task/{tid}/status"))
            .unwrap()
            .lines()
            .find_map(|line| line.strip_prefix("voluntary_ctxt_switches:"))
            .unwrap()
            .trim()
            .parse()
            .unwrap()
    }

    /// `backend` stays asleep while nothing is asked of it, and leaves on `request_quit`.
    ///
    /// A thread that wakes on a timer goes back to sleep every time, and the kernel
    /// counts that: this measures the absence of a poll rather than inferring it.
    fn sleeps_until_quit(backend: impl FnOnce(&'static Shared) + Send + 'static) {
        let shared: &'static Shared = Box::leak(Box::new(Shared::new()));
        let (tid_sender, tid) = mpsc::channel();
        let (left, has_left) = mpsc::channel();
        std::thread::spawn(move || {
            // SAFETY: no arguments, cannot fail.
            let _ = tid_sender.send(unsafe { libc::gettid() });
            backend(shared);
            let _ = left.send(());
        });
        let tid = tid.recv_timeout(PATIENCE).unwrap();

        std::thread::sleep(Duration::from_millis(200));
        let before = voluntary_switches(tid);
        std::thread::sleep(Duration::from_millis(400));
        let woken = voluntary_switches(tid) - before;
        assert!(woken <= 1, "woke {woken} times with nothing to do");

        request_quit(shared);
        has_left
            .recv_timeout(PATIENCE)
            .expect("the quit reached it");
    }

    /// `want_hook` is never set here, so nothing is armed and no device is opened.
    #[test]
    fn the_evdev_backend_sleeps_while_the_engine_is_off() {
        sleeps_until_quit(|shared| {
            assert!(evdev::run(shared));
        });
    }

    #[test]
    fn no_backend_at_all_sleeps_too() {
        sleeps_until_quit(wait_for_quit);
    }

    /// A backend blocked in `wait` has only these to bring it back.
    #[test]
    fn every_request_signals_the_backend() {
        for request in [request_hook, request_unhook, request_quit] {
            let shared: &'static Shared = Box::leak(Box::new(Shared::new()));
            request(shared);

            let (returned, has_returned) = mpsc::channel();
            std::thread::spawn(move || {
                shared.wake.wait();
                let _ = returned.send(());
            });
            has_returned
                .recv_timeout(PATIENCE)
                .expect("it was signalled");
        }
    }
}
