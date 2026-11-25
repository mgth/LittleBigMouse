//! What one cycle of the pump does with what it read, with no device in sight: run the
//! engine, then emit to this machine's virtual devices — or, in the multi-PC mode
//! (docs/multi-pc.md), to another machine through the input stream.
//!
//! Split out of the router so that all of it can be driven by tests and benches
//! through [`Outputs`], the way `frame` already is, without opening `/dev/input` or
//! `/dev/uinput` and without grabbing anything.
//!
//! # The master
//!
//! The engine routes over every zone in play, this machine's and — while the input
//! endpoint has a client, the agent — the other machines' (`ZonesLayout::
//! set_remote_enterable`, set every cycle from [`Outputs::remote_connected`]). The
//! virtual cursor is authoritative, so after each routed report its position alone
//! says where the report goes ([`crossing::locate`]): a position on this machine is
//! emitted to the virtual pointer exactly as it always was; a position in another
//! machine's zone is translated to that machine's own desktop pixels and written to the
//! agent as an `lbm_ipc::input` batch. Another machine's desktop lies outside this
//! one's (each is offset to the right of everything else), so the cursor's extent grows
//! to cover them while they are in play, and what is emitted here is clamped to this
//! desktop and, while the pointer is away, frozen where it left: the local cursor stays
//! where it was, at [`Pipeline::local_pos`].
//!
//! What follows from where the pointer is — the `Enter` and the `Leave`, the
//! keyboards' grab, where a held key or button is released, coming home, the rescue,
//! hiding the local cursor — is the platform-neutral [`crossing::Crossing`]'s. This
//! pipeline is one master among those it serves: it lends the rules, through
//! [`crossing::Platform`], its virtual cursor, the frame being composed and its
//! outputs. Putting the pointer back home also has the engine forget where it was
//! (`MouseEngine::begin_touch`) before it routes again, at the next chance to take its
//! lock without waiting.
//!
//! ## What the rules rely on here
//!
//! * The frame's buttons and keys are sorted by evdev type and code: mouse buttons
//!   (`BTN_LEFT..=BTN_TASK`) and keyboard usages go to the rules; tablet tools and
//!   joystick buttons stay here, meaning nothing over there; wheels go over in 120ths
//!   of a detent, and scancodes (`EV_MSC`) describe this machine's keyboard.
//! * Pressed over there and released after coming home: `Leave` released it over there
//!   (the remote's session releases everything the stream left pressed), and the
//!   release that comes later is handed to this machine's virtual device, which drops a
//!   release of something it never had pressed (the kernel filters an `EV_KEY` that
//!   does not change the state).
//! * A keyboard is grabbed only once none of its keys is held. A key held while
//!   crossing — the modifier of a Ctrl+drag, say — went to the compositor, and its
//!   release has to go there too; grabbing that keyboard would send the release here
//!   instead, and the compositor would keep the key down for good (libinput drops a
//!   release from another device). Until the user lets go, that keyboard keeps typing
//!   on this machine; the next cycle after, it is grabbed. A press that lands in the
//!   instant between the last read and the grab is the one case this does not cover.
//! * With the keyboards grabbed the portal's global shortcut never fires: the
//!   compositor sees no key. That is why the rules watch for the rescue in the grabbed
//!   stream.
//! * The local cursor is hidden by [`super::hider`].
//!
//! ## Cost
//!
//! The path where the pointer stays here allocates nothing and adds a containment test
//! to each report (the host lookup only runs for a position outside this desktop) and
//! a comparison to each cycle. A report sent to another machine allocates its batch.

use std::sync::TryLockError;
use std::time::Instant;

use evdev::{EventType, InputEvent};

use crate::engine::cursor::CursorEnv;
use crate::engine::MouseEngine;
use crate::geometry::{Point, Rect};
use crate::hook::hot_path::{count_event, route_move_then, Routed};
use crate::shared::Shared;
use lbm_ipc::input::Locks;

use super::cursor::EvdevCursor;
use super::frame::PumpBuffers;
use super::probe::{extents, try_device_bounds};
use super::{BTN_MOUSE_RANGE, BTN_RANGE, BTN_TRIGGER_HAPPY_RANGE};
use crate::hook::crossing::{self, Crossing, Input, Place, Platform, Press};

/// Everything a cycle writes to: this machine's two virtual devices, the input stream,
/// the keyboards' exclusivity and the cursor's visibility. The router's implementation
/// owns the devices; the tests' records what it is handed.
pub(crate) trait Outputs {
    /// One frame for the virtual pointer.
    fn pointer(&mut self, frame: &[InputEvent]);
    /// One frame for the virtual keyboard.
    fn keyboard(&mut self, frame: &[InputEvent]);
    /// See [`crossing::Platform::remote_client`].
    fn remote_client(&self) -> Option<u64>;
    /// See [`crossing::Platform::remote_connected`].
    fn remote_connected(&self) -> bool {
        self.remote_client().is_some()
    }
    /// See [`crossing::Platform::send_remote`].
    fn send_remote(&mut self, batch: Vec<u8>) -> bool;
    /// See [`crossing::Platform::locks`].
    fn locks(&self) -> Locks {
        Locks::default()
    }
    /// See [`crossing::Platform::grab_keyboards`].
    fn grab_keyboards(&mut self);
    /// See [`crossing::Platform::release_keyboards`].
    fn release_keyboards(&mut self);
    /// See [`crossing::Platform::hide_cursor`].
    fn hide_cursor(&mut self, from: Point<i32>);
    /// See [`crossing::Platform::cursor_hiding_spot`].
    fn cursor_hiding_spot(&mut self) -> Option<Point<i32>>;
    /// See [`crossing::Platform::show_cursor`].
    fn show_cursor(&mut self);
}

/// The pump's state between devices and outputs.
pub(crate) struct Pipeline {
    pub(super) bufs: PumpBuffers,
    pub(super) env: EvdevCursor,
    sens: f64,
    /// Sub-pixel remainder of the sensitivity-scaled raw motion integration.
    rem: (f64, f64),
    debug: bool,
    /// The engine still tracks the pointer where it was before it came home, and has
    /// to be told before it routes again (`MouseEngine::begin_touch`).
    resync: bool,
    /// Where the pointer is between this machine and the others, and the rules of
    /// going from one to the other.
    crossing: Crossing,
}

/// What the crossing rules are lent of a pipeline for one call: its pointer and
/// the frame being dispatched, with the outputs they go to.
struct Machine<'a, O> {
    bufs: &'a mut PumpBuffers,
    env: &'a mut EvdevCursor,
    resync: &'a mut bool,
    out: &'a mut O,
}

