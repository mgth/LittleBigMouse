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
//! Ctrl-override reads the modifier from keyboards WITHOUT grabbing them (evdev
//! nodes are multi-reader; the compositor keeps them), plus the ctrl usages of
//! the grabbed combined nodes. Hot-plug is handled by a periodic rescan (new
//! mice would otherwise drive the cursor directly, next to the engine) and by
//! purging dead nodes — a removed device reports POLLERR forever and would
//! otherwise turn the pump into a busy loop. Not mapped: focus-based
//! exclusion — reported inert, same as the other Linux backends.

use std::os::fd::AsRawFd;
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
                    .map(|a| a.contains(RelativeAxisCode::REL_X) && a.contains(RelativeAxisCode::REL_Y))
                    .unwrap_or(false)
                && d.supported_keys().map(|k| k.contains(KeyCode::BTN_LEFT)).unwrap_or(false)
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
            && d.supported_keys().map(|k| k.contains(KeyCode::BTN_LEFT)).unwrap_or(false)
    };
    evdev::enumerate()
        .filter(|(_, d)| {
            d.name().map(|n| !n.contains("LittleBigMouse virtual")).unwrap_or(true)
                && d.supported_keys().map(|k| k.contains(KeyCode::KEY_LEFTCTRL)).unwrap_or(false)
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
    let sens = std::env::var("LBM_EVDEV_SENS").ok().and_then(|s| s.parse::<f64>().ok()).unwrap_or(1.0);

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

/// The grabbed devices, the virtual pointer + keyboard, and the authoritative cursor.
struct Router {
    devices: Vec<(std::path::PathBuf, Device)>,
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
}

impl Router {
    fn arm(shared: &Shared, sens: f64, debug: bool, resume_at: Option<Point<i32>>) -> std::io::Result<Router> {
        let desktop = desktop_bounds_blocking(shared);

        // Everything slow happens BEFORE the first grab: from EVIOCGRAB on, the
        // user's mice are captured but not routed yet, so this window must stay
        // minimal (see the module's routing-thread rule). The compositor probe
        // (DBus, ~700ms worst case) and the enumerations (~10ms per /dev/input
        // node) therefore run first; the cursor may drift a few px between the
        // probe and the grab, which is harmless for a start point.
        let probed = kwin_cursor_pos();
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
                    eprintln!("[LittleBigMouse.Hook] evdev: grabbed {} ({:?})",
                        dev.name().unwrap_or("?"), path);
                    devices.push((path, dev));
                }
                Err(e) => eprintln!("[LittleBigMouse.Hook] evdev: cannot grab {path:?}: {e}"),
            }
        }
        if devices.is_empty() {
            return Err(std::io::Error::new(std::io::ErrorKind::NotFound, "no grabbable mouse"));
        }
        eprintln!("[LittleBigMouse.Hook] evdev: observing {} keyboard(s) for ctrl-override",
            keyboards.len());

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
                        Point::new(desktop.left() + desktop.width() / 2, desktop.top() + desktop.height() / 2)
                    }),
                    "fallback",
                ),
            },
        };
        let start = Point::new(
            start.x().clamp(desktop.left(), desktop.left() + desktop.width() - 1),
            start.y().clamp(desktop.top(), desktop.top() + desktop.height() - 1),
        );
        eprintln!("[LittleBigMouse.Hook] evdev: starting at ({},{}) ({origin})", start.x(), start.y());

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
                let scan = ScanResult { mice: enumerate_mice(), keyboards: enumerate_keyboards() };
                if scan_tx.send(scan).is_err() {
                    return;
                }
            });
        }

        let mut router = Router {
            devices,
            keyboards,
            scan_rx,
            scan_stop,
            virt,
            virt_kbd,
            env: EvdevCursor {
                virtual_pos: start,
                clip: None,
                desktop,
                started: Instant::now(),
                ctrl_left: false,
                ctrl_right: false,
            },
            sens,
            debug,
            rem: (0.0, 0.0),
        };

        // Place the cursor at the start point and prime the engine there.
        router.emit_absolute(&mut Vec::new());
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
                    self.emit_absolute(&mut Vec::new());
                }
            }
        }

        self.rescan();

        let n_mice = self.devices.len();
        let mut fds: Vec<libc::pollfd> = self
            .devices
            .iter()
            .map(|(_, d)| d.as_raw_fd())
            .chain(self.keyboards.iter().map(|(_, d)| d.as_raw_fd()))
            .map(|fd| libc::pollfd { fd, events: libc::POLLIN, revents: 0 })
            .collect();
        if fds.is_empty() {
            // Every device vanished: keep the cadence (poll(0 fds) returns
            // immediately) and let the rescan pick devices back up.
            std::thread::sleep(Duration::from_millis(100));
            return;
        }
        unsafe { libc::poll(fds.as_mut_ptr(), fds.len() as libc::nfds_t, 100); }

        let mut acc = (0i64, 0i64);
        let mut passthrough: Vec<InputEvent> = Vec::new();
        let mut kbd: Vec<InputEvent> = Vec::new();
        // A removed node reports POLLERR/POLLHUP forever: it must leave the set,
        // or poll() returns instantly and the pump becomes a busy loop.
        let mut dead: Vec<usize> = Vec::new();

        for (i, pfd) in fds.iter().enumerate() {
            if pfd.revents & (libc::POLLERR | libc::POLLHUP | libc::POLLNVAL) != 0 {
                dead.push(i);
                continue;
            }
            if pfd.revents & libc::POLLIN == 0 {
                continue;
            }
            if i >= n_mice {
                self.track_ctrl_from_keyboard(i - n_mice);
                continue;
            }
            let events: Vec<InputEvent> = match self.devices[i].1.fetch_events() {
                Ok(it) => it.collect(),
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => continue,
                Err(_) => {
                    // Dropping the Device closes the fd, which also releases the
                    // grab — if the node was actually alive the compositor gets
                    // it back rather than the user losing the mouse.
                    dead.push(i);
                    continue;
                }
            };
            for ev in events {
                match ev.event_type() {
                    EventType::SYNCHRONIZATION => self.flush_frame(shared, &mut acc, &mut passthrough, &mut kbd),
                    EventType::RELATIVE if ev.code() == RelativeAxisCode::REL_X.0 => acc.0 += ev.value() as i64,
                    EventType::RELATIVE if ev.code() == RelativeAxisCode::REL_Y.0 => acc.1 += ev.value() as i64,
                    // Wheels and any other relative axis: pass through verbatim.
                    EventType::RELATIVE => passthrough.push(ev),
                    // Buttons stay with the pointer; every other EV_KEY code is a
                    // keyboard usage (onboard macros on combined receiver nodes)
                    // and goes to the virtual keyboard, which declares it.
                    EventType::KEY
                        if BTN_RANGE.contains(&ev.code())
                            || BTN_TRIGGER_HAPPY_RANGE.contains(&ev.code()) =>
                        passthrough.push(ev),
                    EventType::KEY => {
                        // Combined receiver nodes carry the modifier too.
                        self.env.track_ctrl(ev.code(), ev.value());
                        kbd.push(ev);
                    }
                    // MSC_SCAN scancodes stay with the pointer frame: losing the
                    // scancode of a routed key is inconsequential.
                    EventType::MISC => passthrough.push(ev),
                    _ => {}
                }
            }
        }
        if acc != (0, 0) || !passthrough.is_empty() || !kbd.is_empty() {
            self.flush_frame(shared, &mut acc, &mut passthrough, &mut kbd);
        }

        for i in dead.into_iter().rev() {
            let (path, kind) = if i >= n_mice {
                (self.keyboards.remove(i - n_mice).0, "keyboard")
            } else {
                (self.devices.remove(i).0, "device")
            };
            eprintln!("[LittleBigMouse.Hook] evdev: {kind} gone {path:?}");
        }
    }

    /// Drain a (non-grabbed) keyboard, keeping only the ctrl state: the
    /// compositor still owns these devices, we just observe the modifier.
    fn track_ctrl_from_keyboard(&mut self, k: usize) {
        let events: Vec<InputEvent> = match self.keyboards[k].1.fetch_events() {
            Ok(it) => it.collect(),
            Err(_) => return, // dead nodes are purged via POLLERR next cycle
        };
        for ev in events {
            if ev.event_type() == EventType::KEY {
                self.env.track_ctrl(ev.code(), ev.value());
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
                if self.devices.iter().any(|(p, _)| *p == path) {
                    continue;
                }
                if dev.set_nonblocking(true).is_err() {
                    continue;
                }
                match dev.grab() {
                    Ok(()) => {
                        eprintln!("[LittleBigMouse.Hook] evdev: grabbed {} ({:?}, hot-plug)",
                            dev.name().unwrap_or("?"), path);
                        self.devices.push((path, dev));
                    }
                    // Retried every rescan; only worth the noise when debugging.
                    Err(e) if self.debug =>
                        eprintln!("[LittleBigMouse.Hook] evdev: cannot grab {path:?}: {e}"),
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
    fn flush_frame(&mut self, shared: &Shared, acc: &mut (i64, i64), passthrough: &mut Vec<InputEvent>, kbd: &mut Vec<InputEvent>) {
        if *acc == (0, 0) && passthrough.is_empty() && kbd.is_empty() {
            return;
        }

        if *acc != (0, 0) {
            let sx = acc.0 as f64 * self.sens + self.rem.0;
            let sy = acc.1 as f64 * self.sens + self.rem.1;
            let (dx, dy) = (sx.trunc() as i32, sy.trunc() as i32);
            self.rem = (sx - dx as f64, sy - dy as f64);

            let old = self.env.virtual_pos;
            // Win32 parity: the LL hook sees the UNCLIPPED proposed point even while
            // ClipCursor pins the cursor — the growing past-border distance is what
            // drains border resistance. Only the committed position gets clamped.
            let candidate = Point::new(
                old.x().saturating_add(dx),
                old.y().saturating_add(dy),
            );

            let mut engine = match shared.engine.try_lock() {
                Ok(g) => g,
                Err(std::sync::TryLockError::Poisoned(p)) => p.into_inner(),
                Err(std::sync::TryLockError::WouldBlock) => {
                    self.env.virtual_pos = self.env.clamp(candidate);
                    self.emit_absolute(passthrough);
                    *acc = (0, 0);
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
            *acc = (0, 0);
        }

        self.emit_absolute(passthrough);

        // Keyboard usages get their own atomic frame on the virtual keyboard
        // (emit appends the SYN_REPORT), mirroring the per-device framing of the
        // pointer batch above.
        if !kbd.is_empty() {
            let _ = self.virt_kbd.emit(kbd);
            kbd.clear();
        }
    }

    /// Emit the current absolute position plus any pending buttons/wheels as one
    /// atomic uinput frame. ABS values are desktop-relative (the ABS range starts
    /// at 0), so the compositor's 1:1 mapping lands the cursor exactly.
    fn emit_absolute(&mut self, passthrough: &mut Vec<InputEvent>) {
        let ax = self.env.virtual_pos.x() - self.env.desktop.left();
        let ay = self.env.virtual_pos.y() - self.env.desktop.top();
        let mut batch = vec![
            InputEvent::new(EventType::ABSOLUTE.0, AbsoluteAxisCode::ABS_X.0, ax),
            InputEvent::new(EventType::ABSOLUTE.0, AbsoluteAxisCode::ABS_Y.0, ay),
        ];
        batch.append(passthrough);
        let _ = self.virt.emit(&batch);
    }
}

impl Drop for Router {
    fn drop(&mut self) {
        self.scan_stop.store(true, Ordering::Relaxed);
        for (_, d) in &mut self.devices {
            let _ = d.ungrab();
        }
        // Keyboards were never grabbed; closing their fds is enough. The scanner
        // thread exits on the flag (or on its next send, once scan_rx is gone).
    }
}

/// An absolute virtual pointer whose ABS range equals the desktop size, plus
/// buttons and (relative) wheels. KWin maps the ABS range 1:1 onto the whole
/// desktop, so no acceleration is applied and the emitted point is the position.
fn build_virtual(desktop: Rect<i32>) -> std::io::Result<VirtualDevice> {
    let mut keys = AttributeSet::<KeyCode>::new();
    for k in [
        KeyCode::BTN_LEFT, KeyCode::BTN_RIGHT, KeyCode::BTN_MIDDLE,
        KeyCode::BTN_SIDE, KeyCode::BTN_EXTRA, KeyCode::BTN_FORWARD,
        KeyCode::BTN_BACK, KeyCode::BTN_TASK,
    ] {
        keys.insert(k);
    }

    let mut wheels = AttributeSet::<RelativeAxisCode>::new();
    for a in [
        RelativeAxisCode::REL_WHEEL, RelativeAxisCode::REL_HWHEEL,
        RelativeAxisCode::REL_WHEEL_HI_RES, RelativeAxisCode::REL_HWHEEL_HI_RES,
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
    let mut it = engine.layout.main_zones.iter().map(|&id| engine.layout.arena[id].pixels_bounds());
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

    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().ok()?;
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

        let scripting = zbus::Proxy::new(&conn, "org.kde.KWin", "/Scripting", "org.kde.kwin.Scripting")
            .await
            .ok()?;
        // A probe left over by a crashed run would make loadScript return -1.
        let _ = scripting.call_method("unloadScript", &(plugin.as_str(),)).await;

        let id: i32 = scripting
            .call("loadScript", &(script_path.to_string_lossy().as_ref(), plugin.as_str()))
            .await
            .unwrap_or(-1);

        let result = if id < 0 {
            None
        } else {
            match zbus::Proxy::new(&conn, "org.kde.KWin", format!("/Scripting/Script{id}"), "org.kde.kwin.Script").await {
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

        let _ = scripting.call_method("unloadScript", &(plugin.as_str(),)).await;
        let _ = std::fs::remove_file(&script_path);
        result
    })
}

/// Centre of the first main zone, a guaranteed on-screen start point.
fn first_zone_center(shared: &Shared) -> Option<Point<i32>> {
    let engine = shared.engine.lock().unwrap_or_else(|p| p.into_inner());
    let id = *engine.layout.main_zones.first()?;
    let b = engine.layout.arena[id].pixels_bounds();
    Some(Point::new(b.left() + b.width() / 2, b.top() + b.height() / 2))
}

/// CursorEnv over the authoritative virtual position. `set_mouse_location` is a
/// pure state update (the caller emits the absolute point afterwards); the clip
/// is the emulated Win32 ClipCursor the engine's travel path relies on.
struct EvdevCursor {
    virtual_pos: Point<i32>,
    clip: Option<Rect<i32>>,
    desktop: Rect<i32>,
    started: Instant,
    /// Modifier state fed by the observed keyboards and the grabbed combined
    /// nodes; left/right tracked apart so releasing one keeps the other held.
    ctrl_left: bool,
    ctrl_right: bool,
}

impl EvdevCursor {
    fn track_ctrl(&mut self, code: u16, value: i32) {
        // value: 1 press, 2 autorepeat, 0 release.
        if code == KeyCode::KEY_LEFTCTRL.0 {
            self.ctrl_left = value != 0;
        } else if code == KeyCode::KEY_RIGHTCTRL.0 {
            self.ctrl_right = value != 0;
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
