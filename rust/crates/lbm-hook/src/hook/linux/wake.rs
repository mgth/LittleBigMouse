//! What a Linux backend with nothing to do sleeps on.
//!
//! The Windows pump blocks in `GetMessageW` and is woken by a posted `WM_BREAK_LOOP`.
//! Linux has no message queue to post to, so the control surface (`request_hook`,
//! `request_unhook`, `request_quit`) signals an `eventfd` after changing the flag it
//! changes. A signal given before the wait stays in the counter, so it cannot be lost:
//! the waiter consumes it, then reads the flags.

use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::time::Duration;

/// The pause when no signal can be waited for: no descriptor could be had, or `poll`
/// failed. The caller reads the flags again after it, as it did before there was a
/// signal to wait for.
const WITHOUT_SIGNAL: Duration = Duration::from_millis(50);

pub struct Wake {
    fd: Option<OwnedFd>,
}

impl Wake {
    pub fn new() -> Self {
        // SAFETY: plain syscall; the descriptor is owned right after.
        let raw = unsafe { libc::eventfd(0, libc::EFD_CLOEXEC | libc::EFD_NONBLOCK) };
        if raw < 0 {
            eprintln!(
                "[LittleBigMouse.Hook] no eventfd ({}): an idle backend will poll",
                std::io::Error::last_os_error()
            );
            return Wake { fd: None };
        }
        // SAFETY: `raw` is a fresh descriptor nobody else owns.
        let fd = unsafe { OwnedFd::from_raw_fd(raw) };
        Wake { fd: Some(fd) }
    }

    /// Ends the current or the next [`wait`](Self::wait).
    pub fn notify(&self) {
        let Some(fd) = &self.fd else { return };
        let one = 1u64;
        // A full counter (`EAGAIN`) is a signal already pending, which is all this asks.
        // SAFETY: `one` is valid for the 8 bytes an eventfd takes.
        let _ = unsafe { libc::write(fd.as_raw_fd(), (&raw const one).cast(), 8) };
    }

    /// Blocks until [`notify`](Self::notify). May return early: the caller reads its
    /// flags after every return, and that is what decides.
    pub fn wait(&self) {
        let Some(fd) = &self.fd else {
            std::thread::sleep(WITHOUT_SIGNAL);
            return;
        };
        let mut pollfd = libc::pollfd {
            fd: fd.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        // SAFETY: one valid `pollfd`, for the duration of the call.
        if unsafe { libc::poll(&mut pollfd, 1, -1) } < 0 {
            std::thread::sleep(WITHOUT_SIGNAL);
            return;
        }
        let mut count = 0u64;
        // SAFETY: `count` is valid for the 8 bytes an eventfd gives.
        let _ = unsafe { libc::read(fd.as_raw_fd(), (&raw mut count).cast(), 8) };
    }
}

impl Default for Wake {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{mpsc, Arc};

    const PATIENCE: Duration = Duration::from_secs(5);

    /// Runs `wake.wait()` on a thread of its own and hands back what says it returned.
    fn waiting(wake: &Arc<Wake>) -> mpsc::Receiver<()> {
        let (returned, receiver) = mpsc::channel();
        let wake = wake.clone();
        std::thread::spawn(move || {
            wake.wait();
            let _ = returned.send(());
        });
        receiver
    }

    #[test]
    fn a_wait_lasts_until_the_signal() {
        let wake = Arc::new(Wake::new());
        let returned = waiting(&wake);
        assert!(returned.recv_timeout(Duration::from_millis(100)).is_err());

        wake.notify();
        returned.recv_timeout(PATIENCE).expect("the signal ends it");
    }

    #[test]
    fn a_signal_given_before_the_wait_is_kept() {
        let wake = Arc::new(Wake::new());
        wake.notify();
        waiting(&wake)
            .recv_timeout(PATIENCE)
            .expect("the signal was waiting");
    }

    #[test]
    fn a_wait_consumes_every_signal_given_before_it() {
        let wake = Arc::new(Wake::new());
        wake.notify();
        wake.notify();
        wake.wait();

        let returned = waiting(&wake);
        assert!(
            returned.recv_timeout(Duration::from_millis(100)).is_err(),
            "nothing is left to wake the next one"
        );
        wake.notify();
        returned.recv_timeout(PATIENCE).expect("the signal ends it");
    }
}
