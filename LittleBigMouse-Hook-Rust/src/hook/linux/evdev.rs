//! evdev/uinput backend: the real Linux router, the counterpart of the Windows
//! low-level hook. It grabs every physical mouse (`EVIOCGRAB`, so the compositor
//! no longer sees them), runs the unchanged engine over an authoritative cursor
//! position, and drives one uinput virtual pointer. LBM is the sole source of
//! pointer motion — exactly like Windows, where `SetCursorPos` is the whole game
//! — so there is no portal, no capture notification, no compositor round-trip,
//! and fast motion is handled because we own the pipeline synchronously.
//!
//! The virtual pointer is ABSOLUTE: its ABS_X/ABS_Y range is the desktop's
//! logical pixel size, which KWin maps 1:1 onto the whole desktop (verified
//! live). Absolute devices are not accelerated by libinput, so the position we
//! emit IS the position the cursor takes — no gain, no drift, and a warp is just
//! the next absolute point. (A relative virtual pointer would inherit KWin's
//! per-device acceleration and desync from the engine's zone geometry, putting
//! "walls" in the middle of screens.)
//!
//! A second uinput device, the virtual keyboard, re-emits the keyboard usages of
//! grabbed mice: wireless receivers (Logitech Lightspeed…) expose one combined
//! kbd+mouse node, and its onboard macro buttons emit KEY_* codes the pointer
//! device does not declare — the kernel silently drops undeclared codes, so
//! without it those keys vanish while LBM runs.
//!
//! # RULE — nothing potentially blocking on the routing thread
//!
//! From the first `EVIOCGRAB` the physical mice deliver ONLY to this process:
//! any stall in the pump freezes the user's pointer system-wide. Concretely:
//! - no device enumeration inline (~10 ms PER /dev/input node on some
//!   machines — audio jack-detection nodes; ~210 ms per full scan, measured by
//!   `examples/enum_bench.rs`) — enumeration lives on the scanner thread, the
//!   pump only drains its channel and grabs (a cheap ioctl);
//! - no blocking locks — the engine is accessed with `try_lock` (a contended
//!   frame emits raw / keeps cached bounds for one cycle);
//! - no allocation per report — every buffer the cycle needs lives in
//!   [`PumpBuffers`], owned by the `Router` and reused; a call into the global
//!   allocator a thousand times a second is a stall waiting to happen. Measured
//!   by `benches/evdev_pump.rs`;
//! - no synchronous IPC/DBus — the KWin cursor probe runs at arm time, BEFORE
//!   the grabs;
//! - no unbounded writes — state broadcasts go to sockets with a write
//!   timeout (ipc/server.rs); stderr must stay a file/journal, never an
//!   undrained pipe (the C# spawns the daemon with inherited handles — keep
//!   it that way);
//! - the only permitted wait is the bounded 100 ms `poll()` (and the
//!   equivalent sleep when no device is left).
//!
//! Audited 2026-07-19. The same rule applies to the other platform pumps
//! (hook/windows LL-hook callback, hook/linux/x11).
//!
//! Safety: a grab is released when its fd closes, so even `kill -9` frees the
//! mice. We additionally ungrab on unhook, on quit, and on drop; `LBM_EVDEV_
//! AUTORELEASE_SECS` force-unhooks after N seconds for cautious first runs.
//!
//! Freeing the devices is not enough on its own: a button held at that moment
//! was pressed on the *virtual* pointer, so it must be released there before the
//! device disappears, or the seat keeps it down and the user loses that button.
//! `Drop for Router` pays that debt, and the mask it reads is seeded at arm time
//! from `EVIOCGKEY` — a press that predates the grab never reached the pump.
//!
//! Ctrl-override reads the modifier from keyboards WITHOUT grabbing them (evdev
//! nodes are multi-reader; the compositor keeps them), plus the ctrl usages of
//! the grabbed combined nodes. Hot-plug is handled by a periodic rescan (new
//! mice would otherwise drive the cursor directly, next to the engine) and by
//! purging dead nodes — a removed device reports POLLERR forever and would
//! otherwise turn the pump into a busy loop. Focus-based exclusion is handled
//! by the [`super::focus`] watcher, common to all backends: it flips
//! `want_hook`, and the reconcile below ungrabs/regrabs accordingly.

use std::os::fd::{AsRawFd, RawFd};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};

use evdev::{
    uinput::{VirtualDevice, VirtualDeviceBuilder},
    AbsInfo, AbsoluteAxisCode, AttributeSet, BusType, Device, EventType, InputEvent, InputId,
    KeyCode, RelativeAxisCode, UinputAbsSetup,
};

use crate::engine::cursor::CursorEnv;
use crate::engine::event::MouseEventArg;
use crate::geometry::{Point, Rect};
use crate::ipc::protocol;
use crate::shared::Shared;

const VIRTUAL_NAME: &str = "LittleBigMouse virtual pointer";
const VIRTUAL_KBD_NAME: &str = "LittleBigMouse virtual keyboard";

/// EV_KEY code ranges of mouse buttons. Everything else on a grabbed mouse is a
/// keyboard usage: wireless receivers (Logitech Lightspeed…) expose one combined
/// kbd+mouse node, and onboard macro buttons emit KEY_ESC/KEY_TAB/… on it. The
/// kernel silently drops events whose (type, code) is not declared on a uinput
/// device, so those keys must go to a virtual device that declares them.
const BTN_RANGE: std::ops::RangeInclusive<u16> = 0x100..=0x15f;
/// BTN_TRIGGER_HAPPY block — joystick buttons, not keyboard usages.
const BTN_TRIGGER_HAPPY_RANGE: std::ops::RangeInclusive<u16> = 0x2c0..=0x2e7;
/// BTN_LEFT..=BTN_TASK — the buttons whose being held counts as a drag, and
/// exactly the set the virtual pointer declares in `build_virtual`.
const BTN_MOUSE_RANGE: std::ops::RangeInclusive<u16> = 0x110..=0x117;

/// Cadence of the hot-plug rescan (matches the C# side's 2 s sysfs poll).
///
/// The enumeration itself runs on a dedicated scanner thread: opening and
/// querying every /dev/input node takes ~200 ms on some machines (nodes that
/// block on open), and doing that inline in the pump froze the cursor at every
/// rescan — a periodic "sticky mouse" felt in both algorithms.
const RESCAN_EVERY: Duration = Duration::from_secs(2);

