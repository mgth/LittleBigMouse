//! Finding and holding the physical devices: which /dev/input nodes we route,
//! the hot-plug scanner thread, and the removal of nodes that have died.
//!
//! Enumeration is the expensive part of the whole backend — opening and querying
//! every node takes ~10 ms each on some machines (audio jack-detection nodes),
//! ~210 ms for a full scan — which is why it never runs on the routing thread.
//! It happens once before the first grab, then on the scanner thread; the pump
//! only drains a channel and grabs, which is a cheap ioctl.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::time::Duration;

use evdev::{Device, KeyCode, RelativeAxisCode};

use crate::hook::linux::accel::PointerAccel;

use super::uinput::VIRTUAL_NAME;
use super::BTN_MOUSE_RANGE;

/// An opened /dev/input node and the path it came from. What an enumeration
/// hands back, and what an observed keyboard stays as — only grabbed mice carry
/// more state.
pub(super) type Node = (PathBuf, Device);

/// A grabbed mouse: its node, plus its own acceleration state. Velocities must
/// not mix across devices, and kcminputrc settings are per-device.
pub(super) type Mouse = (PathBuf, Device, PointerAccel);

/// One background enumeration pass, handed to the pump over a channel.
pub(super) struct ScanResult {
    pub(super) mice: Vec<Node>,
    pub(super) keyboards: Vec<Node>,
}

/// True when we can create the uinput device and there is at least one mouse to
/// grab. Gates the backend so a permission-less box falls back to portal/X11.
pub fn available() -> bool {
    let uinput_ok = unsafe { libc::access(c"/dev/uinput".as_ptr(), libc::W_OK) == 0 };
    uinput_ok && !enumerate().mice.is_empty()
}

/// True for a uinput-created node (sysfs under /devices/virtual/). Remote-desktop
/// tools (RustDesk, input-leap…) inject through such devices with their own
/// absolute positioning; grabbing them swallows their motion while their buttons
/// still reach the compositor through our pointer, so clicks land at the wrong
/// place. `LBM_EVDEV_SKIP_VIRTUAL=1` leaves them to the compositor; off by
/// default since remappers (input-remapper…) re-emit the physical mouse this way.
fn is_virtual_node(path: &std::path::Path) -> bool {
    let Some(node) = path.file_name() else { return false };
    std::fs::canonicalize(std::path::Path::new("/sys/class/input").join(node))
        .map(|p| p.starts_with("/sys/devices/virtual"))
        .unwrap_or(false)
}

/// A relative X/Y device carrying BTN_LEFT: a mouse, not an accelerometer or a
/// touchpad-gesture-only node.
fn is_mouse(d: &Device) -> bool {
    d.supported_relative_axes()
        .map(|a| a.contains(RelativeAxisCode::REL_X) && a.contains(RelativeAxisCode::REL_Y))
        .unwrap_or(false)
        && d.supported_keys()
            .map(|k| k.contains(KeyCode::BTN_LEFT))
            .unwrap_or(false)
}

/// Sort one opened node into the scan: a mouse we should route (excluding our
/// own virtual pointer and, when opted in, other uinput devices), a keyboard to
/// observe for the ctrl-override (KEY_LEFTCTRL, not a mouse — a grabbed mouse's
/// ctrl usages come through the grabbed stream — and not one of our own virtual
/// devices), or neither, in which case the node is closed right here.
fn classify(scan: &mut ScanResult, path: PathBuf, d: Device, skip_virtual: bool) {
    if is_mouse(&d) {
        if d.name().map(|n| !n.contains(VIRTUAL_NAME)).unwrap_or(true)
            && !(skip_virtual && is_virtual_node(&path))
        {
            scan.mice.push((path, d));
        }
    } else if d.name().map(|n| !n.contains("LittleBigMouse virtual")).unwrap_or(true)
        && d.supported_keys()
            .map(|k| k.contains(KeyCode::KEY_LEFTCTRL))
            .unwrap_or(false)
    {
        scan.keyboards.push((path, d));
    }
}

/// A poll slot the kernel will never make readable again: the node was removed
/// (or was never valid). It reports its error forever, so leaving it in the set
/// turns `poll` into a no-wait call and the pump into a busy loop.
pub(super) fn slot_is_dead(revents: i16) -> bool {
    revents & (libc::POLLERR | libc::POLLHUP | libc::POLLNVAL) != 0
}

