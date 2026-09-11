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
    uinput_ok && !enumerate_mice().is_empty()
}

/// Physical pointers we should route: a relative X/Y device carrying BTN_LEFT
/// (a mouse — not an accelerometer or a touchpad-gesture-only node), excluding
/// our own virtual device.
fn enumerate_mice() -> Vec<Node> {
    evdev::enumerate()
        .filter(|(_, d)| {
            d.name().map(|n| !n.contains(VIRTUAL_NAME)).unwrap_or(true)
                && d.supported_relative_axes()
                    .map(|a| {
                        a.contains(RelativeAxisCode::REL_X) && a.contains(RelativeAxisCode::REL_Y)
                    })
                    .unwrap_or(false)
                && d.supported_keys()
                    .map(|k| k.contains(KeyCode::BTN_LEFT))
                    .unwrap_or(false)
        })
        .collect()
}

/// Keyboards observed (never grabbed) for the ctrl-override: any node declaring
/// KEY_LEFTCTRL that is neither a routed mouse (grabbed — its ctrl usages come
/// through the grabbed stream) nor one of our own virtual devices.
fn enumerate_keyboards() -> Vec<Node> {
    let is_mouse = |d: &Device| {
        d.supported_relative_axes()
            .map(|a| a.contains(RelativeAxisCode::REL_X) && a.contains(RelativeAxisCode::REL_Y))
            .unwrap_or(false)
            && d.supported_keys()
                .map(|k| k.contains(KeyCode::BTN_LEFT))
                .unwrap_or(false)
    };
    evdev::enumerate()
        .filter(|(_, d)| {
            d.name()
                .map(|n| !n.contains("LittleBigMouse virtual"))
                .unwrap_or(true)
                && d.supported_keys()
                    .map(|k| k.contains(KeyCode::KEY_LEFTCTRL))
                    .unwrap_or(false)
                && !is_mouse(d)
        })
        .collect()
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

/// Cadence of the hot-plug rescan (matches the C# side's 2 s sysfs poll).
///
/// The enumeration itself runs on a dedicated scanner thread: opening and
/// querying every /dev/input node takes ~200 ms on some machines (nodes that
/// block on open), and doing that inline in the pump froze the cursor at every
/// rescan — a periodic "sticky mouse" felt in both algorithms.
const RESCAN_EVERY: Duration = Duration::from_secs(2);

/// One full enumeration pass: the mice to grab, then the keyboards to observe.
/// The expensive call — this is the ~210 ms one on a bad machine, so it belongs
/// to arm time or to the scanner thread, never to a pump cycle.
pub(super) fn enumerate() -> ScanResult {
    ScanResult {
        mice: enumerate_mice(),
        keyboards: enumerate_keyboards(),
    }
}

/// Start the enumeration thread and hand back the channel the pump drains and
/// the flag that stops it. The thread also exits on its next send once the
/// receiver is gone, so a `Router` dropped without setting the flag still ends
/// it within one cadence.
pub(super) fn spawn_scanner() -> (mpsc::Receiver<ScanResult>, Arc<AtomicBool>) {
    let (scan_tx, scan_rx) = mpsc::channel();
    let scan_stop = Arc::new(AtomicBool::new(false));
    let stop = scan_stop.clone();
    std::thread::spawn(move || loop {
        std::thread::sleep(RESCAN_EVERY);
        if stop.load(Ordering::Relaxed) {
            return;
        }
        if scan_tx.send(enumerate()).is_err() {
            return;
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
