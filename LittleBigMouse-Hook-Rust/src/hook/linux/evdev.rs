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
//! Safety: a grab is released when its fd closes, so even `kill -9` frees the
//! mice. We additionally ungrab on unhook, on quit, and on drop; `LBM_EVDEV_
//! AUTORELEASE_SECS` force-unhooks after N seconds for cautious first runs.
//!
//! Not mapped yet: ctrl-override (needs reading a grabbed keyboard) and
//! focus-based exclusion — reported inert, same as the other Linux backends.

use std::os::fd::AsRawFd;
use std::sync::atomic::Ordering;
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

    eprintln!("[LittleBigMouse.Hook] evdev backend running (sens={sens})");

    loop {
        if shared.want_quit.load(Ordering::SeqCst) {
            drop(router);
            return true;
        }

        let want = shared.want_hook.load(Ordering::SeqCst);

        if want && router.is_none() {
            match Router::arm(shared, sens, debug) {
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

/// The grabbed devices, the virtual pointer, and the authoritative cursor.
struct Router {
    devices: Vec<Device>,
    virt: VirtualDevice,
    env: EvdevCursor,
    sens: f64,
    debug: bool,
    /// Sub-pixel remainder of the sensitivity-scaled raw motion integration.
    rem: (f64, f64),
}

impl Router {
    fn arm(shared: &Shared, sens: f64, debug: bool) -> std::io::Result<Router> {
        let desktop = desktop_bounds(shared);

        let mut devices = Vec::new();
        for (path, mut dev) in enumerate_mice() {
            dev.set_nonblocking(true)?;
            match dev.grab() {
                Ok(()) => {
                    eprintln!("[LittleBigMouse.Hook] evdev: grabbed {} ({:?})",
                        dev.name().unwrap_or("?"), path);
                    devices.push(dev);
                }
                Err(e) => eprintln!("[LittleBigMouse.Hook] evdev: cannot grab {path:?}: {e}"),
            }
        }
        if devices.is_empty() {
            return Err(std::io::Error::new(std::io::ErrorKind::NotFound, "no grabbable mouse"));
        }

        let virt = build_virtual(desktop)?;

        // Start at a sane, known point: the centre of the first main zone.
        let start = first_zone_center(shared).unwrap_or_else(|| {
            Point::new(desktop.left() + desktop.width() / 2, desktop.top() + desktop.height() / 2)
        });

        let mut router = Router {
            devices,
            virt,
            env: EvdevCursor { virtual_pos: start, clip: None, desktop, started: Instant::now() },
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
        let current = desktop_bounds(shared);
        if current != self.env.desktop {
            if let Ok(v) = build_virtual(current) {
                self.virt = v;
                self.env.desktop = current;
                self.env.virtual_pos = self.env.clamp(self.env.virtual_pos);
                self.emit_absolute(&mut Vec::new());
            }
        }

        let mut fds: Vec<libc::pollfd> = self
            .devices
            .iter()
            .map(|d| libc::pollfd { fd: d.as_raw_fd(), events: libc::POLLIN, revents: 0 })
            .collect();
        unsafe { libc::poll(fds.as_mut_ptr(), fds.len() as libc::nfds_t, 100); }

        let mut acc = (0i64, 0i64);
        let mut passthrough: Vec<InputEvent> = Vec::new();

        for (i, pfd) in fds.iter().enumerate() {
            if pfd.revents & libc::POLLIN == 0 {
                continue;
            }
            let events: Vec<InputEvent> = match self.devices[i].fetch_events() {
                Ok(it) => it.collect(),
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => continue,
                Err(_) => continue,
            };
            for ev in events {
                match ev.event_type() {
                    EventType::SYNCHRONIZATION => self.flush_frame(shared, &mut acc, &mut passthrough),
                    EventType::RELATIVE if ev.code() == RelativeAxisCode::REL_X.0 => acc.0 += ev.value() as i64,
                    EventType::RELATIVE if ev.code() == RelativeAxisCode::REL_Y.0 => acc.1 += ev.value() as i64,
                    // Wheels and any other relative axis: pass through verbatim.
                    EventType::RELATIVE => passthrough.push(ev),
                    // Buttons, and the MSC_SCAN that precedes them.
                    EventType::KEY | EventType::MISC => passthrough.push(ev),
                    _ => {}
                }
            }
        }
        if acc != (0, 0) || !passthrough.is_empty() {
            self.flush_frame(shared, &mut acc, &mut passthrough);
        }
    }

    /// Run the engine over the accumulated motion and place the cursor at the
    /// resulting absolute position, forwarding buttons/wheels in the same frame.
    fn flush_frame(&mut self, shared: &Shared, acc: &mut (i64, i64), passthrough: &mut Vec<InputEvent>) {
        if *acc == (0, 0) && passthrough.is_empty() {
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
                eprintln!("[LittleBigMouse.Hook] evdev: frame d=({dx},{dy}) cand=({},{}) -> emit ({},{}){}",
                    candidate.x(), candidate.y(), self.env.virtual_pos.x(), self.env.virtual_pos.y(),
                    if e.handled { " CROSS" } else { "" });
            }
            *acc = (0, 0);
        }

        self.emit_absolute(passthrough);
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
        for d in &mut self.devices {
            let _ = d.ungrab();
        }
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

/// The union of the layout's main zones — the compositor's logical pixel space
/// (kscreen coordinates), so the ABS mapping and the crossing geometry agree.
fn desktop_bounds(shared: &Shared) -> Rect<i32> {
    let engine = shared.engine.lock().unwrap_or_else(|p| p.into_inner());
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
}

impl EvdevCursor {
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
        false // needs reading a grabbed keyboard; documented Windows/X11-only
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