impl<O: Outputs> Platform for Machine<'_, O> {
    fn remote_client(&self) -> Option<u64> {
        self.out.remote_client()
    }

    fn remote_connected(&self) -> bool {
        self.out.remote_connected()
    }

    fn send_remote(&mut self, batch: Vec<u8>) -> bool {
        self.out.send_remote(batch)
    }

    fn locks(&self) -> Locks {
        self.out.locks()
    }

    fn grab_keyboards(&mut self) {
        self.out.grab_keyboards();
    }

    fn release_keyboards(&mut self) {
        self.out.release_keyboards();
    }

    fn hide_cursor(&mut self, from: Point<i32>) {
        self.out.hide_cursor(from);
    }

    fn cursor_hiding_spot(&mut self) -> Option<Point<i32>> {
        self.out.cursor_hiding_spot()
    }

    fn show_cursor(&mut self) {
        self.out.show_cursor();
    }

    fn pointer(&self) -> Point<i32> {
        self.env.virtual_pos
    }

    fn return_pointer(&mut self, home: Point<i32>) {
        self.env.virtual_pos = home;
        self.env.release_clip();
        // Told before the engine routes again (`Pipeline::resync_with`).
        *self.resync = true;
    }

    fn emit_pointer(&mut self, at: Point<i32>) {
        emit_at(self.bufs, self.env.device, at, self.out);
    }

    fn pointer_pending(&self) -> bool {
        self.bufs.pointer_pending()
    }

    fn emit_keyboard(&mut self) {
        let out = &mut *self.out;
        self.bufs.take_keyboard_frame(|frame| out.keyboard(frame));
    }

    fn take_remote_events(&mut self, mut take: impl FnMut(Input) -> bool) -> (i32, i32) {
        let wheels = self.bufs.wheel_120ths();
        self.bufs.retain_local(|ev| match ev.event_type() {
            // Wheels are counted above; scancodes describe this machine's keyboard.
            EventType::RELATIVE | EventType::MISC => false,
            EventType::KEY if BTN_MOUSE_RANGE.contains(&ev.code()) => {
                !take(Input::Button(ev.code(), Press::from_ev_key(ev.value())))
            }
            // Tablet tools and joystick buttons mean nothing over there.
            EventType::KEY
                if BTN_RANGE.contains(&ev.code())
                    || BTN_TRIGGER_HAPPY_RANGE.contains(&ev.code()) =>
            {
                true
            }
            EventType::KEY => !take(Input::Key(ev.code(), Press::from_ev_key(ev.value()))),
            _ => true,
        });
        wheels
    }
}

/// The pending buttons and wheels as one frame of the virtual pointer, at `p`.
/// Clamped to the desktop the device spans: nothing emitted here may land outside it,
/// whatever the virtual cursor is doing.
fn emit_at(bufs: &mut PumpBuffers, d: Rect<i32>, p: Point<i32>, out: &mut impl Outputs) {
    let ax = p.x().min(d.right() - 1).max(d.left()) - d.left();
    let ay = p.y().min(d.bottom() - 1).max(d.top()) - d.top();
    out.pointer(bufs.pointer_frame(ax, ay));
}

impl Pipeline {
    pub(super) fn new(env: EvdevCursor, sens: f64, debug: bool) -> Pipeline {
        let home = env.virtual_pos;
        Pipeline {
            bufs: PumpBuffers::new(),
            env,
            sens,
            rem: (0.0, 0.0),
            debug,
            resync: false,
            crossing: Crossing::new(home),
        }
    }

