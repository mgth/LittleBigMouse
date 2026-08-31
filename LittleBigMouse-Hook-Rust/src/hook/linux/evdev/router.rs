//! The pump: the loop that owns the grabbed devices and turns their reports into
//! virtual-pointer motion, and the reconcile loop that arms and disarms it.
//!
//! # RULE — nothing potentially blocking here
//!
//! From the first `EVIOCGRAB` the physical mice deliver ONLY to this process:
//! any stall in [`Router::pump`] freezes the user's pointer system-wide. The
//! full rule, its rationale and its audit are in the module documentation one
//! level up; in this file it reads as: everything expensive happens in
//! [`Router::arm`], before the first grab, and the cycle itself only polls,
//! drains, routes and writes.

use std::os::fd::AsRawFd;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};

use evdev::{uinput::VirtualDevice, EventType};

use crate::engine::cursor::CursorEnv;
use crate::engine::event::MouseEventArg;
use crate::geometry::Point;
use crate::hook::hot_path::{count_event, route_move, Routed};
use crate::hook::linux::accel::{AccelConfig, PointerAccel};
use crate::ipc::protocol;
use crate::shared::Shared;

use super::cursor::EvdevCursor;
use super::devices::{
    self, held_buttons_of, purge_dead, slot_is_dead, Gone, Mouse, Node, ScanResult,
};
use super::frame::{Frame, PumpBuffers};
use super::probe::{
    desktop_bounds_blocking, first_zone_center, kwin_cursor_pos, try_desktop_bounds,
};
use super::uinput::{build_virtual, build_virtual_keyboard};

/// The bounded wait of one cycle, and the cadence at which an idle pump
/// reconsiders `want_hook`.
const POLL_TIMEOUT_MS: i32 = 100;

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

/// The grabbed devices, the virtual pointer + keyboard, and the authoritative cursor.
struct Router {
    /// Each grabbed mouse carries its own acceleration state: velocities must
    /// not mix across devices, and kcminputrc settings are per-device.
    devices: Vec<Mouse>,
    /// Observed (non-grabbed) keyboards feeding the ctrl-override state.
    keyboards: Vec<Node>,
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
    accel_cfg: AccelConfig,
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
        let accel_cfg = AccelConfig::load();
        let scan = devices::enumerate();
        let mut keyboards = Vec::new();
        for (path, mut dev) in scan.keyboards {
            if dev.set_nonblocking(true).is_ok() {
                keyboards.push((path, dev));
            }
        }
        let virt = build_virtual(desktop)?;
        let virt_kbd = build_virtual_keyboard()?;

        let mut devices = Vec::new();
        for (path, mut dev) in scan.mice {
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
                        PointerAccel::new(settings.profile, settings.speed),
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
        // drains its channel.
        let (scan_rx, scan_stop) = devices::spawn_scanner();

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
        if self.bufs.slots() == 0 {
            // Every device vanished: keep the cadence (poll(0 fds) returns
            // immediately) and let the rescan pick devices back up.
            std::thread::sleep(Duration::from_millis(POLL_TIMEOUT_MS as u64));
            return;
        }
        self.bufs.poll(POLL_TIMEOUT_MS);

        // Indexed rather than iterated: processing an event needs `&mut self`,
        // which an iterator borrowed from `self.bufs` would hold hostage. Both
        // `pollfd` and `InputEvent` are `Copy`, so a slot is read out and the
        // borrow ends there.
        for slot in 0..self.bufs.slots() {
            let revents = self.bufs.revents(slot);
            if slot_is_dead(revents) {
                self.bufs.mark_dead(slot);
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
                        bufs.mark_dead(slot);
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

        if !self.bufs.dead_slots().is_empty() {
            let Router {
                bufs,
                devices,
                keyboards,
                ..
            } = self;
            purge_dead(bufs.dead_slots(), n_mice, devices, keyboards, |gone| {
                let (path, kind) = match &gone {
                    Gone::Mouse((path, _, _)) => (path, "device"),
                    Gone::Keyboard((path, _)) => (path, "keyboard"),
                };
                eprintln!("[LittleBigMouse.Hook] evdev: {kind} gone {path:?}");
            });
            bufs.clear_dead();
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
                            PointerAccel::new(settings.profile, settings.speed),
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
                .last_motion_dev()
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

            // The evdev variants around the shared route (see the table in
            // `hot_path`): the cursor is authoritative, so someone must always
            // commit a position — the engine's verdict on a crossing, our own
            // clamp otherwise. A contended route therefore cannot just pass: it
            // commits the clamped candidate and emits right here (and that frame
            // has never been counted as an event — the early return skips the
            // pending keyboard frame too, exactly as before).
            let routed = route_move(&shared.engine, &mut self.env, candidate);
            match routed {
                Routed::Contended => {
                    self.env.virtual_pos = self.env.clamp(candidate);
                    self.emit_absolute();
                    return;
                }
                Routed::Passed => {
                    self.env.virtual_pos = self.env.clamp(candidate);
                    count_event();
                }
                Routed::Crossed => count_event(),
            }
            if self.debug {
                // Per-frame trace: raw delta, engine input, emitted position. The
                // ground truth for any "the cursor was seen somewhere we never
                // sent it" investigation (compare against what KWin displays).
                eprintln!("[LittleBigMouse.Hook] evdev: frame d=({dx},{dy}) cand=({},{}) -> emit ({},{}){}{}",
                    candidate.x(), candidate.y(), self.env.virtual_pos.x(), self.env.virtual_pos.y(),
                    if routed.handled() { " CROSS" } else { "" },
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