/// A device removed from the pump's lists by [`purge_dead`].
pub(super) enum Gone<M, K> {
    Mouse(M),
    Keyboard(K),
}

/// Drop the dead poll slots from the two device lists, handing each removed
/// entry to `gone` (the caller owns the reporting, and the value it needs to
/// report — the path — is inside the entry).
///
/// `dead` is ascending, so the removal walks it backwards: taking the highest
/// index first keeps the lower ones pointing at the same entries. Generic over
/// the entry types purely so the slot arithmetic can be tested without opening a
/// device.
pub(super) fn purge_dead<M, K>(
    dead: &[usize],
    n_mice: usize,
    mice: &mut Vec<M>,
    keyboards: &mut Vec<K>,
    mut gone: impl FnMut(Gone<M, K>),
) {
    for &slot in dead.iter().rev() {
        if slot >= n_mice {
            gone(Gone::Keyboard(keyboards.remove(slot - n_mice)));
        } else {
            gone(Gone::Mouse(mice.remove(slot)));
        }
    }
}
/// The [`BTN_MOUSE_RANGE`] buttons held on the freshly grabbed devices, as an
/// [`EvdevCursor::buttons`] mask. EVIOCGKEY is the only way to learn about a
/// press that happened *before* the grab: the event itself went to the
/// compositor, and the pump would otherwise start out believing nothing is down.
pub(super) fn held_buttons_of(devices: &[Mouse]) -> u8 {
    let mut mask = 0u8;
    for (_, d, _) in devices {
        let Ok(state) = d.get_key_state() else {
            continue;
        };
        for code in BTN_MOUSE_RANGE {
            if state.contains(KeyCode::new(code)) {
                mask |= 1u8 << (code - BTN_MOUSE_RANGE.start());
            }
        }
    }
    mask
}

/// Cadence of the hot-plug rescan when inotify is unavailable (matches the C#
/// side's 2 s sysfs poll).
///
/// The enumeration itself runs on a dedicated scanner thread: opening and
/// querying every /dev/input node takes ~200 ms on some machines (nodes that
/// block on open), and doing that inline in the pump froze the cursor at every
/// rescan — a periodic "sticky mouse" felt in both algorithms.
///
/// Polling is the fallback only: every scan closes each node it opened, and the
/// last close of a node nobody else holds (audio jack-detection nodes) runs
/// `synchronize_rcu()` in the kernel — 10–30 ms each. Re-opening every node every
/// 2 s kept the hook in uninterruptible sleep a sizeable share of the time and
/// was felt as a laggy pointer, so normally we only scan when /dev/input changes.
const RESCAN_EVERY: Duration = Duration::from_secs(2);

/// How long /dev/input must stay quiet after a change before we scan: a
/// hot-plug creates several nodes in a burst (eventN, mouseN, jsN), and one scan
/// should cover all of them.
const SETTLE: Duration = Duration::from_millis(300);

/// How often a scanner idling on inotify wakes to check its stop flag.
const STOP_CHECK: Duration = Duration::from_secs(1);

/// One full enumeration pass: the mice to grab and the keyboards to observe,
/// opening every node once. The expensive call — this is the ~210 ms one on a
/// bad machine, so it belongs to arm time or to the scanner thread, never to a
/// pump cycle.
pub(super) fn enumerate() -> ScanResult {
    let skip_virtual = std::env::var("LBM_EVDEV_SKIP_VIRTUAL").is_ok_and(|v| v == "1");
    let mut scan = ScanResult {
        mice: Vec::new(),
        keyboards: Vec::new(),
    };
    for (path, d) in evdev::enumerate() {
        classify(&mut scan, path, d, skip_virtual);
    }
    scan
}

/// An inotify watch on /dev/input for nodes appearing. Removals need no watch:
/// a removed node's poll slot reports POLLHUP and the pump purges it.
struct DirWatch(libc::c_int);

impl DirWatch {
    fn new() -> Option<DirWatch> {
        let fd = unsafe { libc::inotify_init1(libc::IN_NONBLOCK | libc::IN_CLOEXEC) };
        if fd < 0 {
            return None;
        }
        let watch = DirWatch(fd);
        let mask = libc::IN_CREATE | libc::IN_MOVED_TO;
        if unsafe { libc::inotify_add_watch(fd, c"/dev/input".as_ptr(), mask) } < 0 {
            return None;
        }
        Some(watch)
    }