    /// The crossing rules, and what they are lent of this pipeline and `out`.
    fn split<'a, O: Outputs>(&'a mut self, out: &'a mut O) -> (&'a mut Crossing, Machine<'a, O>) {
        let Pipeline {
            bufs,
            env,
            resync,
            crossing,
            ..
        } = self;
        (
            crossing,
            Machine {
                bufs,
                env,
                resync,
                out,
            },
        )
    }

    pub(super) fn is_remote(&self) -> bool {
        self.crossing.is_remote()
    }

    /// Where the pointer is on this machine: the virtual cursor, or where it left
    /// from while it is away.
    pub(super) fn local_pos(&self) -> Point<i32> {
        if self.crossing.is_remote() {
            self.crossing.home()
        } else {
            self.env.virtual_pos
        }
    }

    /// The local position plus the pending buttons and wheels as one frame of the
    /// virtual pointer, clamped to the desktop the device spans. While the pointer is
    /// away, the frozen pointer's.
    pub(super) fn emit_local(&mut self, out: &mut impl Outputs) {
        let p = if self.crossing.is_remote() {
            self.crossing.frozen_at()
        } else {
            self.env.virtual_pos
        };
        emit_at(&mut self.bufs, self.env.device, p, out);
    }

    /// The start of a cycle: catch up with the input stream's client and the layout,
    /// and come home if either says so. Returns the desktop the virtual pointer should
    /// span, when the locks could be taken without waiting.
    pub(super) fn begin_cycle(
        &mut self,
        shared: &'static Shared,
        out: &mut impl Outputs,
    ) -> Option<Rect<i32>> {
        let connected = {
            let (crossing, mut machine) = self.split(out);
            crossing.begin_cycle(shared, &mut machine)
        };

        // Routing-thread rule: never block. On contention nothing is updated and the
        // next cycle asks again.
        let local = {
            let mut engine = match shared.engine.try_lock() {
                Ok(guard) => guard,
                Err(TryLockError::Poisoned(p)) => p.into_inner(),
                Err(TryLockError::WouldBlock) => return None,
            };
            // A no-op unless the client came or went, or a Load brought a layout in
            // which the other machines are not in play yet. Rebuilding allocates, but
            // only on such a change, never per report.
            // Locked here, the other machines' borders are walls, as without a client.
            // Only the machines the agent drives, once it named them.
            self.crossing.put_in_play(&mut engine.layout, connected);
            {
                let pos = self.env.virtual_pos;
                let (crossing, mut machine) = self.split(out);
                crossing.check_layout(&engine.layout, pos, &mut machine);
            }
            if self.resync {
                self.resync_with(&mut engine);
            }
            let (bounds, local) = extents(&engine.layout);
            if bounds != self.env.bounds {
                self.env.set_bounds(bounds);
                self.env.virtual_pos = self.env.clamp(self.env.virtual_pos);
                self.emit_local(out);
            }
            local
        };
        try_device_bounds(shared, local)
    }

    /// The end of a cycle: see [`Crossing::end_cycle`].
    pub(super) fn end_cycle(&mut self, shared: &'static Shared, out: &mut impl Outputs) {
        let (crossing, mut machine) = self.split(out);
        crossing.end_cycle(shared, &mut machine);
    }

    /// The router is going away — Stop, pause, suspend, the rescue's unhook, quit: the
    /// pointer comes home while the devices are still there to say so.
    pub(super) fn shut_down(&mut self, out: &mut impl Outputs) {
        let (crossing, mut machine) = self.split(out);
        crossing.shut_down(&mut machine);
    }

    /// Run the engine over the accumulated motion, then emit the frame where the
    /// pointer now is. `accel` turns a raw delta of the mouse at that index into
    /// pointer motion.
    pub(super) fn flush_frame(
        &mut self,
        shared: &'static Shared,
        accel: impl FnOnce(usize, f64, f64, u64) -> (f64, f64),
        out: &mut impl Outputs,
    ) {
        if !self.bufs.frame_pending() {
            return;
        }

        let acc = self.bufs.take_motion();
        let mut place = None;
        if acc != (0, 0) {
            // Reproduce the pointer acceleration libinput would have applied had
            // we not grabbed the device (the ABS virtual pointer bypasses it):
            // per-device curve on the raw frame delta, THEN the user's flat
            // LBM_EVDEV_SENS multiplier. Sub-pixel remainders carry over frames.
            let now = self.env.started.elapsed().as_micros() as u64;
            let (ax, ay) = accel(self.bufs.last_motion_dev(), acc.0 as f64, acc.1 as f64, now);
            let sx = ax * self.sens + self.rem.0;
            let sy = ay * self.sens + self.rem.1;
            let (dx, dy) = (sx.trunc() as i32, sy.trunc() as i32);
            self.rem = (sx - dx as f64, sy - dy as f64);

            let old = self.env.virtual_pos;
            // Win32 parity: the LL hook sees the UNCLIPPED proposed point even while
            // ClipCursor pins the cursor — the growing past-border distance is what
            // drains border resistance. Only the committed position gets clamped.
            let candidate = Point::new(old.x().saturating_add(dx), old.y().saturating_add(dy));

            // Never on the path where the pointer stays here: the engine has to be told
            // it came home before it routes again; and on another machine, a Load that
            // landed since the cycle began brought a layout with nothing of the other
            // machines in play yet, which would read as that machine having gone.
            if self.resync || self.is_remote() {
                let engine = match shared.engine.try_lock() {
                    Ok(engine) => Some(engine),
                    Err(TryLockError::Poisoned(p)) => Some(p.into_inner()),
                    Err(TryLockError::WouldBlock) => None,
                };
                if let Some(mut engine) = engine {
                    if self.resync {
                        self.resync_with(&mut engine);
                    } else {
                        self.crossing
                            .put_in_play(&mut engine.layout, out.remote_connected());
                    }
                }
            }

            // The evdev variants around the shared route (see the table in
            // `hot_path`): the cursor is authoritative, so someone must always
            // commit a position — the engine's verdict on a crossing, our own
            // clamp otherwise. A contended route therefore cannot just pass: it
            // commits the clamped candidate and emits right here (and that frame
            // has never been counted as an event — the early return skips the
            // pending keyboard frame too, exactly as before).
            let routed = if self.resync {
                (Routed::Contended, None)
            } else {
                let Pipeline { env, crossing, .. } = self;
                let driving = crossing.driving();
                route_move_then(&shared.engine, env, candidate, |engine, env, handled| {
                    locate(engine, env, candidate, handled, driving)
                })
            };
            match routed {
                (Routed::Contended, _) if !self.is_remote() => {
                    self.env.virtual_pos = self.env.clamp(candidate);
                    self.crossing.set_home(self.env.virtual_pos);
                    self.emit_local(out);
                    return;
                }
                // On another machine, where a position is cannot be told without the
                // layout: the pointer holds still for this report.
                (Routed::Contended, _) => self.env.virtual_pos = old,
                (Routed::Passed, located) => {
                    self.env.virtual_pos = self.env.clamp(candidate);
                    count_event();
                    place = located;
                }
                (Routed::Crossed, located) => {
                    count_event();
                    place = located;
                }
            }
            if self.debug {
                // Per-frame trace: raw delta, engine input, emitted position. The
                // ground truth for any "the cursor was seen somewhere we never
                // sent it" investigation (compare against what KWin displays).
                eprintln!("[LittleBigMouse.Hook] evdev: frame d=({dx},{dy}) cand=({},{}) -> virtual ({},{}){}{}{}",
                    candidate.x(), candidate.y(), self.env.virtual_pos.x(), self.env.virtual_pos.y(),
                    if routed.0.handled() { " CROSS" } else { "" },
                    if self.env.ctrl_down() { " ctrl" } else { "" },
                    if self.is_remote() { " remote" } else { "" });
            }
        }

        self.dispatch(shared, place, out);
    }

    /// Sends the frame where `place` says the pointer now is: see
    /// [`Crossing::dispatch`].
    fn dispatch(&mut self, shared: &'static Shared, place: Option<Place>, out: &mut impl Outputs) {
        let (crossing, mut machine) = self.split(out);
        crossing.dispatch(shared, place, &mut machine);
    }

    /// One key of an observed keyboard. `grabbed`: the compositor does not see it, so
    /// it is this process's to forward.
    pub(super) fn keyboard_key(&mut self, code: u16, value: i32, grabbed: bool, now: Instant) {
        self.env.track_ctrl(code, value);
        let press = Press::from_ev_key(value);
        if grabbed {
            self.crossing.grabbed_key(code, press, now);
        } else {
            self.crossing.observed_key(code, press, now);
        }
    }

    /// Sends the keys [`Self::keyboard_key`] collected from one keyboard's drain.
    pub(super) fn flush_keys(&mut self, shared: &'static Shared, out: &mut impl Outputs) {
        let (crossing, mut machine) = self.split(out);
        crossing.flush_keys(shared, &mut machine);
    }

    /// Fires the rescue if its shortcut has been held long enough in the grabbed
    /// stream: see [`Crossing::check_rescue`].
    #[cfg(test)]
    pub(super) fn check_rescue(
        &mut self,
        shared: &'static Shared,
        now: Instant,
        out: &mut impl Outputs,
    ) {
        let (crossing, mut machine) = self.split(out);
        crossing.check_rescue(shared, now, &mut machine);
    }

    /// Drops the engine's crossing history and puts the cursor back where the routing
    /// resumes (home, or where a lock holds it over there): the next
    /// report starts afresh from the zone it is in.
    fn resync_with(&mut self, engine: &mut MouseEngine) {
        engine.begin_touch(&mut self.env);
        self.env.virtual_pos = self.crossing.resume_at();
        self.resync = false;
    }
}

/// Where a routed report leaves the pointer: what the caller will commit — the
/// engine's verdict, or the clamped candidate — located by [`crossing::locate`].
fn locate(
    engine: &MouseEngine,
    env: &EvdevCursor,
    candidate: Point<i32>,
    handled: bool,
    driving: Option<&str>,
) -> Place {
    let pos = if handled {
        env.virtual_pos
    } else {
        env.clamp(candidate)
    };
    crossing::locate(&engine.layout, pos, env.device.contains(pos), driving)
}

#[cfg(test)]
mod tests {
    //! The pipeline driven the way the router drives it, with recording outputs: no
    //! device is opened and nothing is grabbed.

    use super::*;

    use std::sync::atomic::Ordering;
    use std::sync::Arc;
    use std::time::Duration;

    use evdev::{AbsoluteAxisCode, KeyCode, RelativeAxisCode};
    use lbm_ipc::input::InputEvent as Wire;
    use lbm_ipc::input::{by_host, decode, read_batch};

