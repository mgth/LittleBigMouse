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
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};

use evdev::{uinput::VirtualDevice, Device, EventType, InputEvent};

use crate::engine::event::MouseEventArg;
use crate::geometry::Point;
use crate::hook::linux::accel::{AccelConfig, PointerAccel};
use crate::ipc::input::Driver;
use crate::shared::Shared;
use lbm_ipc::protocol::Event;

use super::cursor::EvdevCursor;
use super::devices::{
    self, held_buttons_of, purge_dead, retry_grabs, set_aside, slot_is_dead, Gone, Keyboard, Mouse,
    Node, ScanResult, REGRAB_EVERY,
};
use super::frame::Frame;
use super::hider::CursorHider;
use super::pipeline::{Outputs, Pipeline};
use super::probe::{
    device_bounds_blocking, first_zone_center, kwin_cursor_pos, layout_bounds_blocking,
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
                    shared.broadcast(&Event::Running);
                }
                Err(e) => {
                    eprintln!("[LittleBigMouse.Hook] evdev: arm failed: {e}");
                    shared.want_hook.store(false, Ordering::SeqCst);
                    shared.broadcast(&Event::Stopped);
                }
            }
        } else if !want && router.is_some() {
            // Where it is on this machine: a pointer on another one comes home first.
            resume_at = router.as_ref().map(|r| r.pipe.local_pos());
            drop(router.take());
            hooked_since = None;
            shared.hooked.store(false, Ordering::SeqCst);
            crate::platform::set_process_priority(crate::priority::Priority::from_u8(
                shared.priority_unhooked.load(Ordering::SeqCst),
            ));
            shared.broadcast(&Event::Stopped);
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
            // Nothing is grabbed: nothing happens here until a request says so.
            None => shared.wake.wait(),
        }

        let report_due = debug && last_report.elapsed() >= Duration::from_secs(2);
        if let Some(active) = router.as_ref().filter(|_| report_due) {
            let events = crate::hook::MOUSE_EVENTS.load(Ordering::Relaxed);
            let crossings = crate::hook::CROSSINGS.load(Ordering::Relaxed);
            if events != last_events {
                let p = active.pipe.env.virtual_pos;
                eprintln!("[LittleBigMouse.Hook] evdev: {} motion events, {crossings} crossings (pos {},{})",
                    events - last_events, p.x(), p.y());
                last_events = events;
            }
            last_report = Instant::now();
        }
    }
}

/// A freshly grabbed node becomes a routed mouse: its per-device acceleration
/// is resolved here, and the grab is logged. `how` qualifies the log line
/// (empty at arm time).
fn adopt(accel_cfg: &AccelConfig, path: PathBuf, dev: Device, how: &str) -> Mouse {
    let id = dev.input_id();
    let name = dev.name().unwrap_or("?").to_string();
    let settings = accel_cfg.for_device(id.vendor(), id.product(), &name);
    eprintln!(
        "[LittleBigMouse.Hook] evdev: grabbed {name} ({path:?}, {how}accel {:?} speed {})",
        settings.profile, settings.speed
    );
    (
        path,
        dev,
        PointerAccel::new(settings.profile, settings.speed),
    )
}

/// The grabbed devices, the virtual pointer + keyboard, and the authoritative cursor.
struct Router {
    /// Each grabbed mouse carries its own acceleration state: velocities must
    /// not mix across devices, and kcminputrc settings are per-device.
    devices: Vec<Mouse>,
    /// Observed keyboards feeding the ctrl-override state, grabbed while the pointer
    /// is on another machine.
    keyboards: Vec<Keyboard>,
    /// Mice opened but not grabbed (another program holds them), kept open so
    /// that the grab alone can be retried every [`REGRAB_EVERY`]. Never polled:
    /// their events still go to the compositor.
    pending: Vec<Node>,
    regrab_at: Instant,
    /// Hot-plug scans arriving from the scanner thread.
    scan_rx: mpsc::Receiver<ScanResult>,
    scan_stop: Arc<AtomicBool>,
    virt: VirtualDevice,
    /// Keyboard usages coming from grabbed mice (combined receiver nodes, onboard
    /// macros) are re-emitted here — the pointer device does not declare them, and
    /// mixing a full keyboard into an ABS pointer risks a libinput/KWin
    /// reclassification of the pointer.
    virt_kbd: VirtualDevice,
    /// kcminputrc + env overrides, resolved per device at grab time (arm and
    /// hot-plug rescan) — the pump itself never touches the filesystem.
    accel_cfg: AccelConfig,
    /// The cursor, the reused buffers, and what is done with a frame — on this
    /// machine or another one.
    pipe: Pipeline,
    /// The input stream's client, for the frames that go to another machine.
    link: Driver,
    /// Hides this machine's cursor while the pointer is on another one.
    hider: CursorHider,
}