    /// Wait up to `timeout` for a change, draining whatever is queued. True when
    /// something changed.
    fn wait(&self, timeout: Duration) -> bool {
        let mut pfd = libc::pollfd {
            fd: self.0,
            events: libc::POLLIN,
            revents: 0,
        };
        let ms = timeout.as_millis().min(libc::c_int::MAX as u128) as libc::c_int;
        if unsafe { libc::poll(&mut pfd, 1, ms) } <= 0 {
            return false;
        }
        // The events' contents don't matter, only that there were some.
        let mut buf = [0u8; 4096];
        let mut changed = false;
        while unsafe { libc::read(self.0, buf.as_mut_ptr().cast(), buf.len()) } > 0 {
            changed = true;
        }
        changed
    }
}

impl Drop for DirWatch {
    fn drop(&mut self) {
        unsafe { libc::close(self.0) };
    }
}

/// Start the enumeration thread and hand back the channel the pump drains and
/// the flag that stops it. The thread also exits on its next send once the
/// receiver is gone, so a `Router` dropped without setting the flag still ends
/// it on the next hot-plug.
///
/// With inotify the thread scans once on start (covering nodes that appeared
/// between the arm-time enumeration and the watch), then only when /dev/input
/// gains a node. Without it, it falls back to the fixed-cadence poll.
pub(super) fn spawn_scanner() -> (mpsc::Receiver<ScanResult>, Arc<AtomicBool>) {
    let (scan_tx, scan_rx) = mpsc::channel();
    let scan_stop = Arc::new(AtomicBool::new(false));
    let stop = scan_stop.clone();
    std::thread::spawn(move || {
        let watch = DirWatch::new();
        if watch.is_none() {
            eprintln!(
                "[LittleBigMouse.Hook] evdev: inotify on /dev/input unavailable, polling every {}s",
                RESCAN_EVERY.as_secs()
            );
        }
        let mut first = watch.is_some();
        loop {
            match &watch {
                Some(w) if !first => {
                    if !w.wait(STOP_CHECK) {
                        if stop.load(Ordering::Relaxed) {
                            return;
                        }
                        continue;
                    }
                    while w.wait(SETTLE) {}
                }
                Some(_) => first = false,
                None => std::thread::sleep(RESCAN_EVERY),
            }
            if stop.load(Ordering::Relaxed) {
                return;
            }
            if scan_tx.send(enumerate()).is_err() {
                return;
            }
        }
    });
    (scan_rx, scan_stop)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A removed node reports its error forever. Missing one of these flags turns
    /// the pump into a busy loop (`poll` returns instantly, every cycle).
    #[test]
    fn every_error_flag_condemns_the_slot() {
        for flag in [libc::POLLERR, libc::POLLHUP, libc::POLLNVAL] {
            assert!(slot_is_dead(flag), "flag {flag:#x} must condemn the slot");
            assert!(
                slot_is_dead(flag | libc::POLLIN),
                "readable *and* broken is still broken"
            );
        }
        assert!(!slot_is_dead(libc::POLLIN));
        assert!(!slot_is_dead(0));
    }

    /// Purging walks the slots from the highest down, so the indices it has not
    /// reached yet still point at the entries they were computed for.
    #[test]
    fn purging_removes_exactly_the_dead_slots() {
        let mut mice = vec!["m0", "m1", "m2"];
        let mut keyboards = vec!["k0", "k1"];
        let mut gone = Vec::new();

        // Slots 0 and 3: the first mouse and the first keyboard.
        purge_dead(&[0, 3], 3, &mut mice, &mut keyboards, |g| {
            gone.push(match g {
                Gone::Mouse(m) => format!("mouse {m}"),
                Gone::Keyboard(k) => format!("keyboard {k}"),
            })
        });

        assert_eq!(mice, vec!["m1", "m2"]);
        assert_eq!(keyboards, vec!["k1"]);
        assert_eq!(gone, vec!["keyboard k0", "mouse m0"]);
    }

    /// Every device gone at once — a receiver unplugged with its combined nodes.
    /// The pump then polls an empty set and waits for the scanner.
    #[test]
    fn purging_can_empty_both_lists() {
        let mut mice = vec!["m0", "m1"];
        let mut keyboards = vec!["k0"];

        purge_dead(&[0, 1, 2], 2, &mut mice, &mut keyboards, |_| {});

        assert!(mice.is_empty());
        assert!(keyboards.is_empty());
    }
}