    use super::super::frame::Frame;
    use crate::engine::event::MouseEventArg;
    use crate::hook::crossing::keys::HOLD;
    use crate::ipc::input::{Driver, Outlet};
    use crate::zones::ZonesLayout;

    /// Here (0..1920) is this machine's; There, another machine's 1920×1080 desktop,
    /// sits at 3000 in the engine's pixel space. Their facing borders link to each
    /// other, without resistance.
    const LAYOUT: &str = concat!(
        r#"<ZonesLayout Algorithm="Strait"><MainZones>"#,
        r#"<Zone Id="0" Name="Here"><PixelsBounds><Rect Left="0" Top="0" Width="1920" Height="1080"></Rect></PixelsBounds><PhysicalBounds><Rect Left="0" Top="0" Width="527" Height="296"></Rect></PhysicalBounds>"#,
        r#"<RightLinks><ZoneLink From="0" To="296" SourceFromPixel="0" SourceToPixel="1080" TargetFromPixel="0" TargetToPixel="1080" BorderResistance="0" TargetId="1"></ZoneLink></RightLinks></Zone>"#,
        r#"<Zone Id="1" Name="There" Host="abcd" HostX="3000" HostY="0"><PixelsBounds><Rect Left="3000" Top="0" Width="1920" Height="1080"></Rect></PixelsBounds><PhysicalBounds><Rect Left="527" Top="0" Width="527" Height="296"></Rect></PhysicalBounds>"#,
        r#"<LeftLinks><ZoneLink From="0" To="296" SourceFromPixel="0" SourceToPixel="1080" TargetFromPixel="0" TargetToPixel="1080" BorderResistance="0" TargetId="0"></ZoneLink></LeftLinks></Zone>"#,
        r#"</MainZones></ZonesLayout>"#,
    );

    const START: (i32, i32) = (1900, 500);
    const BTN_LEFT: u16 = 0x110;
    const KEY_A: u16 = 30;
    const KEY_M: u16 = 50;
    const KEY_LEFTCTRL: u16 = 29;
    const KEY_LEFTALT: u16 = 56;
    const KEY_LEFTSHIFT: u16 = 42;

    fn rel(axis: RelativeAxisCode, value: i32) -> InputEvent {
        InputEvent::new(EventType::RELATIVE.0, axis.0, value)
    }

    fn key(code: u16, value: i32) -> InputEvent {
        InputEvent::new(EventType::KEY.0, code, value)
    }

    fn syn() -> InputEvent {
        InputEvent::new(EventType::SYNCHRONIZATION.0, 0, 0)
    }

    fn shape(frame: &[InputEvent]) -> Vec<(u16, u16, i32)> {
        frame
            .iter()
            .map(|e| (e.event_type().0, e.code(), e.value()))
            .collect()
    }

    /// What the pipeline handed out. The input stream is either a scripted one —
    /// connected and accepting as the test says — or a real endpoint's client.
    struct Recording {
        pointer: Vec<Vec<(u16, u16, i32)>>,
        keyboard: Vec<Vec<(u16, u16, i32)>>,
        connected: bool,
        accepting: bool,
        sent: Vec<Wire>,
        /// Every event sent, with the machine the last `Host` marker named.
        routed: Vec<(Option<String>, Wire)>,
        marked: Option<String>,
        /// The keyboards are grabbed: asked for, and not let go since.
        grabbed: bool,
        releases: usize,
        /// Which client the stream has, without a driver.
        client: u64,
        driver: Option<(Driver, Arc<Outlet>)>,
        /// Each hide: where from, and how many events had been sent by then.
        hides: Vec<((i32, i32), usize)>,
        shows: usize,
        /// What hides the cursor is in place, under this spot.
        spot: Option<Point<i32>>,
    }

    impl Outputs for Recording {
        fn pointer(&mut self, frame: &[InputEvent]) {
            self.pointer.push(shape(frame));
        }

        fn keyboard(&mut self, frame: &[InputEvent]) {
            self.keyboard.push(shape(frame));
        }

        fn remote_client(&self) -> Option<u64> {
            match &self.driver {
                Some((driver, _)) => driver.client(),
                None => self.connected.then_some(self.client),
            }
        }

        fn send_remote(&mut self, batch: Vec<u8>) -> bool {
            if let Some((driver, _)) = &mut self.driver {
                return driver.send(batch);
            }
            if !(self.connected && self.accepting) {
                return false;
            }
            for (host, run) in by_host(&batch).expect("a batch that walks") {
                if let Some(host) = host {
                    self.marked = Some(host.to_owned());
                }
                for event in decode(run).expect("a batch that decodes") {
                    self.sent.push(event);
                    self.routed.push((self.marked.clone(), event));
                }
            }
            true
        }

        fn grab_keyboards(&mut self) {
            self.grabbed = true;
        }

        fn release_keyboards(&mut self) {
            self.grabbed = false;
            self.releases += 1;
        }

        fn hide_cursor(&mut self, from: Point<i32>) {
            let sent = match &self.driver {
                Some(_) => usize::MAX,
                None => self.sent.len(),
            };
            self.hides.push(((from.x(), from.y()), sent));
        }

        fn cursor_hiding_spot(&mut self) -> Option<Point<i32>> {
            self.spot
        }

        fn show_cursor(&mut self) {
            self.shows += 1;
        }
    }

    struct Rig {
        shared: &'static Shared,
        pipe: Pipeline,
        out: Recording,
    }

    impl Rig {
        fn new(connected: bool) -> Rig {
            Rig::with_driver(connected, None)
        }

        fn with_driver(connected: bool, driver: Option<(Driver, Arc<Outlet>)>) -> Rig {
            Rig::build(connected, driver, LAYOUT, None)
        }

        fn build(
            connected: bool,
            driver: Option<(Driver, Arc<Outlet>)>,
            layout: &str,
            drivable: Option<&[&str]>,
        ) -> Rig {
            let shared: &'static Shared = Box::leak(Box::new(Shared::new()));
            shared
                .engine
                .lock()
                .unwrap()
                .load(ZonesLayout::from_xml(layout).unwrap());
            *shared.drivable.lock().unwrap() =
                drivable.map(|hosts| hosts.iter().map(|h| (*h).to_owned()).collect());
            shared.drivable_revision.fetch_add(1, Ordering::SeqCst);
            let desktop = Rect::new(0, 0, 1920, 1080);
            *shared.desktop.lock().unwrap() = Some(desktop);
            shared.want_hook.store(true, Ordering::SeqCst);

            let start = Point::new(START.0, START.1);
            let mut pipe = Pipeline::new(EvdevCursor::new(desktop, desktop, start), 1.0, false);
            // As `Router::arm` does: the engine starts tracking at the start point.
            shared
                .engine
                .lock()
                .unwrap()
                .on_mouse_move(&mut pipe.env, &mut MouseEventArg::new(start));
            let mut rig = Rig {
                shared,
                pipe,
                out: Recording {
                    pointer: Vec::new(),
                    keyboard: Vec::new(),
                    connected,
                    accepting: true,
                    sent: Vec::new(),
                    routed: Vec::new(),
                    marked: None,
                    grabbed: false,
                    releases: 0,
                    client: 1,
                    driver,
                    hides: Vec::new(),
                    shows: 0,
                    spot: None,
                },
            };
            // The first cycle takes in the client and the layout.
            rig.cycle(&[]);
            rig.out.pointer.clear();
            rig
        }