/// The router's devices and input stream, as the pipeline writes to them.
struct Outs<'a> {
    virt: &'a mut VirtualDevice,
    virt_kbd: &'a mut VirtualDevice,
    keyboards: &'a mut Vec<Keyboard>,
    link: &'a mut Driver,
    hider: &'a mut CursorHider,
}

impl Outputs for Outs<'_> {
    fn pointer(&mut self, frame: &[InputEvent]) {
        let _ = self.virt.emit(frame);
    }

    fn keyboard(&mut self, frame: &[InputEvent]) {
        let _ = self.virt_kbd.emit(frame);
    }

    fn remote_client(&self) -> Option<u64> {
        self.link.client()
    }

    fn send_remote(&mut self, batch: Vec<u8>) -> bool {
        self.link.send(batch)
    }

    fn grab_keyboards(&mut self) {
        for keyboard in self.keyboards.iter_mut() {
            keyboard.grab_if_idle();
        }
    }

    fn release_keyboards(&mut self) {
        for keyboard in self.keyboards.iter_mut() {
            keyboard.release();
        }
    }

    fn hide_cursor(&mut self, from: Point<i32>) {
        self.hider.hide(from);
    }

    fn cursor_hiding_spot(&mut self) -> Option<Point<i32>> {
        self.hider.spot()
    }

    fn show_cursor(&mut self) {
        self.hider.show();
    }
}

