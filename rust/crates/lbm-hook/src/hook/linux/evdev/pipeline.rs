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
//! says where the report goes: a position on this machine is emitted to the virtual
//! pointer exactly as it always was; a position in another machine's zone is
//! translated to that machine's own desktop pixels and written to the agent as an
//! `lbm_ipc::input` batch. Another machine's desktop lies outside this one's (each is
//! offset to the right of everything else), so the cursor's extent grows to cover them
//! while they are in play, and what is emitted here is clamped to this desktop and,
//! while the pointer is away, frozen where it left: the local cursor stays where it
//! was, at [`Pipeline::local_pos`].
//!
//! Crossing out: `Enter` first, at the landing pixel; then nothing more to the virtual
//! pointer, and the keyboards are grabbed so that typing follows the pointer. Crossing
//! back: `Leave`, the keyboards are let go, and emission resumes here from where the
//! engine landed.
//!
//! ## The local cursor
//!
//! The compositor would go on drawing the frozen pointer, so it is hidden while the
//! pointer is away ([`super::hider`]): [`Outputs::hide_cursor`] once the `Enter` is
//! queued, [`Outputs::show_cursor`] every time the pointer comes home, whichever way.
//! Once the surface that hides it is in place, the frozen pointer is moved onto it
//! ([`Outputs::cursor_hiding_spot`]), a few pixels inside the edge it left by, and
//! stays there; coming home starts from where it left, not from there.
//!
//! ## Held keys and buttons
//!
//! One rule, in both directions: **a release goes where its press went.** Each side
//! then sees a press and its release, never one without the other, whatever the
//! pointer did in between.
//!
//! * A button or key pressed here and held across the border is released here: a
//!   drag that reaches the border ends at the border when the button comes up, and the
//!   other machine never sees a release it had no press for. Its own motion meanwhile
//!   goes on over there, buttonless. Dragging across machines is not supported
//!   (docs/multi-pc.md), and this is the form of "not supported" that leaves nothing
//!   stuck.
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
//! * Key autorepeat is forwarded as a repeated press, which the remote turns into its
//!   own repeat; button repeats do not exist and are dropped.
//!
//! ## Coming home
//!
//! When a hop of the input stream ends, the pointer is home (docs/multi-pc.md): the
//! virtual cursor goes back to the last position it had here, the keyboards are let go
//! and the engine forgets it was elsewhere. That happens when the client goes away,
//! when a batch cannot be queued (no client, a connection that closed, or a full queue
//! — which also closes the connection, [`crate::ipc::input::Driver::send`]), when a
//! reloaded layout no longer has the machine the pointer is on, when the rescue
//! shortcut fires, and when the router goes away, which is how Stop, pause, suspend,
//! the rescue's unhook and quit all end ([`Pipeline::shut_down`]). Without a client,
//! the other machines' zones are not in play and their borders are walls. `Leave` is
//! sent whenever it can be; when it cannot, the connection is closing, and its end
//! means the same thing to the remote.
//!
//! ## The rescue
//!
//! With the keyboards grabbed the portal's global shortcut never fires: the compositor
//! sees no key. The configured shortcut is therefore watched for in the grabbed stream
//! ([`crate::hook::crossing::keys::RescueKeys`]), with the same hold, and fires the same
//! `daemon::rescue_fired` after bringing the pointer home. Its final key is kept back
//! while it is held, so the other machine never sees it; given up early, it is
//! forwarded late, in order.
//!
//! ## One remote
//!
//! Batches carry no destination: the agent relays them to the one remote it drives. A
//! layout with zones of two machines is routed over both by the engine, but crossing
//! from one to the other is treated as leaving the one being driven, and the pointer
//! comes home.
//!
//! ## Cost
//!
//! The path where the pointer stays here allocates nothing and adds a containment test
//! to each report (the host lookup only runs for a position outside this desktop) and
//! a comparison to each cycle. A report sent to another machine allocates its batch.

use std::sync::TryLockError;
use std::time::Instant;

use evdev::{EventType, InputEvent};
use lbm_ipc::input::InputEvent as Wire;