/// One background enumeration pass, handed to the pump over a channel.
struct ScanResult {
    mice: Vec<(std::path::PathBuf, Device)>,
    keyboards: Vec<(std::path::PathBuf, Device)>,
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
fn enumerate_mice() -> Vec<(std::path::PathBuf, Device)> {
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
fn enumerate_keyboards() -> Vec<(std::path::PathBuf, Device)> {
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

pub fn run(shared: &'static Shared) -> bool {
    let autorelease = std::env::var("LBM_EVDEV_AUTORELEASE_SECS")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .map(Duration::from_secs);

    // Pointer sensitivity: raw device counts per logical pixel. Absolute output
    // makes this a pure feel knob (it never affects zone geometry). Default 1:1.
    // Applied ON TOP of the libinput-parity pointer acceleration (see accel.rs;
    // override with LBM_EVDEV_ACCEL=none|flat|adaptive and LBM_EVDEV_ACCEL_SPEED).
    let sens = std::env::var("LBM_EVDEV_SENS")
        .ok()
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(1.0);

    let debug = std::env::var("LBM_HOOK_DEBUG").is_ok();

    let mut router: Option<Router> = None;
    let mut hooked_since: Option<Instant> = None;
    let mut last_report = Instant::now();
    let mut last_events = 0u64;
    // Where the previous Router left the cursor: the re-arm fallback when the
    // compositor cannot be asked for the real position.
    let mut resume_at: Option<Point<i32>> = None;

    eprintln!("[LittleBigMouse.Hook] evdev backend running (sens={sens})");

    loop {
        if shared.want_quit.load(Ordering::SeqCst) {
            drop(router);
            return true;
        }

        let want = shared.want_hook.load(Ordering::SeqCst);

        if want && router.is_none() {
            match Router::arm(shared, sens, debug, resume_at) {
                Ok(r) => {
                    router = Some(r);
                    hooked_since = Some(Instant::now());
                    shared.hooked.store(true, Ordering::SeqCst);
                    crate::platform::set_process_priority(crate::priority::Priority::from_u8(
                        shared.priority.load(Ordering::SeqCst),
                    ));
                    shared.broadcast(protocol::RUNNING);
                }
                Err(e) => {
                    eprintln!("[LittleBigMouse.Hook] evdev: arm failed: {e}");
                    shared.want_hook.store(false, Ordering::SeqCst);
                    shared.broadcast(protocol::STOPPED);
                }
            }
        } else if !want && router.is_some() {
            resume_at = router.as_ref().map(|r| r.env.virtual_pos);
            drop(router.take());
            hooked_since = None;
            shared.hooked.store(false, Ordering::SeqCst);
            crate::platform::set_process_priority(crate::priority::Priority::from_u8(
                shared.priority_unhooked.load(Ordering::SeqCst),
            ));
            shared.broadcast(protocol::STOPPED);
        }

        if let (Some(deadline), Some(since)) = (autorelease, hooked_since) {
            if since.elapsed() >= deadline {
                eprintln!("[LittleBigMouse.Hook] evdev: auto-release deadline, unhooking");
                shared.want_hook.store(false, Ordering::SeqCst);
                continue;
            }
        }

        match router.as_mut() {
            Some(r) => r.pump(shared),
            None => std::thread::sleep(Duration::from_millis(50)),
        }

        if debug && router.is_some() && last_report.elapsed() >= Duration::from_secs(2) {
            let events = crate::hook::MOUSE_EVENTS.load(Ordering::Relaxed);
            let crossings = crate::hook::CROSSINGS.load(Ordering::Relaxed);
            if events != last_events {
                let p = router.as_ref().unwrap().env.virtual_pos;
                eprintln!("[LittleBigMouse.Hook] evdev: {} motion events, {crossings} crossings (pos {},{})",
                    events - last_events, p.x(), p.y());
                last_events = events;
            }
            last_report = Instant::now();
        }
    }
}

/// Whether an event closed the frame being accumulated.
///
/// `SYN_REPORT` is the kernel's frame delimiter: everything read before it
/// belongs to one atomic report (a REL_X and a REL_Y are one motion, not two),
/// so the pump only runs the engine and writes to uinput on a `Complete`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[must_use]
pub enum Frame {
    Pending,
    Complete,
}

/// Every buffer the pump reuses, owned by the [`Router`] for the whole hooked
/// session instead of being recreated per cycle.
///
/// The pump runs once per readable batch — up to one cycle per mouse report, so
/// ~1000/s per grabbed device at a 1 kHz polling rate. Each cycle used to build
/// a `Vec` for the poll set, one per `fetch_events` drain, one for the pending
/// pointer events, one for the pending keyboard events and one per uinput frame;
/// at steady state those are all the same sizes over and over. Holding them here
/// makes the routing path allocation-free once the capacities have settled,
/// which is what the module's "nothing potentially blocking" rule is really
/// after: an allocation is a call into the global allocator, and a slow path
/// through it is a stall with the user's mice grabbed.
///
/// Public (with private fields) so `benches/evdev_pump.rs` can drive the
/// classification and frame composition without a device or `/dev/uinput`.
pub struct PumpBuffers {
    /// `poll(2)` set: the grabbed mice first, then the observed keyboards. The
    /// split point is the mouse count, which is how a slot maps back to a list.
    fds: Vec<libc::pollfd>,
    /// One device's `fetch_events` drain. Collected rather than iterated because
    /// processing a frame needs `&mut Router`, which the iterator borrows from.
    events: Vec<InputEvent>,
    /// Poll slots that reported POLLERR/POLLHUP/POLLNVAL, ascending.
    dead: Vec<usize>,
    /// Raw REL_X/REL_Y counts accumulated since the last frame.
    acc: (i64, i64),
    /// Wheels, buttons and MSC_SCAN waiting to ride the frame's pointer write.
    passthrough: Vec<InputEvent>,
    /// Keyboard usages of grabbed mice, waiting for the virtual keyboard's own
    /// frame.
    kbd: Vec<InputEvent>,
    /// The composed uinput pointer frame: the absolute point, then
    /// `passthrough`.
    batch: Vec<InputEvent>,
    /// Index (into `Router::devices`) of the mouse that produced the motion
    /// being accumulated — its accelerator gets fed at flush time.
    last_motion_dev: usize,
}

impl Default for PumpBuffers {
    fn default() -> Self {
        PumpBuffers::new()
    }
}

impl PumpBuffers {
    /// Capacities sized for the ordinary session — a couple of mice and
    /// keyboards, a frame or two of events — so the steady state never grows
    /// them. They are a starting point, not a limit: a burst reallocates once
    /// and the larger capacity is then kept for the rest of the session.
    pub fn new() -> PumpBuffers {
        PumpBuffers {
            fds: Vec::with_capacity(8),
            events: Vec::with_capacity(64),
            dead: Vec::with_capacity(4),
            acc: (0, 0),
            passthrough: Vec::with_capacity(16),
            kbd: Vec::with_capacity(16),
            batch: Vec::with_capacity(16),
            last_motion_dev: 0,
        }
    }

    /// Rebuild the poll set from the current devices. Called every cycle: the
    /// device list changes on hot-plug, and `poll` writes `revents` in place, so
    /// the set has to be reset even when the fds are the same.
    pub fn fill_poll_set(&mut self, fds: impl Iterator<Item = RawFd>) {
        self.fds.clear();
        self.fds.extend(fds.map(|fd| libc::pollfd {
            fd,
            events: libc::POLLIN,
            revents: 0,
        }));
    }

    /// Replace the pending drain with one device's events.
    pub fn refill_events(&mut self, events: impl Iterator<Item = InputEvent>) {
        self.events.clear();
        self.events.extend(events);
    }

    /// How many events the last [`refill_events`](Self::refill_events) holds.
    pub fn event_count(&self) -> usize {
        self.events.len()
    }

    /// The `k`th event of the drain, by value: routing one needs `&mut Router`,
    /// which a reference into the buffer would hold hostage. `InputEvent` is 24
    /// bytes of `Copy`, so this is a load, not a lifetime problem.
    pub fn event(&self, k: usize) -> InputEvent {
        self.events[k]
    }

    /// Route one event of the frame being accumulated. `dev` is the poll slot of
    /// the mouse it came from, remembered for the acceleration curve.
    ///
    /// Order is the contract here: motion accumulates, everything else queues in
    /// arrival order in the buffer that matches its destination device, and
    /// `SYN_REPORT` closes the frame.
    pub fn push(&mut self, ev: InputEvent, env: &mut EvdevCursor, dev: usize) -> Frame {
        match ev.event_type() {
            EventType::SYNCHRONIZATION => return Frame::Complete,
            EventType::RELATIVE if ev.code() == RelativeAxisCode::REL_X.0 => {
                self.acc.0 += ev.value() as i64;
                self.last_motion_dev = dev;
            }
            EventType::RELATIVE if ev.code() == RelativeAxisCode::REL_Y.0 => {
                self.acc.1 += ev.value() as i64;
                self.last_motion_dev = dev;
            }
            // Wheels and any other relative axis: pass through verbatim.
            EventType::RELATIVE => self.passthrough.push(ev),
            // Buttons stay with the pointer; every other EV_KEY code is a
            // keyboard usage (onboard macros on combined receiver nodes) and goes
            // to the virtual keyboard, which declares it.
            EventType::KEY
                if BTN_RANGE.contains(&ev.code())
                    || BTN_TRIGGER_HAPPY_RANGE.contains(&ev.code()) =>
            {
                env.track_button(ev.code(), ev.value());
                self.passthrough.push(ev);
            }
            EventType::KEY => {
                // Combined receiver nodes carry the modifier too.
                env.track_ctrl(ev.code(), ev.value());
                self.kbd.push(ev);
            }
            // MSC_SCAN scancodes stay with the pointer frame: losing the
            // scancode of a routed key is inconsequential.
            EventType::MISC => self.passthrough.push(ev),
            _ => {}
        }
        Frame::Pending
    }

    /// Whether anything is waiting to be emitted. False right after a flush, and
    /// for a cycle that read only events the pump ignores.
    pub fn frame_pending(&self) -> bool {
        self.acc != (0, 0) || !self.passthrough.is_empty() || !self.kbd.is_empty()
    }

    /// The raw counts accumulated since the last frame, reset to zero. Taken
    /// rather than read: a delta the engine has been run over must not be
    /// applied twice, on any path out of the flush.
    pub fn take_motion(&mut self) -> (i64, i64) {
        std::mem::take(&mut self.acc)
    }

    /// Compose the pointer frame: the absolute point first, then the pending
    /// buttons/wheels/scancodes in arrival order. Drains `passthrough` — those
    /// events are now the caller's to write — and leaves its capacity behind.
    ///
    /// uinput appends the closing `SYN_REPORT` itself, so the emitted frame is
    /// ABS_X, ABS_Y, passthrough…, SYN_REPORT.
    pub fn pointer_frame(&mut self, ax: i32, ay: i32) -> &[InputEvent] {
        self.batch.clear();
        self.batch.push(InputEvent::new(
            EventType::ABSOLUTE.0,
            AbsoluteAxisCode::ABS_X.0,
            ax,
        ));
        self.batch.push(InputEvent::new(
            EventType::ABSOLUTE.0,
            AbsoluteAxisCode::ABS_Y.0,
            ay,
        ));
        self.batch.append(&mut self.passthrough);
        &self.batch
    }

    /// Hand the pending keyboard usages to `write` as one frame, then drop them.
    /// A no-op when none are pending, which is the ordinary case: only combined
    /// receiver nodes emit KEY_* codes through a grabbed mouse.
    pub fn take_keyboard_frame(&mut self, write: impl FnOnce(&[InputEvent])) {
        if self.kbd.is_empty() {
            return;
        }
        write(&self.kbd);
        self.kbd.clear();
    }
}

/// A poll slot the kernel will never make readable again: the node was removed
/// (or was never valid). It reports its error forever, so leaving it in the set
/// turns `poll` into a no-wait call and the pump into a busy loop.
fn slot_is_dead(revents: i16) -> bool {
    revents & (libc::POLLERR | libc::POLLHUP | libc::POLLNVAL) != 0
}

/// A device removed from the pump's lists by [`purge_dead`].
enum Gone<M, K> {
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
fn purge_dead<M, K>(
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

/// The grabbed devices, the virtual pointer + keyboard, and the authoritative cursor.
struct Router {
    /// Each grabbed mouse carries its own acceleration state: velocities must
    /// not mix across devices, and kcminputrc settings are per-device.
    devices: Vec<(std::path::PathBuf, Device, super::accel::PointerAccel)>,
    /// Observed (non-grabbed) keyboards feeding the ctrl-override state.
    keyboards: Vec<(std::path::PathBuf, Device)>,
    /// Hot-plug scans arriving from the scanner thread.
    scan_rx: mpsc::Receiver<ScanResult>,
    scan_stop: Arc<AtomicBool>,
    virt: VirtualDevice,
    /// Keyboard usages coming from grabbed mice (combined receiver nodes, onboard
    /// macros) are re-emitted here — the pointer device does not declare them, and
    /// mixing a full keyboard into an ABS pointer risks a libinput/KWin
    /// reclassification of the pointer.
    virt_kbd: VirtualDevice,
    env: EvdevCursor,
    sens: f64,
    debug: bool,
    /// Sub-pixel remainder of the sensitivity-scaled raw motion integration.
    rem: (f64, f64),
    /// kcminputrc + env overrides, resolved per device at grab time (arm and
    /// hot-plug rescan) — the pump itself never touches the filesystem.
    accel_cfg: super::accel::AccelConfig,
    /// Poll set, event drains and pending frames — reused cycle after cycle so
    /// the routing path does not allocate.
    bufs: PumpBuffers,
}

impl Router {
    fn arm(
        shared: &Shared,
        sens: f64,
        debug: bool,
        resume_at: Option<Point<i32>>,
    ) -> std::io::Result<Router> {
        let desktop = desktop_bounds_blocking(shared);

        // Everything slow happens BEFORE the first grab: from EVIOCGRAB on, the
        // user's mice are captured but not routed yet, so this window must stay
        // minimal (see the module's routing-thread rule). The compositor probe
        // (DBus, ~700ms worst case) and the enumerations (~10ms per /dev/input
        // node) therefore run first; the cursor may drift a few px between the
        // probe and the grab, which is harmless for a start point.
        let probed = kwin_cursor_pos();
        // Pointer-accel settings (kcminputrc read) also belong to the slow
        // pre-grab phase: the pump must stay I/O-free.
        let accel_cfg = super::accel::AccelConfig::load();
        let mice = enumerate_mice();
        let mut keyboards = Vec::new();
        for (path, mut dev) in enumerate_keyboards() {
            if dev.set_nonblocking(true).is_ok() {
                keyboards.push((path, dev));
            }
        }
        let virt = build_virtual(desktop)?;
        let virt_kbd = build_virtual_keyboard()?;

        let mut devices = Vec::new();
        for (path, mut dev) in mice {
            dev.set_nonblocking(true)?;
            match dev.grab() {
                Ok(()) => {
                    let id = dev.input_id();
                    let name = dev.name().unwrap_or("?").to_string();
                    let settings = accel_cfg.for_device(id.vendor(), id.product(), &name);
                    eprintln!("[LittleBigMouse.Hook] evdev: grabbed {name} ({path:?}, accel {:?} speed {})",
                        settings.profile, settings.speed);
                    devices.push((
                        path,
                        dev,
                        super::accel::PointerAccel::new(settings.profile, settings.speed),
                    ));
                }
                Err(e) => eprintln!("[LittleBigMouse.Hook] evdev: cannot grab {path:?}: {e}"),
            }
        }
        if devices.is_empty() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "no grabbable mouse",
            ));
        }
        eprintln!(
            "[LittleBigMouse.Hook] evdev: observing {} keyboard(s) for ctrl-override",
            keyboards.len()
        );

        // Take over from where the cursor really is: ask the compositor (KWin
        // scripting, logical coordinates — the zones' space), else where the
        // previous arm left it. Only a first arm on a non-KDE compositor falls
        // back to a neutral point (centre of the first main zone).
        let (start, origin) = match probed {
            Some(p) => (p, "compositor"),
            None => match resume_at {
                Some(p) => (p, "previous position"),
                None => (
                    first_zone_center(shared).unwrap_or_else(|| {
                        Point::new(
                            desktop.left() + desktop.width() / 2,
                            desktop.top() + desktop.height() / 2,
                        )
                    }),
                    "fallback",
                ),
            },
        };
        let start = Point::new(
            start
                .x()
                .clamp(desktop.left(), desktop.left() + desktop.width() - 1),
            start
                .y()
                .clamp(desktop.top(), desktop.top() + desktop.height() - 1),
        );
        eprintln!(
            "[LittleBigMouse.Hook] evdev: starting at ({},{}) ({origin})",
            start.x(),
            start.y()
        );

        // The scanner thread owns the expensive enumeration; the pump only
        // drains its channel. It exits on the stop flag or once the Router
        // (receiver) is gone.
        let (scan_tx, scan_rx) = mpsc::channel();
        let scan_stop = Arc::new(AtomicBool::new(false));
        {
            let stop = scan_stop.clone();
            std::thread::spawn(move || loop {
                std::thread::sleep(RESCAN_EVERY);
                if stop.load(Ordering::Relaxed) {
                    return;
                }
                let scan = ScanResult {
                    mice: enumerate_mice(),
                    keyboards: enumerate_keyboards(),
                };
                if scan_tx.send(scan).is_err() {
                    return;
                }
            });
        }

        // Buttons already held when the grab lands: the press went to the
        // compositor from the real device, and from here on the release comes
        // out of the virtual one. Starting from the real state keeps
        // `buttons_down()` honest for the drag detection, and makes the
        // teardown release cover a button that was held across the whole
        // session.
        let mut env = EvdevCursor::new(desktop, start);
        env.buttons = held_buttons_of(&devices);

        let mut router = Router {
            devices,
            keyboards,
            scan_rx,
            scan_stop,
            virt,
            virt_kbd,
            env,
            sens,
            debug,
            rem: (0.0, 0.0),
            accel_cfg,
            bufs: PumpBuffers::new(),
        };

        // Place the cursor at the start point and prime the engine there.
        router.emit_absolute();
        let mut engine = shared.engine.lock().unwrap_or_else(|p| p.into_inner());
        let mut e = MouseEventArg::new(start);
        engine.on_mouse_move(&mut router.env, &mut e);
        drop(engine);

        Ok(router)
    }

    /// One poll cycle: drain readable devices, process each SYN frame.
    fn pump(&mut self, shared: &'static Shared) {
        // The desktop can change under us (a Load with a new layout). Rebuild the
        // absolute device to the new size so the 1:1 mapping stays exact.
        // try_desktop_bounds: never block on the engine lock here (module rule);
        // on contention the cached bounds serve one more cycle.
        if let Some(current) = try_desktop_bounds(shared) {
            if current != self.env.desktop {
                if let Ok(v) = build_virtual(current) {
                    self.virt = v;
                    self.env.desktop = current;
                    self.env.virtual_pos = self.env.clamp(self.env.virtual_pos);
                    self.emit_absolute();
                }
            }
        }

        self.rescan();

        let n_mice = self.devices.len();
        {
            let Router {
                bufs,
                devices,
                keyboards,
                ..
            } = self;
            bufs.fill_poll_set(
                devices
                    .iter()
                    .map(|(_, d, _)| d.as_raw_fd())
                    .chain(keyboards.iter().map(|(_, d)| d.as_raw_fd())),
            );
        }
        if self.bufs.fds.is_empty() {
            // Every device vanished: keep the cadence (poll(0 fds) returns
            // immediately) and let the rescan pick devices back up.
            std::thread::sleep(Duration::from_millis(100));
            return;
        }
        unsafe {
            libc::poll(
                self.bufs.fds.as_mut_ptr(),
                self.bufs.fds.len() as libc::nfds_t,
                100,
            );
        }

        // Indexed rather than iterated: processing an event needs `&mut self`,
        // which an iterator borrowed from `self.bufs` would hold hostage. Both
        // `pollfd` and `InputEvent` are `Copy`, so a slot is read out and the
        // borrow ends there.
        for slot in 0..self.bufs.fds.len() {
            let revents = self.bufs.fds[slot].revents;
            if slot_is_dead(revents) {
                self.bufs.dead.push(slot);
                continue;
            }
            if revents & libc::POLLIN == 0 {
                continue;
            }
            if slot >= n_mice {
                self.track_ctrl_from_keyboard(slot - n_mice);
                continue;
            }
            {
                let Router { bufs, devices, .. } = self;
                match devices[slot].1.fetch_events() {
                    Ok(it) => bufs.refill_events(it),
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => continue,
                    Err(_) => {
                        // Dropping the Device closes the fd, which also releases
                        // the grab — if the node was actually alive the
                        // compositor gets it back rather than the user losing
                        // the mouse.
                        bufs.dead.push(slot);
                        continue;
                    }
                }
            }
            for k in 0..self.bufs.event_count() {
                let ev = self.bufs.event(k);
                if self.bufs.push(ev, &mut self.env, slot) == Frame::Complete {
                    self.flush_frame(shared);
                }
            }
        }
        // A frame split across two reads (or a device that stopped mid-report)
        // leaves events with no SYN behind them: emit what is pending rather
        // than hold the motion until the next one. A no-op when nothing is.
        self.flush_frame(shared);

        if !self.bufs.dead.is_empty() {
            let Router {
                bufs,
                devices,
                keyboards,
                ..
            } = self;
            purge_dead(&bufs.dead, n_mice, devices, keyboards, |gone| {
                let (path, kind) = match &gone {
                    Gone::Mouse((path, _, _)) => (path, "device"),
                    Gone::Keyboard((path, _)) => (path, "keyboard"),
                };
                eprintln!("[LittleBigMouse.Hook] evdev: {kind} gone {path:?}");
            });
            bufs.dead.clear();
        }
    }

    /// Drain a (non-grabbed) keyboard, keeping only the ctrl state: the
    /// compositor still owns these devices, we just observe the modifier.
    ///
    /// Nothing here needs `&mut Router`, only the cursor's modifier state, so
    /// the drain is consumed as it comes: `keyboards` and `env` are distinct
    /// fields, and the events never have to be collected anywhere.
    fn track_ctrl_from_keyboard(&mut self, k: usize) {
        let Router { keyboards, env, .. } = self;
        let Ok(events) = keyboards[k].1.fetch_events() else {
            return; // dead nodes are purged via POLLERR next cycle
        };
        for ev in events {
            if ev.event_type() == EventType::KEY {
                env.track_ctrl(ev.code(), ev.value());
            }
        }
    }

    /// Drain the scanner thread's results and pick up hot-plugged devices. A
    /// mouse appearing mid-run would otherwise drive the cursor directly, next
    /// to the engine; a new keyboard would not feed the ctrl-override. Grabbing
    /// is a cheap ioctl — the expensive enumeration happened off-thread.
    fn rescan(&mut self) {
        while let Ok(scan) = self.scan_rx.try_recv() {
            for (path, mut dev) in scan.mice {
                if self.devices.iter().any(|(p, _, _)| *p == path) {
                    continue;
                }
                if dev.set_nonblocking(true).is_err() {
                    continue;
                }
                match dev.grab() {
                    Ok(()) => {
                        let id = dev.input_id();
                        let name = dev.name().unwrap_or("?").to_string();
                        let settings = self.accel_cfg.for_device(id.vendor(), id.product(), &name);
                        eprintln!("[LittleBigMouse.Hook] evdev: grabbed {name} ({path:?}, hot-plug, accel {:?} speed {})",
                            settings.profile, settings.speed);
                        self.devices.push((
                            path,
                            dev,
                            super::accel::PointerAccel::new(settings.profile, settings.speed),
                        ));
                    }
                    // Retried every rescan; only worth the noise when debugging.
                    Err(e) if self.debug => {
                        eprintln!("[LittleBigMouse.Hook] evdev: cannot grab {path:?}: {e}")
                    }
                    Err(_) => {}
                }
            }
            for (path, mut dev) in scan.keyboards {
                if self.keyboards.iter().any(|(p, _)| *p == path) {
                    continue;
                }
                if dev.set_nonblocking(true).is_ok() {
                    self.keyboards.push((path, dev));
                }
            }
        }
    }

    /// Run the engine over the accumulated motion and place the cursor at the
    /// resulting absolute position, forwarding buttons/wheels in the same frame.
    fn flush_frame(&mut self, shared: &Shared) {
        if !self.bufs.frame_pending() {
            return;
        }

        let acc = self.bufs.take_motion();
        if acc != (0, 0) {
            // Reproduce the pointer acceleration libinput would have applied had
            // we not grabbed the device (the ABS virtual pointer bypasses it):
            // per-device curve on the raw frame delta, THEN the user's flat
            // LBM_EVDEV_SENS multiplier. Sub-pixel remainders carry over frames.
            let now = self.env.started.elapsed().as_micros() as u64;
            let idx = self
                .bufs
                .last_motion_dev
                .min(self.devices.len().saturating_sub(1));
            let (ax, ay) = match self.devices.get_mut(idx) {
                Some((_, _, accel)) => accel.apply(acc.0 as f64, acc.1 as f64, now),
                None => (acc.0 as f64, acc.1 as f64),
            };
            let sx = ax * self.sens + self.rem.0;
            let sy = ay * self.sens + self.rem.1;
            let (dx, dy) = (sx.trunc() as i32, sy.trunc() as i32);
            self.rem = (sx - dx as f64, sy - dy as f64);

            let old = self.env.virtual_pos;
            // Win32 parity: the LL hook sees the UNCLIPPED proposed point even while
            // ClipCursor pins the cursor — the growing past-border distance is what
            // drains border resistance. Only the committed position gets clamped.
            let candidate = Point::new(old.x().saturating_add(dx), old.y().saturating_add(dy));

            let mut engine = match shared.engine.try_lock() {
                Ok(g) => g,
                Err(std::sync::TryLockError::Poisoned(p)) => p.into_inner(),
                Err(std::sync::TryLockError::WouldBlock) => {
                    self.env.virtual_pos = self.env.clamp(candidate);
                    self.emit_absolute();
                    return;
                }
            };
            let mut e = MouseEventArg::new(candidate);
            engine.on_mouse_move(&mut self.env, &mut e);
            drop(engine);

            if !e.handled {
                self.env.virtual_pos = self.env.clamp(candidate);
            }
            crate::hook::MOUSE_EVENTS.fetch_add(1, Ordering::Relaxed);
            if e.handled {
                crate::hook::CROSSINGS.fetch_add(1, Ordering::Relaxed);
            }
            if self.debug {
                // Per-frame trace: raw delta, engine input, emitted position. The
                // ground truth for any "the cursor was seen somewhere we never
                // sent it" investigation (compare against what KWin displays).
                eprintln!("[LittleBigMouse.Hook] evdev: frame d=({dx},{dy}) cand=({},{}) -> emit ({},{}){}{}",
                    candidate.x(), candidate.y(), self.env.virtual_pos.x(), self.env.virtual_pos.y(),
                    if e.handled { " CROSS" } else { "" },
                    if self.env.ctrl_down() { " ctrl" } else { "" });
            }
        }

        self.emit_absolute();

        // Keyboard usages get their own atomic frame on the virtual keyboard
        // (emit appends the SYN_REPORT), mirroring the per-device framing of the
        // pointer batch above.
        let Router { bufs, virt_kbd, .. } = self;
        bufs.take_keyboard_frame(|frame| {
            let _ = virt_kbd.emit(frame);
        });
    }

    /// Emit the current absolute position plus any pending buttons/wheels as one
    /// atomic uinput frame. ABS values are desktop-relative (the ABS range starts
    /// at 0), so the compositor's 1:1 mapping lands the cursor exactly.
    fn emit_absolute(&mut self) {
        let ax = self.env.virtual_pos.x() - self.env.desktop.left();
        let ay = self.env.virtual_pos.y() - self.env.desktop.top();
        let frame = self.bufs.pointer_frame(ax, ay);
        let _ = self.virt.emit(frame);
    }
}

impl Drop for Router {
    fn drop(&mut self) {
        self.scan_stop.store(true, Ordering::Relaxed);
        // A button still held here is one the compositor saw pressed on the
        // virtual pointer and would never see released: the device is about to
        // disappear, and whatever release follows lands on the real device the
        // ungrab below hands back. The seat keeps the button down, and the user
        // loses that button until something resets the state — the failure
        // reads as "I have no left click any more". Pay the releases while the
        // virtual device is still alive.
        let releases = self.env.release_frame();
        if !releases.is_empty() {
            let _ = self.virt.emit(&releases);
            self.env.buttons = 0;
        }
        for (_, d, _) in &mut self.devices {
            let _ = d.ungrab();
        }
        // Keyboards were never grabbed; closing their fds is enough. The scanner
        // thread exits on the flag (or on its next send, once scan_rx is gone).
    }
}

/// The [`BTN_MOUSE_RANGE`] buttons held on the freshly grabbed devices, as an
/// [`EvdevCursor::buttons`] mask. EVIOCGKEY is the only way to learn about a
/// press that happened *before* the grab: the event itself went to the
/// compositor, and the pump would otherwise start out believing nothing is down.
fn held_buttons_of(devices: &[(std::path::PathBuf, Device, super::accel::PointerAccel)]) -> u8 {
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

/// An absolute virtual pointer whose ABS range equals the desktop size, plus
/// buttons and (relative) wheels. KWin maps the ABS range 1:1 onto the whole
/// desktop, so no acceleration is applied and the emitted point is the position.
fn build_virtual(desktop: Rect<i32>) -> std::io::Result<VirtualDevice> {
    let mut keys = AttributeSet::<KeyCode>::new();
    for k in [
        KeyCode::BTN_LEFT,
        KeyCode::BTN_RIGHT,
        KeyCode::BTN_MIDDLE,
        KeyCode::BTN_SIDE,
        KeyCode::BTN_EXTRA,
        KeyCode::BTN_FORWARD,
        KeyCode::BTN_BACK,
        KeyCode::BTN_TASK,
    ] {
        keys.insert(k);
    }

    let mut wheels = AttributeSet::<RelativeAxisCode>::new();
    for a in [
        RelativeAxisCode::REL_WHEEL,
        RelativeAxisCode::REL_HWHEEL,
        RelativeAxisCode::REL_WHEEL_HI_RES,
        RelativeAxisCode::REL_HWHEEL_HI_RES,
    ] {
        wheels.insert(a);
    }

    let w = (desktop.width().max(1)) - 1;
    let h = (desktop.height().max(1)) - 1;
    let ax = UinputAbsSetup::new(AbsoluteAxisCode::ABS_X, AbsInfo::new(0, 0, w, 0, 0, 0));
    let ay = UinputAbsSetup::new(AbsoluteAxisCode::ABS_Y, AbsInfo::new(0, 0, h, 0, 0, 0));

    VirtualDeviceBuilder::new()?
        .name(VIRTUAL_NAME)
        .input_id(InputId::new(BusType::BUS_VIRTUAL, 0x4c42, 0x4d55, 1))
        .with_keys(&keys)?
        .with_relative_axes(&wheels)?
        .with_absolute_axis(&ax)?
        .with_absolute_axis(&ay)?
        .build()
}

/// A full-range virtual keyboard for the keyboard usages of grabbed mice.
/// Declaring (almost) every KEY_* code up front means the device never has to
/// be rebuilt to match a given mouse's capabilities. EV_REP is deliberately
/// absent: key repeat belongs to the compositor/xkb, as with a real keyboard.
fn build_virtual_keyboard() -> std::io::Result<VirtualDevice> {
    let mut keys = AttributeSet::<KeyCode>::new();
    // 0x2ff = KEY_MAX; skip the mouse/joystick button blocks routed to the pointer.
    for code in 1..=0x2ffu16 {
        if BTN_RANGE.contains(&code) || BTN_TRIGGER_HAPPY_RANGE.contains(&code) {
            continue;
        }
        keys.insert(KeyCode::new(code));
    }

    VirtualDeviceBuilder::new()?
        .name(VIRTUAL_KBD_NAME)
        .input_id(InputId::new(BusType::BUS_VIRTUAL, 0x4c42, 0x4d56, 1))
        .with_keys(&keys)?
        .build()
}

/// The union of the layout's main zones — the compositor's logical pixel space
/// (kscreen coordinates), so the ABS mapping and the crossing geometry agree.
/// Arm-time variant: routing has not started, a blocking lock is fine here.
fn desktop_bounds_blocking(shared: &Shared) -> Rect<i32> {
    let engine = shared.engine.lock().unwrap_or_else(|p| p.into_inner());
    bounds_of(&engine)
}

/// Pump-side variant — routing-thread rule: never block. On contention (an IPC
/// Load swapping the layout under the lock) returns None and the caller keeps
/// its cached bounds for one cycle.
fn try_desktop_bounds(shared: &Shared) -> Option<Rect<i32>> {
    let engine = match shared.engine.try_lock() {
        Ok(g) => g,
        Err(std::sync::TryLockError::Poisoned(p)) => p.into_inner(),
        Err(std::sync::TryLockError::WouldBlock) => return None,
    };
    Some(bounds_of(&engine))
}

fn bounds_of(engine: &crate::engine::MouseEngine) -> Rect<i32> {
    let mut it = engine
        .layout
        .main_zones
        .iter()
        .map(|&id| engine.layout.arena[id].pixels_bounds());
    let Some(first) = it.next() else {
        return Rect::new(0, 0, 1920, 1080);
    };
    let (mut l, mut t, mut r, mut b) = (first.left(), first.top(), first.right(), first.bottom());
    for z in it {
        l = l.min(z.left());
        t = t.min(z.top());
        r = r.max(z.right());
        b = b.max(z.bottom());
    }
    Rect::new(l, t, r - l, b - t)
}

/// Ask KWin for the real cursor position (logical coordinates, the same space
/// as the zones) through its scripting API — the only channel an ordinary
/// process has under Wayland, where no global pointer query exists. A one-shot
/// script reports `workspace.cursorPos` back over DBus (as a string: KWin
/// marshals JS numbers as doubles, which would not match an integer signature)
/// and is unloaded again. Returns None on any failure (no session bus, not
/// KWin, timeout): the caller falls back to a neutral position.
pub fn kwin_cursor_pos() -> Option<Point<i32>> {
    use tokio::sync::mpsc;

    struct Probe {
        tx: mpsc::Sender<(i32, i32)>,
    }

    #[zbus::interface(name = "org.littlebigmouse.CursorProbe")]
    impl Probe {
        fn report(&self, pos: String) {
            if let Some((x, y)) = pos.split_once(',') {
                if let (Ok(x), Ok(y)) = (x.trim().parse(), y.trim().parse()) {
                    let _ = self.tx.try_send((x, y));
                }
            }
        }
    }

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .ok()?;
    rt.block_on(async {
        let (tx, mut rx) = mpsc::channel(1);
        let service = format!("org.littlebigmouse.CursorProbe{}", std::process::id());
        let conn = zbus::connection::Builder::session()
            .ok()?
            .name(service.as_str())
            .ok()?
            .serve_at("/", Probe { tx })
            .ok()?
            .build()
            .await
            .ok()?;

        let plugin = format!("lbm-cursor-probe-{}", std::process::id());
        let script_path = std::env::temp_dir().join(format!("{plugin}.js"));
        std::fs::write(
            &script_path,
            format!(
                // "Report": the zbus interface macro exposes rust methods under
                // their PascalCase DBus names; callDBus swallows NoSuchMethod.
                "callDBus(\"{service}\", \"/\", \"org.littlebigmouse.CursorProbe\", \"Report\", \
                 workspace.cursorPos.x + \",\" + workspace.cursorPos.y);\n"
            ),
        )
        .ok()?;

        let scripting = zbus::Proxy::new(
            &conn,
            "org.kde.KWin",
            "/Scripting",
            "org.kde.kwin.Scripting",
        )
        .await
        .ok()?;
        // A probe left over by a crashed run would make loadScript return -1.
        let _ = scripting
            .call_method("unloadScript", &(plugin.as_str(),))
            .await;

        let id: i32 = scripting
            .call(
                "loadScript",
                &(script_path.to_string_lossy().as_ref(), plugin.as_str()),
            )
            .await
            .unwrap_or(-1);

        let result = if id < 0 {
            None
        } else {
            match zbus::Proxy::new(
                &conn,
                "org.kde.KWin",
                format!("/Scripting/Script{id}"),
                "org.kde.kwin.Script",
            )
            .await
            {
                Ok(script) if script.call::<_, _, ()>("run", &()).await.is_ok() => {
                    tokio::time::timeout(Duration::from_millis(700), rx.recv())
                        .await
                        .ok()
                        .flatten()
                        .map(|(x, y)| Point::new(x, y))
                }
                _ => None,
            }
        };

        let _ = scripting
            .call_method("unloadScript", &(plugin.as_str(),))
            .await;
        let _ = std::fs::remove_file(&script_path);
        result
    })
}

/// Centre of the first main zone, a guaranteed on-screen start point.
fn first_zone_center(shared: &Shared) -> Option<Point<i32>> {
    let engine = shared.engine.lock().unwrap_or_else(|p| p.into_inner());
    let id = *engine.layout.main_zones.first()?;
    let b = engine.layout.arena[id].pixels_bounds();
    Some(Point::new(
        b.left() + b.width() / 2,
        b.top() + b.height() / 2,
    ))
}

/// CursorEnv over the authoritative virtual position. `set_mouse_location` is a
/// pure state update (the caller emits the absolute point afterwards); the clip
/// is the emulated Win32 ClipCursor the engine's travel path relies on.
///
/// Public (with private fields) for the same reason as [`PumpBuffers`]: the
/// classification benchmark tracks buttons and modifiers through the real
/// cursor rather than a stand-in.
pub struct EvdevCursor {
    virtual_pos: Point<i32>,
    clip: Option<Rect<i32>>,
    desktop: Rect<i32>,
    started: Instant,
    /// Modifier state fed by the observed keyboards and the grabbed combined
    /// nodes; left/right tracked apart so releasing one keeps the other held.
    ctrl_left: bool,
    ctrl_right: bool,
    /// Bitmask of held [`BTN_MOUSE_RANGE`] buttons, one bit per code. Tracked
    /// from the grabbed devices' own stream, so it costs nothing to read.
    buttons: u8,
}

impl EvdevCursor {
    /// A cursor sitting at `start` on `desktop`, nothing held. The grabbed
    /// devices' real button state is seeded by the caller (`EVIOCGKEY` at arm
    /// time — a press that predates the grab never reached the pump).
    pub fn new(desktop: Rect<i32>, start: Point<i32>) -> EvdevCursor {
        EvdevCursor {
            virtual_pos: start,
            clip: None,
            desktop,
            started: Instant::now(),
            ctrl_left: false,
            ctrl_right: false,
            buttons: 0,
        }
    }

    fn track_ctrl(&mut self, code: u16, value: i32) {
        // value: 1 press, 2 autorepeat, 0 release.
        if code == KeyCode::KEY_LEFTCTRL.0 {
            self.ctrl_left = value != 0;
        } else if code == KeyCode::KEY_RIGHTCTRL.0 {
            self.ctrl_right = value != 0;
        }
    }

    /// The [`BTN_MOUSE_RANGE`] codes currently held, lowest code first.
    fn held_buttons(&self) -> impl Iterator<Item = u16> + '_ {
        let start = *BTN_MOUSE_RANGE.start();
        (0..=(BTN_MOUSE_RANGE.end() - BTN_MOUSE_RANGE.start()) as u8)
            .filter(move |i| self.buttons & (1u8 << i) != 0)
            .map(move |i| start + i as u16)
    }

    /// The frame the compositor is still owed when the virtual pointer goes
    /// away: one release per button left held. Empty in the ordinary case, which
    /// is why `Drop` can emit it unconditionally.
    fn release_frame(&self) -> Vec<InputEvent> {
        self.held_buttons()
            .map(|code| InputEvent::new(EventType::KEY.0, code, 0))
            .collect()
    }

    fn track_button(&mut self, code: u16, value: i32) {
        if !BTN_MOUSE_RANGE.contains(&code) {
            return;
        }
        let bit = 1u8 << (code - BTN_MOUSE_RANGE.start());
        if value != 0 {
            self.buttons |= bit;
        } else {
            self.buttons &= !bit;
        }
    }

    fn clamp(&self, p: Point<i32>) -> Point<i32> {
        let r = self.clip.unwrap_or(self.desktop);
        Point::new(
            p.x().clamp(r.left(), r.right() - 1),
            p.y().clamp(r.top(), r.bottom() - 1),
        )
    }
}

impl CursorEnv for EvdevCursor {
    fn get_mouse_location(&self) -> Point<i32> {
        self.clamp(self.virtual_pos)
    }

    fn set_mouse_location(&mut self, location: Point<i32>) {
        self.virtual_pos = location;
    }

    fn get_clip(&self) -> Rect<i32> {
        self.clip.unwrap_or(self.desktop)
    }

    fn set_clip(&mut self, r: Rect<i32>) {
        if r.is_empty() || r == self.desktop {
            self.clip = None;
            return;
        }
        self.clip = Some(r);
        self.virtual_pos = self.clamp(self.virtual_pos);
    }

    fn ctrl_down(&self) -> bool {
        self.ctrl_left || self.ctrl_right
    }

    fn buttons_down(&self) -> bool {
        self.buttons != 0
    }

    fn cursor_hidden(&self) -> bool {
        false
    }

    fn clip_is_subrect_of_virtual_screen(&self) -> bool {
        false
    }

    fn tick_count(&self) -> u64 {
        self.started.elapsed().as_millis() as u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cursor() -> EvdevCursor {
        EvdevCursor::new(Rect::new(0, 0, 1920, 1080), Point::new(0, 0))
    }

    // --- event constructors, in the shape the kernel delivers them ------------

    fn rel(axis: RelativeAxisCode, value: i32) -> InputEvent {
        InputEvent::new(EventType::RELATIVE.0, axis.0, value)
    }

    fn key(code: u16, value: i32) -> InputEvent {
        InputEvent::new(EventType::KEY.0, code, value)
    }

    fn misc(value: i32) -> InputEvent {
        InputEvent::new(EventType::MISC.0, 4 /* MSC_SCAN */, value)
    }

    fn syn() -> InputEvent {
        InputEvent::new(EventType::SYNCHRONIZATION.0, 0 /* SYN_REPORT */, 0)
    }

    /// `(type, code, value)` of each event — what a uinput frame really is, and
    /// comparable with `assert_eq!` (`InputEvent`'s own equality includes the
    /// timestamp).
    fn shape(events: &[InputEvent]) -> Vec<(u16, u16, i32)> {
        events
            .iter()
            .map(|e| (e.event_type().0, e.code(), e.value()))
            .collect()
    }

    /// Feed events that do not close a frame — asserting they don't, which is
    /// half of what every routing test below is checking.
    fn feed(bufs: &mut PumpBuffers, env: &mut EvdevCursor, events: &[InputEvent]) {
        for &ev in events {
            assert_eq!(
                bufs.push(ev, env, 0),
                Frame::Pending,
                "only SYN_REPORT closes a frame"
            );
        }
    }

    /// Feed a whole report, whose trailing SYN_REPORT must be what closes it.
    fn feed_frame(bufs: &mut PumpBuffers, env: &mut EvdevCursor, events: &[InputEvent]) {
        let (syn, rest) = events.split_last().expect("a report ends with SYN_REPORT");
        feed(bufs, env, rest);
        assert_eq!(bufs.push(*syn, env, 0), Frame::Complete);
    }

    // --- frame accumulation ---------------------------------------------------

    /// The two axes of one report are one motion, and only SYN_REPORT closes it.
    #[test]
    fn a_motion_frame_accumulates_until_syn() {
        let (mut bufs, mut env) = (PumpBuffers::new(), cursor());

        feed(
            &mut bufs,
            &mut env,
            &[
                rel(RelativeAxisCode::REL_X, 3),
                rel(RelativeAxisCode::REL_Y, -2),
            ],
        );

        assert_eq!(bufs.acc, (3, -2));
        assert!(bufs.frame_pending());
        assert_eq!(bufs.push(syn(), &mut env, 0), Frame::Complete);
    }

    /// A report split across two reads — the kernel ring buffer filled up, or the
    /// device stopped mid-frame. The deltas must survive the cycle boundary and
    /// add up, not be flushed as two half-motions or dropped.
    #[test]
    fn a_partial_frame_survives_the_cycle_boundary() {
        let (mut bufs, mut env) = (PumpBuffers::new(), cursor());

        feed(
            &mut bufs,
            &mut env,
            &[
                rel(RelativeAxisCode::REL_X, 5),
                rel(RelativeAxisCode::REL_WHEEL, 1),
            ],
        );
        // ... pump returns, polls again, the rest of the report arrives ...
        feed_frame(
            &mut bufs,
            &mut env,
            &[rel(RelativeAxisCode::REL_Y, 7), syn()],
        );

        assert_eq!(bufs.acc, (5, 7));
        assert_eq!(
            shape(&bufs.passthrough),
            shape(&[rel(RelativeAxisCode::REL_WHEEL, 1)]),
            "the wheel of the first half must still ride this frame"
        );
    }

    /// Nothing pending, nothing to emit: what lets the pump call `flush_frame`
    /// unconditionally at the end of a cycle.
    #[test]
    fn an_empty_frame_is_not_pending() {
        let (mut bufs, mut env) = (PumpBuffers::new(), cursor());
        assert!(!bufs.frame_pending());
        assert_eq!(bufs.push(syn(), &mut env, 0), Frame::Complete);
        assert!(!bufs.frame_pending(), "a bare SYN carries nothing");
    }

    // --- routing --------------------------------------------------------------

    /// Wheels are relative axes the engine has no opinion about: they ride the
    /// pointer frame verbatim, in arrival order.
    #[test]
    fn wheels_ride_the_pointer_frame() {
        let (mut bufs, mut env) = (PumpBuffers::new(), cursor());

        feed(
            &mut bufs,
            &mut env,
            &[
                rel(RelativeAxisCode::REL_WHEEL, 1),
                rel(RelativeAxisCode::REL_WHEEL_HI_RES, 120),
            ],
        );

        assert_eq!(
            shape(&bufs.passthrough),
            shape(&[
                rel(RelativeAxisCode::REL_WHEEL, 1),
                rel(RelativeAxisCode::REL_WHEEL_HI_RES, 120),
            ])
        );
        assert!(bufs.kbd.is_empty());
    }

    /// Mouse buttons stay on the pointer AND update the held mask the drag
    /// detection and the teardown release read.
    #[test]
    fn buttons_stay_on_the_pointer_and_update_the_mask() {
        let (mut bufs, mut env) = (PumpBuffers::new(), cursor());

        feed(&mut bufs, &mut env, &[key(KeyCode::BTN_LEFT.0, 1)]);

        assert_eq!(
            shape(&bufs.passthrough),
            shape(&[key(KeyCode::BTN_LEFT.0, 1)])
        );
        assert!(bufs.kbd.is_empty());
        assert!(env.buttons_down());
    }

    /// The BTN_TRIGGER_HAPPY block is buttons, not keyboard usages: routing them
    /// to the virtual keyboard would emit codes the pointer never declared.
    #[test]
    fn trigger_happy_buttons_are_pointer_buttons() {
        let (mut bufs, mut env) = (PumpBuffers::new(), cursor());

        feed(&mut bufs, &mut env, &[key(0x2c0, 1)]);

        assert_eq!(bufs.passthrough.len(), 1);
        assert!(bufs.kbd.is_empty());
    }

    /// The reason the virtual keyboard exists: a combined receiver node emits
    /// KEY_* codes the ABS pointer does not declare, and the kernel drops
    /// undeclared codes silently.
    #[test]
    fn keyboard_usages_leave_the_pointer_frame() {
        let (mut bufs, mut env) = (PumpBuffers::new(), cursor());

        feed(&mut bufs, &mut env, &[key(KeyCode::KEY_LEFTCTRL.0, 1)]);

        assert!(bufs.passthrough.is_empty());
        assert_eq!(shape(&bufs.kbd), shape(&[key(KeyCode::KEY_LEFTCTRL.0, 1)]));
        assert!(env.ctrl_down(), "the modifier feeds the ctrl-override");
    }

    /// One frame off a combined kbd+mouse node: motion accumulates, buttons and
    /// scancodes queue for the pointer, key usages queue for the keyboard — each
    /// buffer keeping arrival order, and neither leaking into the other.
    #[test]
    fn a_combined_device_splits_one_frame_in_two() {
        let (mut bufs, mut env) = (PumpBuffers::new(), cursor());

        feed_frame(
            &mut bufs,
            &mut env,
            &[
                misc(0x90001),
                key(KeyCode::KEY_A.0, 1),
                rel(RelativeAxisCode::REL_X, 4),
                key(KeyCode::BTN_RIGHT.0, 1),
                rel(RelativeAxisCode::REL_Y, -1),
                key(KeyCode::KEY_A.0, 0),
                rel(RelativeAxisCode::REL_HWHEEL, -1),
                syn(),
            ],
        );

        assert_eq!(bufs.acc, (4, -1));
        assert_eq!(
            shape(&bufs.passthrough),
            shape(&[
                misc(0x90001),
                key(KeyCode::BTN_RIGHT.0, 1),
                rel(RelativeAxisCode::REL_HWHEEL, -1),
            ])
        );
        assert_eq!(
            shape(&bufs.kbd),
            shape(&[key(KeyCode::KEY_A.0, 1), key(KeyCode::KEY_A.0, 0)])
        );
        assert!(env.buttons_down());
    }

    /// LEDs, sounds, force feedback and the rest: read off the grabbed device,
    /// deliberately not re-emitted.
    #[test]
    fn unrouted_event_types_are_dropped() {
        let (mut bufs, mut env) = (PumpBuffers::new(), cursor());

        feed(
            &mut bufs,
            &mut env,
            &[InputEvent::new(EventType::LED.0, 0, 1)],
        );

        assert!(!bufs.frame_pending());
    }

    // --- frame composition ----------------------------------------------------

    /// The uinput frame the compositor sees: absolute point first (it is the
    /// position, not a delta), then the buttons/wheels of the same report.
    /// uinput appends the SYN_REPORT itself.
    #[test]
    fn the_pointer_frame_puts_the_absolute_point_first() {
        let (mut bufs, mut env) = (PumpBuffers::new(), cursor());
        feed(
            &mut bufs,
            &mut env,
            &[
                key(KeyCode::BTN_LEFT.0, 1),
                rel(RelativeAxisCode::REL_WHEEL, 1),
            ],
        );

        let frame = shape(bufs.pointer_frame(100, 200));

        assert_eq!(
            frame,
            shape(&[
                InputEvent::new(EventType::ABSOLUTE.0, AbsoluteAxisCode::ABS_X.0, 100),
                InputEvent::new(EventType::ABSOLUTE.0, AbsoluteAxisCode::ABS_Y.0, 200),
                key(KeyCode::BTN_LEFT.0, 1),
                rel(RelativeAxisCode::REL_WHEEL, 1),
            ])
        );
        assert!(
            bufs.passthrough.is_empty(),
            "composing the frame hands those events over"
        );
    }

    /// The overwhelmingly common frame: a move and nothing else.
    #[test]
    fn an_idle_frame_is_just_the_absolute_point() {
        let mut bufs = PumpBuffers::new();
        assert_eq!(bufs.pointer_frame(1, 2).len(), 2);
    }

    // --- poll set and device errors -------------------------------------------

    /// Slot order IS the mapping back to a device list: mice first, then the
    /// observed keyboards. Getting it wrong would drain a keyboard as a mouse.
    #[test]
    fn the_poll_set_lists_mice_before_keyboards() {
        let mut bufs = PumpBuffers::new();

        bufs.fill_poll_set([7, 8].into_iter().chain([9]));

        assert_eq!(
            bufs.fds.iter().map(|p| p.fd).collect::<Vec<_>>(),
            vec![7, 8, 9]
        );
        assert!(bufs.fds.iter().all(|p| p.events == libc::POLLIN));
        assert!(bufs.fds.iter().all(|p| p.revents == 0));
    }

    /// Refilling is a reset, not an append: `poll` writes `revents` in place, and
    /// a hot-plug changes the list under us.
    #[test]
    fn refilling_the_poll_set_forgets_the_previous_one() {
        let mut bufs = PumpBuffers::new();
        bufs.fill_poll_set([7, 8].into_iter());
        bufs.fds[0].revents = libc::POLLIN;

        bufs.fill_poll_set([9].into_iter());

        assert_eq!(bufs.fds.len(), 1);
        assert_eq!(bufs.fds[0].fd, 9);
        assert_eq!(bufs.fds[0].revents, 0);
    }

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

    // --- allocation behaviour -------------------------------------------------

    /// The point of owning the buffers: a steady stream of reports must stop
    /// touching the allocator. Capacity is the observable proxy — a `Vec` only
    /// allocates when it grows — and it must not move once the first frames have
    /// sized it.
    #[test]
    fn a_steady_stream_stops_allocating() {
        let (mut bufs, mut env) = (PumpBuffers::new(), cursor());
        let frame = [
            rel(RelativeAxisCode::REL_X, 2),
            rel(RelativeAxisCode::REL_Y, -1),
            key(KeyCode::BTN_LEFT.0, 1),
            key(KeyCode::KEY_A.0, 1),
            syn(),
        ];
        // The shape of one pump cycle, minus the devices and the uinput writes.
        let cycle = |bufs: &mut PumpBuffers, env: &mut EvdevCursor| {
            bufs.fill_poll_set([3, 4, 5].into_iter());
            bufs.events.clear();
            bufs.events.extend(frame);
            for k in 0..bufs.events.len() {
                let ev = bufs.events[k];
                if bufs.push(ev, env, 0) == Frame::Complete {
                    bufs.acc = (0, 0);
                    bufs.pointer_frame(0, 0);
                    bufs.kbd.clear();
                }
            }
        };

        cycle(&mut bufs, &mut env);
        let settled = (
            bufs.fds.capacity(),
            bufs.events.capacity(),
            bufs.passthrough.capacity(),
            bufs.kbd.capacity(),
            bufs.batch.capacity(),
        );
        for _ in 0..10_000 {
            cycle(&mut bufs, &mut env);
        }

        assert_eq!(
            settled,
            (
                bufs.fds.capacity(),
                bufs.events.capacity(),
                bufs.passthrough.capacity(),
                bufs.kbd.capacity(),
                bufs.batch.capacity(),
            ),
            "a settled pump must not grow a buffer again"
        );
    }

    #[test]
    fn nothing_held_owes_no_release() {
        assert_eq!(cursor().held_buttons().count(), 0);
    }

    #[test]
    fn a_held_button_is_owed_a_release() {
        let mut c = cursor();
        c.track_button(KeyCode::BTN_LEFT.0, 1);
        assert_eq!(
            c.held_buttons().collect::<Vec<_>>(),
            vec![KeyCode::BTN_LEFT.0]
        );
    }

    #[test]
    fn releasing_clears_the_debt() {
        let mut c = cursor();
        c.track_button(KeyCode::BTN_LEFT.0, 1);
        c.track_button(KeyCode::BTN_LEFT.0, 0);
        assert_eq!(c.held_buttons().count(), 0);
    }

    /// The regression this module's `Drop` exists for: dropping the router
    /// mid-press owes exactly the buttons still down, and only those.
    #[test]
    fn only_the_still_held_buttons_are_owed() {
        let mut c = cursor();
        c.track_button(KeyCode::BTN_LEFT.0, 1);
        c.track_button(KeyCode::BTN_RIGHT.0, 1);
        c.track_button(KeyCode::BTN_LEFT.0, 0);
        assert_eq!(
            c.held_buttons().collect::<Vec<_>>(),
            vec![KeyCode::BTN_RIGHT.0]
        );
    }

    /// What `Drop` actually hands to uinput: EV_KEY, the held code, value 0.
    /// A press (or a wrong event type) here would leave the seat exactly as
    /// stuck as emitting nothing.
    #[test]
    fn the_owed_frame_is_a_key_release() {
        let mut c = cursor();
        c.track_button(KeyCode::BTN_LEFT.0, 1);

        let frame = c.release_frame();

        assert_eq!(frame.len(), 1);
        assert_eq!(frame[0].event_type(), EventType::KEY);
        assert_eq!(frame[0].code(), KeyCode::BTN_LEFT.0);
        assert_eq!(frame[0].value(), 0, "a press would not unstick the button");
    }

    /// The ordinary teardown: nothing held, nothing emitted. `Drop` skips the
    /// uinput write entirely on an empty frame.
    #[test]
    fn an_idle_teardown_owes_an_empty_frame() {
        assert!(cursor().release_frame().is_empty());
    }

    /// Autorepeat (value 2) is a press, not a second one: the mask must not
    /// double-count it, and the button must still be owed a single release.
    #[test]
    fn autorepeat_still_counts_as_held() {
        let mut c = cursor();
        c.track_button(KeyCode::BTN_LEFT.0, 1);
        c.track_button(KeyCode::BTN_LEFT.0, 2);
        assert_eq!(
            c.held_buttons().collect::<Vec<_>>(),
            vec![KeyCode::BTN_LEFT.0]
        );
    }

    /// Every code the virtual pointer declares must round-trip through the
    /// bitmask: BTN_TASK is bit 7, so a narrower mask would silently drop it.
    #[test]
    fn the_whole_declared_range_round_trips() {
        for code in BTN_MOUSE_RANGE {
            let mut c = cursor();
            c.track_button(code, 1);
            assert_eq!(
                c.held_buttons().collect::<Vec<_>>(),
                vec![code],
                "code {code:#x} did not round-trip"
            );
        }
    }

    /// Codes outside the pointer's declared set (tablet/touch, joystick) must
    /// not touch the mask — shifting by their offset would overflow the u8.
    #[test]
    fn codes_outside_the_range_are_ignored() {
        let mut c = cursor();
        c.track_button(KeyCode::BTN_TOOL_PEN.0, 1);
        c.track_button(KeyCode::BTN_TOUCH.0, 1);
        c.track_button(KeyCode::BTN_0.0, 1);
        assert_eq!(c.held_buttons().count(), 0);
    }
}