impl Router {
    fn arm(
        shared: &Shared,
        sens: f64,
        debug: bool,
        resume_at: Option<Point<i32>>,
    ) -> std::io::Result<Router> {
        // Where the cursor may go, and the desktop the device has to span. The same
        // rectangle whenever the layout covers the desktop, which is the ordinary case.
        // The device spans this machine's desktop only: another machine's zones, in play
        // or not, are never emitted here.
        let (bounds, local) = layout_bounds_blocking(shared);
        let device = device_bounds_blocking(shared, local);

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
        for (path, dev) in scan.keyboards {
            if dev.set_nonblocking(true).is_ok() {
                keyboards.push(Keyboard::open(path, dev));
            }
        }
        let virt = build_virtual(device)?;
        let virt_kbd = build_virtual_keyboard()?;
        // Its thread, the first time; it reaches for the display server only when the
        // pointer first goes to another machine.
        let hider = CursorHider::new();

        let mut devices = Vec::new();
        let mut pending = Vec::new();
        for (path, mut dev) in scan.mice {
            dev.set_nonblocking(true)?;
            match dev.grab() {
                Ok(()) => devices.push(adopt(&accel_cfg, path, dev, "")),
                Err(e) => {
                    eprintln!("[LittleBigMouse.Hook] evdev: cannot grab {path:?}: {e}");
                    set_aside(&mut pending, path, dev);
                }
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
                            local.left() + local.width() / 2,
                            local.top() + local.height() / 2,
                        )
                    }),
                    "fallback",
                ),
            },
        };
        // Into the layout, not merely onto the desktop: a monitor excluded from the
        // layout is part of the desktop and is where the cursor is not to start. And
        // on this machine: the pointer always starts here.
        let start = Point::new(
            start
                .x()
                .clamp(local.left(), local.left() + local.width() - 1),
            start
                .y()
                .clamp(local.top(), local.top() + local.height() - 1),
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
        let mut env = EvdevCursor::new(bounds, device, start);
        env.buttons = held_buttons_of(&devices);

        let mut router = Router {
            devices,
            keyboards,
            pending,
            regrab_at: Instant::now() + REGRAB_EVERY,
            scan_rx,
            scan_stop,
            virt,
            virt_kbd,
            accel_cfg,
            pipe: Pipeline::new(env, sens, debug),
            link: Driver::default(),
            hider,
        };

        // Place the cursor at the start point and prime the engine there.
        let (pipe, mut out) = router.split();
        pipe.emit_local(&mut out);
        let mut engine = shared.engine.lock().unwrap_or_else(|p| p.into_inner());
        let mut e = MouseEventArg::new(start);
        engine.on_mouse_move(&mut router.pipe.env, &mut e);
        drop(engine);

        Ok(router)
    }

    /// The pipeline, and the devices and stream it writes to: two borrows of
    /// distinct fields, for the calls that need both.
    fn split(&mut self) -> (&mut Pipeline, Outs<'_>) {
        (
            &mut self.pipe,
            Outs {
                virt: &mut self.virt,
                virt_kbd: &mut self.virt_kbd,
                keyboards: &mut self.keyboards,
                link: &mut self.link,
                hider: &mut self.hider,
            },
        )
    }

    /// One poll cycle: drain readable devices, process each SYN frame.
    fn pump(&mut self, shared: &'static Shared) {
        // The input stream's client, the layout and the desktop it sits on can all
        // change under us: an agent connects or goes, a Load brings a new layout and
        // names its desktop. All are read without waiting — this is the routing thread,
        // and a blocking lock here is a pointer frozen system-wide (the module rule). On
        // contention nothing is updated and the next cycle asks again.
        self.link.refresh(&shared.outlet);
        let (pipe, mut out) = self.split();
        if let Some(device) = pipe.begin_cycle(shared, &mut out) {
            if device != self.pipe.env.device {
                // Rebuilt to the new span, so the compositor's mapping stays 1:1. A
                // build that fails leaves the old device and the old rectangle in
                // place, and the next cycle tries again. Only when it changed: this
                // runs every cycle, and an absolute frame per cycle is not a thing the
                // compositor should be sent.
                if let Ok(v) = build_virtual(device) {
                    self.virt = v;
                    self.pipe.env.device = device;
                    self.pipe.env.virtual_pos = self.pipe.env.clamp(self.pipe.env.virtual_pos);
                    let (pipe, mut out) = self.split();
                    pipe.emit_local(&mut out);
                }
            }
        }

        self.rescan();
        self.regrab();

        let n_mice = self.devices.len();
        {
            let Router {
                pipe,
                devices,
                keyboards,
                ..
            } = self;
            pipe.bufs.fill_poll_set(
                devices
                    .iter()
                    .map(|(_, d, _)| d.as_raw_fd())
                    .chain(keyboards.iter().map(|k| k.dev.as_raw_fd())),
            );
        }
        if self.pipe.bufs.slots() == 0 {
            // Every device vanished: keep the cadence (poll(0 fds) returns
            // immediately) and let the rescan pick devices back up.
            std::thread::sleep(Duration::from_millis(POLL_TIMEOUT_MS as u64));
            return;
        }
        self.pipe.bufs.poll(POLL_TIMEOUT_MS);

        // Indexed rather than iterated: processing an event needs `&mut self`,
        // which an iterator borrowed from `self.pipe.bufs` would hold hostage. Both
        // `pollfd` and `InputEvent` are `Copy`, so a slot is read out and the
        // borrow ends there.
        for slot in 0..self.pipe.bufs.slots() {
            let revents = self.pipe.bufs.revents(slot);
            if slot_is_dead(revents) {
                self.pipe.bufs.mark_dead(slot);
                continue;
            }
            if revents & libc::POLLIN == 0 {
                continue;
            }
            if slot >= n_mice {
                self.drain_keyboard(shared, slot - n_mice);
                continue;
            }
            {
                let Router { pipe, devices, .. } = self;
                let bufs = &mut pipe.bufs;
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
            for k in 0..self.pipe.bufs.event_count() {
                let ev = self.pipe.bufs.event(k);
                let Pipeline { bufs, env, .. } = &mut self.pipe;
                if bufs.push(ev, env, slot) == Frame::Complete {
                    self.flush_frame(shared);
                }
            }
        }
        // A frame split across two reads (or a device that stopped mid-report)
        // leaves events with no SYN behind them: emit what is pending rather
        // than hold the motion until the next one. A no-op when nothing is.
        self.flush_frame(shared);

        if !self.pipe.bufs.dead_slots().is_empty() {
            let Router {
                pipe,
                devices,
                keyboards,
                ..
            } = self;
            let bufs = &mut pipe.bufs;
            purge_dead(bufs.dead_slots(), n_mice, devices, keyboards, |gone| {
                let (path, kind) = match &gone {
                    Gone::Mouse((path, _, _)) => (path, "device"),
                    Gone::Keyboard(keyboard) => (&keyboard.path, "keyboard"),
                };
                eprintln!("[LittleBigMouse.Hook] evdev: {kind} gone {path:?}");
            });
            bufs.clear_dead();
        }

        let (pipe, mut out) = self.split();
        pipe.end_cycle(shared, &mut out);
    }

    /// Drain a keyboard. While the compositor owns it, only its ctrl state is
    /// taken; while it is grabbed — the pointer is on another machine — its keys
    /// are forwarded there, as one batch per drain.
    ///
    /// The drain is consumed as it comes: the keyboard and the pipeline are
    /// distinct fields, and the events never have to be collected anywhere.
    fn drain_keyboard(&mut self, shared: &'static Shared, k: usize) {
        let now = Instant::now();
        {
            let Router {
                keyboards, pipe, ..
            } = self;
            let keyboard = &mut keyboards[k];
            let grabbed = keyboard.grabbed();
            let Ok(events) = keyboard.dev.fetch_events() else {
                return; // dead nodes are purged via POLLERR next cycle
            };
            for ev in events {
                if ev.event_type() == EventType::KEY {
                    keyboard.held.track(ev.code(), ev.value());
                    pipe.keyboard_key(ev.code(), ev.value(), grabbed, now);
                }
            }
        }
        let (pipe, mut out) = self.split();
        pipe.flush_keys(shared, &mut out);
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
                        // An older open of the same path, set aside, is now
                        // either the same device (its grab would fail on our
                        // own grab forever) or a gone one: drop it either way.
                        self.pending.retain(|(p, _)| *p != path);
                        let mouse = adopt(&self.accel_cfg, path, dev, "hot-plug, ");
                        self.devices.push(mouse);
                    }
                    Err(e) => {
                        eprintln!("[LittleBigMouse.Hook] evdev: cannot grab {path:?}: {e}");
                        set_aside(&mut self.pending, path, dev);
                    }
                }
            }
            for (path, dev) in scan.keyboards {
                if self.keyboards.iter().any(|k| k.path == path) {
                    continue;
                }
                // Grabbed at the end of the cycle if the pointer is on another
                // machine and nothing is held on it.
                if dev.set_nonblocking(true).is_ok() {
                    self.keyboards.push(Keyboard::open(path, dev));
                }
            }
        }
    }

    /// Retry the grab of the mice set aside, every [`REGRAB_EVERY`]: one that
    /// another program held when we found it is routed once it lets go. Only
    /// the grab ioctl runs here; the node is already open.
    fn regrab(&mut self) {
        if self.pending.is_empty() {
            return;
        }
        let now = Instant::now();
        if now < self.regrab_at {
            return;
        }
        self.regrab_at = now + REGRAB_EVERY;
        let Router {
            pending,
            devices,
            accel_cfg,
            ..
        } = self;
        retry_grabs(pending, |path, dev| {
            devices.push(adopt(accel_cfg, path, dev, "released, "))
        });
    }

    /// Run the engine over the accumulated motion and emit the frame where the
    /// pointer now is (`Pipeline::flush_frame`), feeding the acceleration curve of
    /// the mouse the motion came from.
    fn flush_frame(&mut self, shared: &'static Shared) {
        let Router {
            pipe,
            devices,
            virt,
            virt_kbd,
            keyboards,
            link,
            hider,
            ..
        } = self;
        let mut out = Outs {
            virt,
            virt_kbd,
            keyboards,
            link,
            hider,
        };
        pipe.flush_frame(
            shared,
            |idx, x, y, now| {
                let idx = idx.min(devices.len().saturating_sub(1));
                match devices.get_mut(idx) {
                    Some((_, _, accel)) => accel.apply(x, y, now),
                    None => (x, y),
                }
            },
            &mut out,
        );
    }
}

impl Drop for Router {
    fn drop(&mut self) {
        self.scan_stop.store(true, Ordering::Relaxed);
        // A pointer on another machine comes home first: its `Leave` releases what
        // the stream pressed there, and the keyboards go back to the compositor.
        let (pipe, mut out) = self.split();
        pipe.shut_down(&mut out);
        // A button still held here is one the compositor saw pressed on the
        // virtual pointer and would never see released: the device is about to
        // disappear, and whatever release follows lands on the real device the
        // ungrab below hands back. The seat keeps the button down, and the user
        // loses that button until something resets the state — the failure
        // reads as "I have no left click any more". Pay the releases while the
        // virtual device is still alive. (A button held on the other machine is
        // among them; a release of a button the virtual pointer never pressed
        // changes nothing.)
        let releases = self.pipe.env.release_frame();
        if !releases.is_empty() {
            let _ = self.virt.emit(&releases);
            self.pipe.env.buttons = 0;
        }
        for (_, d, _) in &mut self.devices {
            let _ = d.ungrab();
        }
        // Keyboards were let go above; closing their fds would release a grab
        // anyway. The scanner thread exits on the flag (or on its next send, once
        // scan_rx is gone).
    }
}