use crate::engine::cursor::CursorEnv;
use crate::engine::MouseEngine;
use crate::geometry::{Point, Rect};
use crate::hook::hot_path::{count_event, route_move_then, Routed};
use crate::shared::Shared;
use crate::zones::HostPoint;

use super::cursor::EvdevCursor;
use super::frame::PumpBuffers;
use super::probe::{extents, try_device_bounds};
use super::{BTN_MOUSE_RANGE, BTN_RANGE, BTN_TRIGGER_HAPPY_RANGE};
use crate::hook::crossing::keys::{HeldSet, RescueKeys, Verdict};

/// Everything a cycle writes to: this machine's two virtual devices, the input stream,
/// and the keyboards' exclusivity. The router's implementation owns the devices; the
/// tests' records what it is handed.
pub(crate) trait Outputs {
    /// One frame for the virtual pointer.
    fn pointer(&mut self, frame: &[InputEvent]);
    /// One frame for the virtual keyboard.
    fn keyboard(&mut self, frame: &[InputEvent]);
    /// The input stream's client to drive another machine through, if it has one. A
    /// client that replaced another has another value.
    fn remote_client(&self) -> Option<u64>;
    /// Whether the input stream has a client to drive another machine through.
    fn remote_connected(&self) -> bool {
        self.remote_client().is_some()
    }
    /// Queues one encoded batch without waiting; `false` when it will never be sent.
    fn send_remote(&mut self, batch: Vec<u8>) -> bool;
    /// Grabs every keyboard that has no key held and is not grabbed yet.
    fn grab_keyboards(&mut self);
    /// Lets every keyboard go.
    fn release_keyboards(&mut self);
    /// The pointer went to another machine from `from`, where it is frozen: hide this
    /// machine's cursor.
    fn hide_cursor(&mut self, from: Point<i32>);
    /// Where the frozen pointer is to go to be under what hides the cursor, once that
    /// is in place; `None` until then, and when nothing hides it.
    fn cursor_hiding_spot(&mut self) -> Option<Point<i32>>;
    /// The pointer is home: show this machine's cursor.
    fn show_cursor(&mut self);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Side {
    Local,
    Remote,
}

/// Where a routed position is, as far as the next frame is concerned.
#[derive(Debug, PartialEq)]
enum Place {
    Local,
    /// On another machine, the pointer coming from here.
    Enter {
        pixel: Point<i32>,
        host: String,
    },
    /// On the machine being driven.
    Remote(Point<i32>),
    /// Neither here nor on the machine being driven.
    Away,
}

/// The pump's state between devices and outputs.
pub(crate) struct Pipeline {
    pub(super) bufs: PumpBuffers,
    pub(super) env: EvdevCursor,
    sens: f64,
    /// Sub-pixel remainder of the sensitivity-scaled raw motion integration.
    rem: (f64, f64),
    debug: bool,
    side: Side,
    /// The last position the pointer had on this machine: where the local cursor
    /// stays while the pointer is away, and where it comes back to.
    home: Point<i32>,
    /// The last pixel sent to the other machine.
    sent: Point<i32>,
    /// Where the frozen pointer was moved to be under what hides the cursor, while the
    /// pointer is away.
    parked: Option<Point<i32>>,
    /// The machine being driven.
    host: String,
    /// The input stream's client the `Enter` went to: the only one that knows the
    /// pointer is over there.
    client: Option<u64>,
    /// Buttons and keys pressed on the other machine and not released there yet.
    remote_held: HeldSet,
    /// The engine still tracks the pointer where it was before it came home, and has
    /// to be told before it routes again (`MouseEngine::begin_touch`).
    resync: bool,
    rescue: RescueKeys,
    /// The batch being composed; reused.
    wire: Vec<Wire>,
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
            side: Side::Local,
            home,
            sent: home,
            parked: None,
            host: String::new(),
            client: None,
            remote_held: HeldSet::default(),
            resync: false,
            rescue: RescueKeys::default(),
            wire: Vec::with_capacity(16),
        }
    }

    pub(super) fn is_remote(&self) -> bool {
        self.side == Side::Remote
    }

    /// Where the pointer is on this machine: the virtual cursor, or where it left
    /// from while it is away.
    pub(super) fn local_pos(&self) -> Point<i32> {
        match self.side {
            Side::Local => self.env.virtual_pos,
            Side::Remote => self.home,
        }
    }

    /// The local position plus the pending buttons and wheels as one frame of the
    /// virtual pointer. Clamped to the desktop the device spans: nothing emitted here
    /// may land outside it, whatever the virtual cursor is doing.
    pub(super) fn emit_local(&mut self, out: &mut impl Outputs) {
        // A frame while the pointer is away — a release of a button pressed here — must
        // not take it off what hides the cursor.
        let p = match self.side {
            Side::Remote => self.parked.unwrap_or(self.home),
            Side::Local => self.local_pos(),
        };
        let d = self.env.device;
        let ax = p.x().min(d.right() - 1).max(d.left()) - d.left();
        let ay = p.y().min(d.bottom() - 1).max(d.top()) - d.top();
        out.pointer(self.bufs.pointer_frame(ax, ay));
    }

    /// The start of a cycle: catch up with the input stream's client and the layout,
    /// and come home if either says so. Returns the desktop the virtual pointer should
    /// span, when the locks could be taken without waiting.
    pub(super) fn begin_cycle(
        &mut self,
        shared: &'static Shared,
        out: &mut impl Outputs,
    ) -> Option<Rect<i32>> {
        let client = out.remote_client();
        let connected = client.is_some();
        if self.is_remote() && !connected {
            eprintln!(
                "[LittleBigMouse.Hook] evdev: the input stream has no client, the pointer comes home"
            );
            self.go_home(None, out);
        } else if self.is_remote() && client != self.client {
            // The agent closed the stream and opened another between two cycles — it
            // does after the other machine dropped input. The new client never read
            // the `Enter`: what it is sent now would be ignored, with the keyboards
            // still grabbed here. Its `Leave` is one it has nothing to do with.
            eprintln!(
                "[LittleBigMouse.Hook] evdev: the input stream has another client, the pointer comes home"
            );
            self.go_home(None, out);
        }
        self.check_rescue(shared, Instant::now(), out);

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
            engine.layout.set_remote_enterable(connected);
            if self.is_remote() {
                let here = engine
                    .layout
                    .host_point(self.env.virtual_pos)
                    .is_some_and(|p| p.host == Some(self.host.as_str()));
                if !here {
                    eprintln!(
                        "[LittleBigMouse.Hook] evdev: the layout no longer has the machine the pointer is on, the pointer comes home"
                    );
                    self.go_home(Some(&mut engine), out);
                }
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

    /// The end of a cycle: a keyboard that had a key held when the pointer left can be
    /// grabbed once it has none; and a rescue held long enough fires.
    pub(super) fn end_cycle(&mut self, shared: &'static Shared, out: &mut impl Outputs) {
        if self.is_remote() {
            out.grab_keyboards();
            if self.parked.is_none() {
                if let Some(spot) = out.cursor_hiding_spot() {
                    eprintln!(
                        "[LittleBigMouse.Hook] evdev: the frozen pointer moves to ({},{}), onto what hides it",
                        spot.x(),
                        spot.y()
                    );
                    self.parked = Some(spot);
                    self.emit_local(out);
                }
            }
        }
        self.check_rescue(shared, Instant::now(), out);
    }

    /// The router is going away — Stop, pause, suspend, the rescue's unhook, quit: the
    /// pointer comes home while the devices are still there to say so.
    pub(super) fn shut_down(&mut self, out: &mut impl Outputs) {
        if self.is_remote() {
            self.go_home(None, out);
        }
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
                        engine.layout.set_remote_enterable(out.remote_connected());
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
                let remote = self.is_remote();
                let Pipeline { env, host, .. } = self;
                route_move_then(&shared.engine, env, candidate, |engine, env, handled| {
                    locate(engine, env, candidate, handled, remote, host)
                })
            };
            match routed {
                (Routed::Contended, _) if !self.is_remote() => {
                    self.env.virtual_pos = self.env.clamp(candidate);
                    self.home = self.env.virtual_pos;
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

    /// Sends the frame where `place` says the pointer now is, crossing to or from
    /// another machine first when it has just done so. `None`: it did not move.
    fn dispatch(&mut self, shared: &'static Shared, place: Option<Place>, out: &mut impl Outputs) {
        match (self.side, place) {
            (Side::Local, Some(Place::Enter { pixel, host })) => {
                if !(out.remote_connected() && self.enter(shared, pixel, host, out)) {
                    // Nobody to take it — the client left between the cycle's start
                    // and this report: the border is a wall.
                    self.go_home(None, out);
                    self.local_leftovers(out);
                }
            }
            (Side::Local, _) => self.local_frame(out),
            (Side::Remote, None) => self.remote_frame(None, out),
            (Side::Remote, Some(Place::Remote(pixel))) => self.remote_frame(Some(pixel), out),
            (Side::Remote, Some(Place::Local)) => {
                // The engine brought it back across a border: it landed where it is.
                self.wire.clear();
                self.wire.push(Wire::Leave);
                self.send(out);
                self.leave(out);
                self.local_frame(out);
            }
            (Side::Remote, Some(Place::Enter { .. } | Place::Away)) => {
                self.go_home(None, out);
                self.local_leftovers(out);
            }
        }
    }

    /// The ordinary frame: everything pending, to this machine's devices.
    fn local_frame(&mut self, out: &mut impl Outputs) {
        self.home = self.env.virtual_pos;
        self.emit_local(out);
        self.bufs.take_keyboard_frame(|frame| out.keyboard(frame));
    }

    /// Crossing out. `false` when the `Enter` could not be queued: the pointer stays.
    fn enter(
        &mut self,
        shared: &Shared,
        pixel: Point<i32>,
        host: String,
        out: &mut impl Outputs,
    ) -> bool {
        // Read when it may be needed, and never waited for: a Load holding the lock
        // leaves the shortcut that was last read.
        if let Ok(text) = shared.rescue_shortcut.try_lock() {
            self.rescue.set(&text);
        }
        self.rescue.reset();
        self.side = Side::Remote;
        self.host = host;
        self.client = out.remote_client();
        self.sent = pixel;
        self.wire.clear();
        self.wire.push(Wire::Enter {
            x: pixel.x(),
            y: pixel.y(),
        });
        self.take_remote_events(Instant::now());
        if !self.send(out) {
            self.side = Side::Local;
            self.remote_held.clear();
            return false;
        }
        eprintln!(
            "[LittleBigMouse.Hook] evdev: the pointer is on {}",
            self.host
        );
        self.parked = None;
        out.hide_cursor(self.home);
        out.grab_keyboards();
        self.local_leftovers(out);
        true
    }

    /// A frame while the pointer is away: its motion, and whatever it pressed or
    /// released there, to the other machine; what it released of this one, here.
    fn remote_frame(&mut self, pixel: Option<Point<i32>>, out: &mut impl Outputs) {
        self.wire.clear();
        if let Some(pixel) = pixel.filter(|&p| p != self.sent) {
            self.sent = pixel;
            self.wire.push(Wire::Motion {
                x: pixel.x(),
                y: pixel.y(),
            });
        }
        self.take_remote_events(Instant::now());
        if !self.send(out) {
            eprintln!(
                "[LittleBigMouse.Hook] evdev: the input stream took no more, the pointer comes home"
            );
            self.go_home(None, out);
        }
        self.local_leftovers(out);
    }

    /// Emits what a frame left for this machine while the pointer is away — releases
    /// of what was pressed here — if anything.
    fn local_leftovers(&mut self, out: &mut impl Outputs) {
        if self.bufs.pointer_pending() {
            self.emit_local(out);
        }
        self.bufs.take_keyboard_frame(|frame| out.keyboard(frame));
    }

    /// Moves this frame's buttons, wheels and keys that belong to the other machine
    /// into the batch, leaving in the frame those that belong here.
    fn take_remote_events(&mut self, now: Instant) {
        let (horizontal, vertical) = self.bufs.wheel_120ths();
        let Pipeline {
            bufs,
            wire,
            remote_held,
            rescue,
            ..
        } = self;
        bufs.retain_local(|ev| match ev.event_type() {
            // Wheels are counted above; scancodes describe this machine's keyboard.
            EventType::RELATIVE | EventType::MISC => false,
            EventType::KEY if BTN_MOUSE_RANGE.contains(&ev.code()) => {
                !route_button(wire, remote_held, ev.code(), ev.value())
            }
            // Tablet tools and joystick buttons mean nothing over there.
            EventType::KEY
                if BTN_RANGE.contains(&ev.code())
                    || BTN_TRIGGER_HAPPY_RANGE.contains(&ev.code()) =>
            {
                true
            }
            EventType::KEY => !route_key(wire, remote_held, rescue, ev.code(), ev.value(), now),
            _ => true,
        });
        if (horizontal, vertical) != (0, 0) {
            wire.push(Wire::Wheel {
                horizontal,
                vertical,
            });
        }
    }

    /// One key of an observed keyboard. `grabbed`: the compositor does not see it, so
    /// it is this process's to forward.
    pub(super) fn keyboard_key(&mut self, code: u16, value: i32, grabbed: bool, now: Instant) {
        self.env.track_ctrl(code, value);
        if grabbed && self.is_remote() {
            // Not taken: the release of a key this stream never pressed, which has
            // nowhere to go — the compositor cannot receive from a grabbed keyboard.
            route_key(
                &mut self.wire,
                &mut self.remote_held,
                &mut self.rescue,
                code,
                value,
                now,
            );
        }
    }

    /// Sends the keys [`Self::keyboard_key`] collected from one keyboard's drain.
    pub(super) fn flush_keys(&mut self, shared: &'static Shared, out: &mut impl Outputs) {
        if self.wire.is_empty() {
            return;
        }
        if !self.send(out) {
            eprintln!(
                "[LittleBigMouse.Hook] evdev: the input stream took no more, the pointer comes home"
            );
            self.go_home(None, out);
        }
        self.check_rescue(shared, Instant::now(), out);
    }

    /// Fires the rescue if its shortcut has been held long enough in the grabbed
    /// stream: home first, so that the `Leave` releases the modifiers over there.
    pub(super) fn check_rescue(
        &mut self,
        shared: &'static Shared,
        now: Instant,
        out: &mut impl Outputs,
    ) {
        if self.is_remote() && self.rescue.due(now) {
            self.go_home(None, out);
            crate::daemon::rescue_fired(shared);
        }
    }

    /// Encodes and queues the batch being composed. An empty one is not sent.
    fn send(&mut self, out: &mut impl Outputs) -> bool {
        if self.wire.is_empty() {
            return true;
        }
        let mut batch = Vec::with_capacity(self.wire.len() * 9);
        for event in &self.wire {
            event.encode(&mut batch);
        }
        self.wire.clear();
        out.send_remote(batch)
    }

    /// The other machine is no longer driven: what it holds pressed, its `Leave`
    /// released.
    fn leave(&mut self, out: &mut impl Outputs) {
        self.side = Side::Local;
        self.remote_held.clear();
        self.rescue.reset();
        self.parked = None;
        out.show_cursor();
        out.release_keyboards();
        eprintln!("[LittleBigMouse.Hook] evdev: the pointer is back");
    }

    /// Brings the pointer back to the last position it had here, whatever the engine
    /// thought. `engine` when its lock is already held; otherwise the engine is told
    /// before it routes again.
    fn go_home(&mut self, engine: Option<&mut MouseEngine>, out: &mut impl Outputs) {
        if self.is_remote() {
            self.wire.clear();
            self.wire.push(Wire::Leave);
            // When this fails the connection is closing, and its end is a Leave too.
            self.send(out);
            self.leave(out);
        }
        self.env.virtual_pos = self.home;
        self.env.release_clip();
        match engine {
            Some(engine) => self.resync_with(engine),
            None => self.resync = true,
        }
        self.emit_local(out);
    }

    /// Drops the engine's crossing history and puts the cursor back at home: the next
    /// report starts afresh from the zone it is in.
    fn resync_with(&mut self, engine: &mut MouseEngine) {
        engine.begin_touch(&mut self.env);
        self.env.virtual_pos = self.home;
        self.resync = false;
    }
}

/// Where a routed report leaves the pointer. Only a position outside this desktop is
/// looked up: the other machines' desktops lie outside it, so a position inside is
/// this machine's without asking.
fn locate(
    engine: &MouseEngine,
    env: &EvdevCursor,
    candidate: Point<i32>,
    handled: bool,
    remote: bool,
    host: &str,
) -> Place {
    // What the caller will commit: the engine's verdict, or the clamped candidate.
    let pos = if handled {
        env.virtual_pos
    } else {
        env.clamp(candidate)
    };
    if !remote && env.device.contains(pos) {
        return Place::Local;
    }
    match engine.layout.host_point(pos) {
        Some(HostPoint { host: None, .. }) => Place::Local,
        Some(HostPoint {
            host: Some(there),
            pixel,
        }) if remote => {
            if there == host {
                Place::Remote(pixel)
            } else {
                Place::Away
            }
        }
        Some(HostPoint {
            host: Some(there),
            pixel,
        }) => Place::Enter {
            pixel,
            host: there.to_owned(),
        },
        None if remote => Place::Away,
        None => Place::Local,
    }
}

/// A mouse button while the pointer is away: `true` when it went into the batch.
fn route_button(wire: &mut Vec<Wire>, held: &mut HeldSet, code: u16, value: i32) -> bool {
    match value {
        1 => {
            held.insert(code);
            wire.push(Wire::Button {
                code,
                pressed: true,
            });
            true
        }
        // Released where it was pressed.
        0 if held.remove(code) => {
            wire.push(Wire::Button {
                code,
                pressed: false,
            });
            true
        }
        0 => false,
        // A repeat goes where its press went; a button's means nothing anywhere.
        _ => held.contains(code),
    }
}

/// A key of the grabbed stream while the pointer is away: `true` when it was taken —
/// into the batch, or kept back as the rescue's final key.
fn route_key(
    wire: &mut Vec<Wire>,
    held: &mut HeldSet,
    rescue: &mut RescueKeys,
    code: u16,
    value: i32,
    now: Instant,
) -> bool {
    match rescue.filter(code, value, now) {
        Verdict::Hold => return true,
        Verdict::Release(kept) => {
            held.insert(kept);
            wire.push(Wire::Key {
                code: kept,
                pressed: true,
            });
        }
        Verdict::Pass => {}
    }
    match value {
        1 => {
            held.insert(code);
            wire.push(Wire::Key {
                code,
                pressed: true,
            });
            true
        }
        0 if held.remove(code) => {
            wire.push(Wire::Key {
                code,
                pressed: false,
            });
            true
        }
        0 => false,
        // Autorepeat: the remote repeats an injected key only when told to.
        _ if held.contains(code) => {
            wire.push(Wire::Key {
                code,
                pressed: true,
            });
            true
        }
        _ => false,
    }
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
    use lbm_ipc::input::{decode, read_batch};

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
            self.sent
                .extend(decode(&batch).expect("a batch that decodes"));
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
            let shared: &'static Shared = Box::leak(Box::new(Shared::new()));
            shared
                .engine
                .lock()
                .unwrap()
                .load(ZonesLayout::from_xml(LAYOUT).unwrap());
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
    fn keys_go_to_the_other_machine_only_while_the_pointer_is_there() {
        let mut rig = Rig::new(true);
        rig.type_key(KEY_A, 1);
        rig.type_key(KEY_A, 0);
        assert!(rig.out.sent.is_empty(), "typed here: {:?}", rig.out.sent);

        rig.go_over();
        rig.type_key(KEY_A, 1);
        rig.type_key(KEY_A, 2);
        rig.type_key(KEY_A, 0);
        assert_eq!(
            rig.out.sent[1..],
            [
                Wire::Key {
                    code: KEY_A,
                    pressed: true
                },
                Wire::Key {
                    code: KEY_A,
                    pressed: true
                },
                Wire::Key {
                    code: KEY_A,
                    pressed: false
                },
            ]
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
    fn a_stream_that_takes_no_more_brings_the_pointer_home() {
        let mut rig = Rig::new(true);
        rig.go_over();
        rig.out.accepting = false;
        rig.move_by(10, 0);

        assert!(!rig.pipe.is_remote());
        assert_eq!(rig.out.releases, 1);
        assert_eq!(rig.out.shows, 1);
        assert_eq!(rig.local_cursor(), START);
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
    fn going_away_brings_the_pointer_home_first() {
        let mut rig = Rig::new(true);
        rig.go_over();
        rig.pipe.shut_down(&mut rig.out);
        assert!(!rig.pipe.is_remote());
        assert_eq!(rig.out.sent.last(), Some(&Wire::Leave));
        assert_eq!(rig.out.releases, 1);
        assert_eq!(rig.out.shows, 1);
        assert_eq!(rig.local_cursor(), START);
    }

    #[test]
    fn the_rescue_held_over_there_brings_the_pointer_home_and_never_sends_its_key() {
        let mut rig = Rig::new(true);
        rig.go_over();
        for code in [KEY_LEFTCTRL, KEY_LEFTALT, KEY_LEFTSHIFT] {
            rig.type_key(code, 1);
        }
        rig.type_key(KEY_M, 1);
        rig.type_key(KEY_M, 2);
        rig.pipe
            .check_rescue(rig.shared, Instant::now() + HOLD, &mut rig.out);

        assert!(!rig.pipe.is_remote());
        assert!(
            !rig.shared.want_hook.load(Ordering::SeqCst),
            "the rescue's unhook was asked for"
        );
        assert_eq!(rig.out.sent.last(), Some(&Wire::Leave));
        assert!(
            !rig.out
                .sent
                .iter()
                .any(|e| matches!(e, Wire::Key { code: KEY_M, .. })),
            "{:?}",
            rig.out.sent
        );
        assert_eq!(rig.out.releases, 1);
        assert_eq!(rig.out.shows, 1);
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
    fn a_button_pressed_over_there_is_released_there_by_the_leave() {
        let mut rig = Rig::new(true);
        rig.go_over();
        rig.cycle(&[key(BTN_LEFT, 1), syn()]);
        rig.move_by(-40, 0);
        rig.cycle(&[key(BTN_LEFT, 0), syn()]);

        assert_eq!(
            rig.out.sent,
            [
                Wire::Enter { x: 0, y: 500 },
                Wire::Button {
                    code: BTN_LEFT,
                    pressed: true
                },
                Wire::Leave,
            ]
        );
    }

    #[test]
    fn a_key_held_over_there_is_released_there_by_the_leave_and_not_again() {
        let mut rig = Rig::new(true);
        rig.go_over();
        rig.type_key(KEY_LEFTCTRL, 1);
        rig.move_by(-40, 0);
        rig.type_key(KEY_LEFTCTRL, 0);

        assert_eq!(
            rig.out.sent,
            [
                Wire::Enter { x: 0, y: 500 },
                Wire::Key {
                    code: KEY_LEFTCTRL,
                    pressed: true
                },
                Wire::Leave,
            ]
        );
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
    fn the_cursor_is_hidden_once_on_crossing_out_after_the_enter_is_queued() {
        let mut rig = Rig::new(true);
        rig.go_over();
        rig.move_by(10, 4);
        rig.move_by(10, 4);
        // From where the pointer last was here; the Enter was the first thing sent.
        assert_eq!(rig.out.hides, [(START, 1)]);
        assert_eq!(rig.out.shows, 0);
    }

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
    fn the_frozen_pointer_moves_onto_what_hides_it_once_it_is_in_place_and_comes_home_from_where_it_left(
    ) {
        let mut rig = Rig::new(true);
        rig.go_over();
        rig.move_by(5, 0);
        assert!(rig.out.pointer.is_empty(), "nothing in place yet");

        rig.out.spot = Some(Point::new(1915, 500));
        rig.cycle(&[]);
        assert_eq!(rig.local_cursor(), (1915, 500));
        rig.cycle(&[]);
        rig.move_by(5, 0);
        assert_eq!(rig.out.pointer.len(), 1, "moved there once");

        // Home is where it left, not where it hid.
        rig.pipe.shut_down(&mut rig.out);
        assert_eq!(rig.local_cursor(), START);
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
}