        /// One pump cycle over the events a mouse reported, as `Router::pump` runs it.
        fn cycle(&mut self, events: &[InputEvent]) {
            if let Some((driver, outlet)) = &mut self.out.driver {
                driver.refresh(outlet);
            }
            if let Some(device) = self.pipe.begin_cycle(self.shared, &mut self.out) {
                self.pipe.env.device = device;
            }
            for &ev in events {
                let Pipeline { bufs, env, .. } = &mut self.pipe;
                if bufs.push(ev, env, 0) == Frame::Complete {
                    self.flush();
                }
            }
            self.flush();
            self.pipe.end_cycle(self.shared, &mut self.out);
        }

        fn flush(&mut self) {
            self.pipe
                .flush_frame(self.shared, |_, x, y, _| (x, y), &mut self.out);
        }

        fn move_by(&mut self, dx: i32, dy: i32) {
            self.cycle(&[
                rel(RelativeAxisCode::REL_X, dx),
                rel(RelativeAxisCode::REL_Y, dy),
                syn(),
            ]);
        }

        /// A key of a keyboard, as `Router::drain_keyboard` hands it over. These
        /// keyboards are grabbed whenever the pipeline asks, and only then.
        fn type_key(&mut self, code: u16, value: i32) {
            let grabbed = self.out.grabbed;
            self.pipe.keyboard_key(code, value, grabbed, Instant::now());
            self.pipe.flush_keys(self.shared, &mut self.out);
        }

        /// Where the last frame of the virtual pointer put the local cursor.
        fn local_cursor(&self) -> (i32, i32) {
            let frame = self.out.pointer.last().expect("a pointer frame");
            let axis = |code: AbsoluteAxisCode| {
                frame
                    .iter()
                    .find(|e| e.0 == EventType::ABSOLUTE.0 && e.1 == code.0)
                    .map(|e| e.2)
                    .expect("an absolute position")
            };
            (axis(AbsoluteAxisCode::ABS_X), axis(AbsoluteAxisCode::ABS_Y))
        }

        /// Crosses Here's right border.
        fn go_over(&mut self) {
            self.move_by(30, 0);
            assert!(self.pipe.is_remote(), "the pointer went over");
        }
    }

    #[test]
    fn crossing_sends_enter_at_the_remote_pixel_then_motion_in_the_remotes_own_pixels() {
        let mut rig = Rig::new(true);
        rig.go_over();
        // Landed on There's left column, at the same height: its own pixel (0, 500).
        assert_eq!(rig.out.sent, [Wire::Enter { x: 0, y: 500 }]);
        assert!(rig.out.grabbed, "the keyboards are grabbed on the way over");

        rig.move_by(10, 4);
        assert_eq!(
            rig.out.sent,
            [Wire::Enter { x: 0, y: 500 }, Wire::Motion { x: 10, y: 504 }]
        );
        assert!(
            rig.out.pointer.is_empty(),
            "the virtual pointer got {:?} while the pointer was away",
            rig.out.pointer
        );
    }

    #[test]
    fn a_grabbed_mouses_own_keys_follow_the_pointer_too() {
        let mut rig = Rig::new(true);
        rig.go_over();
        rig.cycle(&[key(KEY_A, 1), syn()]);
        assert_eq!(
            rig.out.sent[1..],
            [Wire::Key {
                code: KEY_A,
                pressed: true
            }]
        );
        assert!(rig.out.keyboard.is_empty());
    }

    #[test]
    fn coming_back_sends_leave_lets_the_keyboards_go_and_resumes_here() {
        let mut rig = Rig::new(true);
        rig.go_over();
        rig.move_by(10, 0);
        rig.move_by(-20, 0);

        assert!(!rig.pipe.is_remote());
        assert_eq!(rig.out.sent.last(), Some(&Wire::Leave));
        assert_eq!(rig.out.releases, 1);
        assert_eq!(rig.out.shows, 1, "the cursor is shown again");
        // Landed on Here's last column, where the engine put it.
        assert_eq!(rig.local_cursor(), (1919, 500));

        rig.move_by(-100, 0);
        assert_eq!(rig.local_cursor(), (1819, 500));
        assert_eq!(
            rig.out.sent.last(),
            Some(&Wire::Leave),
            "nothing more over there"
        );
    }

    /// Sets the pointer lock's shortcut, Ctrl+Alt+L, and presses it once.
    fn lock(rig: &mut Rig) {
        const KEY_L: u16 = 38;
        if rig.shared.pointer_lock_shortcut.lock().unwrap().is_empty() {
            *rig.shared.pointer_lock_shortcut.lock().unwrap() = "Ctrl+Alt+L".to_owned();
            rig.shared.shortcuts_revision.fetch_add(1, Ordering::SeqCst);
            rig.cycle(&[]);
        }
        for (code, value) in [
            (KEY_LEFTCTRL, 1),
            (KEY_LEFTALT, 1),
            (KEY_L, 1),
            (KEY_L, 0),
            (KEY_LEFTALT, 0),
            (KEY_LEFTCTRL, 0),
        ] {
            rig.type_key(code, value);
        }
    }

    #[test]
    fn locked_over_there_the_engine_routes_on_from_where_it_was_held() {
        let mut rig = Rig::new(true);
        rig.go_over();
        rig.move_by(10, 0);
        lock(&mut rig);
        rig.out.sent.clear();

        // Back across the border: held on There, at (10, 500).
        rig.move_by(-60, 0);
        assert!(rig.pipe.is_remote(), "the pointer did not come back");
        assert!(!rig.out.sent.contains(&Wire::Leave), "{:?}", rig.out.sent);
        // And routed on from there: no jump to where the engine had put it here.
        rig.move_by(5, 2);
        assert_eq!(rig.out.sent.last(), Some(&Wire::Motion { x: 15, y: 502 }));

        lock(&mut rig);
        rig.move_by(-60, 0);
        assert!(!rig.pipe.is_remote(), "unlocked, it comes back");
    }

    /// The maintainer's case: a French keyboard, `Alt+Shift+M`, pressed while the
    /// pointer is on the other machine. The key marked M is the one right of L.
    #[test]
    fn on_a_french_keyboard_alt_shift_m_locks_the_pointer_over_there() {
        use crate::hook::crossing::keys::Letters;
        const KEY_SEMICOLON: u16 = 39;
        const KEY_M: u16 = 50;
        let mut rig = Rig::new(true);
        *rig.shared.letters.lock().unwrap() = Letters::from_keysyms([
            (KEY_SEMICOLON, vec![u32::from(b'm'), u32::from(b'M')]),
            (KEY_M, vec![u32::from(b','), u32::from(b'?')]),
        ]);
        *rig.shared.pointer_lock_shortcut.lock().unwrap() = "Alt+Shift+M".to_owned();
        rig.shared.shortcuts_revision.fetch_add(1, Ordering::SeqCst);
        rig.cycle(&[]);
        rig.go_over();
        for (code, value) in [
            (KEY_LEFTALT, 1),
            (KEY_LEFTSHIFT, 1),
            (KEY_SEMICOLON, 1),
            (KEY_SEMICOLON, 0),
            (KEY_LEFTSHIFT, 0),
            (KEY_LEFTALT, 0),
        ] {
            rig.type_key(code, value);
        }
        rig.move_by(-60, 0);
        assert!(rig.pipe.is_remote(), "locked there, it stays there");
    }

    #[test]
    fn locked_here_the_border_is_a_wall_for_the_engine() {
        let mut rig = Rig::new(true);
        lock(&mut rig);
        // The wall goes up at the next cycle's look at the layout.
        rig.cycle(&[]);
        rig.move_by(30, 0);
        rig.move_by(300, 0);
        assert!(!rig.pipe.is_remote());
        assert!(rig.out.sent.is_empty(), "{:?}", rig.out.sent);
        assert_eq!(rig.local_cursor(), (1919, 500));
    }

    #[test]
    fn with_no_client_the_border_is_a_wall() {
        let mut rig = Rig::new(false);
        rig.move_by(30, 0);
        rig.move_by(300, 0);
        assert!(!rig.pipe.is_remote());
        assert!(rig.out.sent.is_empty());
        assert_eq!(rig.local_cursor(), (1919, 500));
    }

    #[test]
    fn a_client_that_goes_away_while_the_pointer_is_over_there_brings_it_home() {
        let mut rig = Rig::new(true);
        rig.go_over();
        rig.type_key(KEY_A, 1);
        rig.out.connected = false;
        rig.cycle(&[]);

        assert!(!rig.pipe.is_remote());
        assert_eq!(rig.out.releases, 1, "the keyboards are let go");
        assert_eq!(rig.out.shows, 1, "the cursor is shown again");
        assert_eq!(rig.local_cursor(), START, "back where it left");

        // And the border is a wall again.
        rig.move_by(300, 0);
        assert!(!rig.pipe.is_remote());
        assert_eq!(rig.local_cursor(), (1919, 500));
    }

    #[test]
    fn a_client_replaced_while_the_pointer_is_over_there_brings_it_home() {
        let mut rig = Rig::new(true);
        rig.go_over();
        rig.type_key(KEY_A, 1);
        assert!(rig.out.grabbed);
        // Between two cycles: the stream never looked unconnected.
        rig.out.client = 2;
        rig.out.sent.clear();
        rig.cycle(&[]);

        assert!(!rig.pipe.is_remote());
        assert!(!rig.out.grabbed, "the keyboards are let go");
        assert_eq!(rig.out.shows, 1, "the cursor is shown again");
        assert_eq!(rig.local_cursor(), START, "back where it left");
        assert!(
            rig.out
                .sent
                .iter()
                .all(|event| matches!(event, Wire::Leave)),
            "the new client got {:?} without an Enter",
            rig.out.sent
        );

        // And crossing again starts afresh with it. The first report after coming
        // home is where the engine starts tracking again, as on any homecoming.
        rig.out.sent.clear();
        rig.move_by(-10, 0);
        rig.go_over();
        assert!(matches!(rig.out.sent.first(), Some(Wire::Enter { .. })));
    }

    #[test]
    fn a_layout_without_the_machine_brings_the_pointer_home() {
        let mut rig = Rig::new(true);
        rig.go_over();
        let here_only = LAYOUT.replace(r#"Host="abcd""#, r#"Host="efgh""#);
        rig.shared
            .engine
            .lock()
            .unwrap()
            .load(ZonesLayout::from_xml(&here_only).unwrap());
        rig.cycle(&[]);

        assert!(!rig.pipe.is_remote());
        assert_eq!(rig.out.sent.last(), Some(&Wire::Leave));
        assert_eq!(rig.out.shows, 1);
        assert_eq!(rig.local_cursor(), START);
    }

    /// Every Load brings a layout with nothing of the other machines in play: one that
    /// lands between the start of a cycle and a report must not read as the machine
    /// having gone.
    #[test]
    fn a_layout_reloaded_in_the_middle_of_a_cycle_keeps_the_pointer_over_there() {
        let mut rig = Rig::new(true);
        rig.go_over();
        rig.shared
            .engine
            .lock()
            .unwrap()
            .load(ZonesLayout::from_xml(LAYOUT).unwrap());
        for ev in [rel(RelativeAxisCode::REL_X, 10), syn()] {
            let Pipeline { bufs, env, .. } = &mut rig.pipe;
            if bufs.push(ev, env, 0) == Frame::Complete {
                rig.flush();
            }
        }

        assert!(rig.pipe.is_remote());
        assert_eq!(rig.out.sent.last(), Some(&Wire::Motion { x: 10, y: 500 }));
    }

    #[test]
    fn a_button_pressed_here_is_released_here_even_from_over_there() {
        let mut rig = Rig::new(true);
        rig.cycle(&[key(BTN_LEFT, 1), syn()]);
        assert!(rig.out.pointer.last().unwrap().contains(&(1, BTN_LEFT, 1)));
        rig.go_over();
        rig.cycle(&[key(BTN_LEFT, 0), syn()]);

        assert!(
            !rig.out
                .sent
                .iter()
                .any(|e| matches!(e, Wire::Button { .. })),
            "{:?}",
            rig.out.sent
        );
        let frame = rig.out.pointer.last().unwrap();
        assert!(frame.contains(&(1, BTN_LEFT, 0)), "{frame:?}");
        assert_eq!(rig.local_cursor(), START, "the local cursor did not move");
    }

    #[test]
    fn a_hi_res_wheel_is_sent_in_its_own_120ths_and_a_plain_one_in_detents_times_120() {
        let mut rig = Rig::new(true);
        rig.go_over();
        rig.cycle(&[
            rel(RelativeAxisCode::REL_WHEEL, 1),
            rel(RelativeAxisCode::REL_WHEEL_HI_RES, 120),
            syn(),
        ]);
        rig.cycle(&[
            rel(RelativeAxisCode::REL_HWHEEL, -2),
            rel(RelativeAxisCode::REL_WHEEL_HI_RES, 30),
            syn(),
        ]);
        assert_eq!(
            rig.out.sent[1..],
            [
                Wire::Wheel {
                    horizontal: 0,
                    vertical: 120
                },
                Wire::Wheel {
                    horizontal: -240,
                    vertical: 30
                },
            ]
        );
        assert!(rig.out.pointer.is_empty());
    }

    #[test]
    fn a_layout_with_only_this_machine_routes_and_emits_as_before() {
        let mut rig = Rig::new(true);
        rig.shared.engine.lock().unwrap().load(
            ZonesLayout::from_xml(&LAYOUT.replace(r#" Host="abcd" HostX="3000" HostY="0""#, ""))
                .unwrap(),
        );
        rig.cycle(&[]);
        rig.move_by(-100, 3);
        assert_eq!(rig.local_cursor(), (1800, 503));
        rig.cycle(&[key(KeyCode::KEY_A.0, 1), syn()]);
        assert_eq!(rig.out.keyboard, [vec![(1, KEY_A, 1)]]);
        assert!(rig.out.sent.is_empty());
    }

    // --- the local cursor ----------------------------------------------------

    #[test]
    fn the_cursor_is_never_hidden_while_the_pointer_stays_here() {
        let mut rig = Rig::new(true);
        rig.move_by(-100, 3);
        rig.cycle(&[key(BTN_LEFT, 1), syn()]);
        rig.cycle(&[key(BTN_LEFT, 0), syn()]);
        let mut walled = Rig::new(false);
        walled.move_by(300, 0);
        let mut refused = Rig::new(true);
        refused.out.accepting = false;
        refused.move_by(30, 0);
        assert!(!refused.pipe.is_remote(), "an Enter nobody took is a wall");
        for rig in [&rig, &walled, &refused] {
            assert!(rig.out.hides.is_empty(), "{:?}", rig.out.hides);
            assert_eq!(rig.out.shows, 0);
        }
    }

    #[test]
    fn every_way_of_coming_home_shows_the_cursor_once() {
        type Home = fn(&mut Rig);
        let ways: [(&str, Home); 6] = [
            ("crossing back", |rig| rig.move_by(-40, 0)),
            ("no client", |rig| {
                rig.out.connected = false;
                rig.cycle(&[]);
            }),
            ("a failed write", |rig| {
                rig.out.accepting = false;
                rig.move_by(10, 0);
            }),
            ("a layout without the machine", |rig| {
                let other = LAYOUT.replace(r#"Host="abcd""#, r#"Host="efgh""#);
                rig.shared
                    .engine
                    .lock()
                    .unwrap()
                    .load(ZonesLayout::from_xml(&other).unwrap());
                rig.cycle(&[]);
            }),
            ("the rescue", |rig| {
                for code in [KEY_LEFTCTRL, KEY_LEFTALT, KEY_LEFTSHIFT, KEY_M] {
                    rig.type_key(code, 1);
                }
                rig.type_key(KEY_M, 2);
                rig.pipe
                    .check_rescue(rig.shared, Instant::now() + HOLD, &mut rig.out);
            }),
            ("the router going away", |rig| {
                rig.pipe.shut_down(&mut rig.out)
            }),
        ];
        for (way, home) in ways {
            let mut rig = Rig::new(true);
            rig.go_over();
            home(&mut rig);
            assert!(!rig.pipe.is_remote(), "{way}: home");
            assert_eq!(rig.out.shows, 1, "{way}: shown once");
            // Whatever comes after, here, shows nothing more and hides nothing.
            rig.move_by(-10, 0);
            rig.pipe.shut_down(&mut rig.out);
            assert_eq!(rig.out.shows, 1, "{way}: shown once only");
            assert_eq!(rig.out.hides.len(), 1, "{way}: hidden once, on the way out");
        }
    }

    #[test]
    fn a_button_released_here_while_away_is_released_where_the_frozen_pointer_hides() {
        let mut rig = Rig::new(true);
        rig.cycle(&[key(BTN_LEFT, 1), syn()]);
        rig.go_over();
        rig.out.spot = Some(Point::new(1915, 500));
        rig.cycle(&[]);
        rig.cycle(&[key(BTN_LEFT, 0), syn()]);
        let frame = rig.out.pointer.last().unwrap();
        assert!(frame.contains(&(1, BTN_LEFT, 0)), "{frame:?}");
        assert_eq!(rig.local_cursor(), (1915, 500));
    }

    // --- through a real input endpoint ---------------------------------------

    /// Injects nothing: on a master the endpoint's injecting side is idle.
    struct Nowhere;

    impl crate::remote::Injector for Nowhere {
        fn inject(&mut self, _: &[crate::remote::Injection]) {}
        fn park(&mut self) {}
        fn unpark(&mut self) {}
    }

    #[tokio::test]
    async fn through_the_endpoint_the_agent_reads_the_enter_and_its_leaving_brings_the_pointer_home(
    ) {
        let outlet = Arc::new(Outlet::default());
        let endpoint = crate::testing::endpoint();
        let _server = crate::ipc::input::start(endpoint.clone(), Box::new(Nowhere), outlet.clone())
            .expect("binds");
        let mut agent =
            tokio::time::timeout(Duration::from_secs(5), crate::testing::connect(&endpoint))
                .await
                .expect("the endpoint accepts");

        let mut driver = Driver::default();
        let deadline = Instant::now() + Duration::from_secs(5);
        while !driver.connected() {
            assert!(Instant::now() < deadline, "the client was never published");
            tokio::time::sleep(Duration::from_millis(5)).await;
            driver.refresh(&outlet);
        }

        let mut rig = Rig::with_driver(true, Some((driver, outlet)));
        rig.go_over();
        rig.move_by(10, 0);
        let batch = tokio::time::timeout(Duration::from_secs(5), read_batch(&mut agent))
            .await
            .expect("a batch arrives")
            .expect("read");
        assert_eq!(decode(&batch).unwrap(), [Wire::Enter { x: 0, y: 500 }]);
        let batch = read_batch(&mut agent).await.expect("read");
        assert_eq!(decode(&batch).unwrap(), [Wire::Motion { x: 10, y: 500 }]);

        drop(agent);
        let deadline = Instant::now() + Duration::from_secs(5);
        while rig.pipe.is_remote() {
            assert!(Instant::now() < deadline, "the pointer never came home");
            tokio::time::sleep(Duration::from_millis(5)).await;
            rig.cycle(&[]);
        }
        assert_eq!(rig.local_cursor(), START);
        assert_eq!(rig.out.releases, 1);
        assert_eq!(rig.out.shows, 1);
    }

    #[tokio::test]
    async fn through_the_endpoint_a_client_replaced_between_two_cycles_is_sent_an_enter_before_anything_else(
    ) {
        let outlet = Arc::new(Outlet::default());
        let endpoint = crate::testing::endpoint();
        let _server = crate::ipc::input::start(endpoint.clone(), Box::new(Nowhere), outlet.clone())
            .expect("binds");
        let mut first =
            tokio::time::timeout(Duration::from_secs(5), crate::testing::connect(&endpoint))
                .await
                .expect("the endpoint accepts");

        let mut driver = Driver::default();
        let deadline = Instant::now() + Duration::from_secs(5);
        while !driver.connected() {
            assert!(Instant::now() < deadline, "the client was never published");
            tokio::time::sleep(Duration::from_millis(5)).await;
            driver.refresh(&outlet);
        }
        let before = driver.client();

        let mut rig = Rig::with_driver(true, Some((driver, outlet.clone())));
        rig.go_over();
        let batch = tokio::time::timeout(Duration::from_secs(5), read_batch(&mut first))
            .await
            .expect("a batch arrives")
            .expect("read");
        assert_eq!(decode(&batch).unwrap(), [Wire::Enter { x: 0, y: 500 }]);

        // The agent's restart: the stream closed and another opened, with no cycle in
        // between to see it unconnected.
        drop(first);
        let mut second =
            tokio::time::timeout(Duration::from_secs(5), crate::testing::connect(&endpoint))
                .await
                .expect("the endpoint accepts again");
        let mut watcher = Driver::default();
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            watcher.refresh(&outlet);
            if watcher.client().is_some_and(|id| Some(id) != before) {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "the successor was never published"
            );
            tokio::time::sleep(Duration::from_millis(5)).await;
        }

        rig.move_by(11, 0);
        assert!(!rig.pipe.is_remote(), "the crossing ended with its client");
        assert!(!rig.out.grabbed, "the keyboards are let go");
        assert_eq!(rig.out.shows, 1);

        // Whatever the successor reads first, it is not input without an `Enter`.
        rig.go_over();
        rig.move_by(10, 0);
        let mut seen = Vec::new();
        while !seen
            .iter()
            .any(|event| matches!(event, Wire::Motion { .. }))
        {
            let batch = tokio::time::timeout(Duration::from_secs(5), read_batch(&mut second))
                .await
                .expect("a batch arrives")
                .expect("read");
            seen.extend(decode(&batch).unwrap());
        }
        let first_input = seen
            .iter()
            .position(|event| !matches!(event, Wire::Leave))
            .expect("something besides a Leave");
        assert!(
            matches!(seen[first_input], Wire::Enter { .. }),
            "the successor read {seen:?}"
        );
    }

    /// Here (0..1920), then A (abcd, 3000..4920), then B (efgh, 6000..7920), each
    /// border linked both ways.
    const THREE_MACHINES: &str = concat!(
        r#"<ZonesLayout Algorithm="Strait"><MainZones>"#,
        r#"<Zone Id="0" Name="Here"><PixelsBounds><Rect Left="0" Top="0" Width="1920" Height="1080"></Rect></PixelsBounds><PhysicalBounds><Rect Left="0" Top="0" Width="527" Height="296"></Rect></PhysicalBounds>"#,
        r#"<RightLinks><ZoneLink From="0" To="296" SourceFromPixel="0" SourceToPixel="1080" TargetFromPixel="0" TargetToPixel="1080" BorderResistance="0" TargetId="1"></ZoneLink></RightLinks></Zone>"#,
        r#"<Zone Id="1" Name="A" Host="abcd" HostX="3000" HostY="0"><PixelsBounds><Rect Left="3000" Top="0" Width="1920" Height="1080"></Rect></PixelsBounds><PhysicalBounds><Rect Left="527" Top="0" Width="527" Height="296"></Rect></PhysicalBounds>"#,
        r#"<LeftLinks><ZoneLink From="0" To="296" SourceFromPixel="0" SourceToPixel="1080" TargetFromPixel="0" TargetToPixel="1080" BorderResistance="0" TargetId="0"></ZoneLink></LeftLinks>"#,
        r#"<RightLinks><ZoneLink From="0" To="296" SourceFromPixel="0" SourceToPixel="1080" TargetFromPixel="0" TargetToPixel="1080" BorderResistance="0" TargetId="2"></ZoneLink></RightLinks></Zone>"#,
        r#"<Zone Id="2" Name="B" Host="efgh" HostX="6000" HostY="0"><PixelsBounds><Rect Left="6000" Top="0" Width="1920" Height="1080"></Rect></PixelsBounds><PhysicalBounds><Rect Left="1054" Top="0" Width="527" Height="296"></Rect></PhysicalBounds>"#,
        r#"<LeftLinks><ZoneLink From="0" To="296" SourceFromPixel="0" SourceToPixel="1080" TargetFromPixel="0" TargetToPixel="1080" BorderResistance="0" TargetId="1"></ZoneLink></LeftLinks></Zone>"#,
        r#"</MainZones></ZonesLayout>"#,
    );

    fn hosts_of(routed: &[(Option<String>, Wire)], wanted: fn(&Wire) -> bool) -> Vec<String> {
        routed
            .iter()
            .filter(|(_, e)| wanted(e))
            .map(|(h, _)| h.clone().unwrap_or_default())
            .collect()
    }

    #[test]
    fn evdev_takes_the_pointer_from_one_remote_straight_to_the_next_and_back_home() {
        let mut rig = Rig::build(true, None, THREE_MACHINES, Some(&["abcd", "efgh"]));
        rig.go_over();
        rig.type_key(KEY_LEFTSHIFT, 1);
        for _ in 0..6 {
            rig.move_by(500, 0);
        }
        let routed = std::mem::take(&mut rig.out.routed);
        let enters = hosts_of(&routed, |e| matches!(e, Wire::Enter { .. }));
        let leaves = hosts_of(&routed, |e| matches!(e, Wire::Leave));
        assert_eq!(enters, ["abcd", "efgh"], "{routed:?}");
        assert_eq!(leaves, ["abcd"]);
        assert!(rig.out.grabbed, "still grabbed on B");
        assert_eq!(rig.out.shows, 0);
        assert!(
            rig.out.pointer.is_empty(),
            "nothing here: {:?}",
            rig.out.pointer
        );
        // The Shift went to A, whose Leave released it; its release goes to nobody.
        rig.type_key(KEY_LEFTSHIFT, 0);
        assert!(rig.out.routed.is_empty(), "{:?}", rig.out.routed);

        for _ in 0..12 {
            rig.move_by(-500, 0);
        }
        assert!(!rig.pipe.is_remote(), "home");
        let leaves = hosts_of(&rig.out.routed, |e| matches!(e, Wire::Leave));
        assert_eq!(leaves, ["efgh", "abcd"], "{:?}", rig.out.routed);
        assert_eq!(rig.out.shows, 1);
        assert!(!rig.out.grabbed);
    }

    #[test]
    fn evdev_walls_a_machine_the_agent_does_not_drive() {
        let mut rig = Rig::build(true, None, THREE_MACHINES, Some(&["abcd"]));
        rig.go_over();
        for _ in 0..6 {
            rig.move_by(500, 0);
        }
        assert!(rig.pipe.is_remote());
        let enters = hosts_of(&rig.out.routed, |e| matches!(e, Wire::Enter { .. }));
        assert_eq!(enters, ["abcd"], "{:?}", rig.out.routed);
    }
}
