//! The multi-PC master's crossing rules, with no platform in sight (docs/multi-pc.md).
//!
//! A master drives another machine through the input stream while the pointer is
//! there. What it does then — which side a release goes to, when the keyboards are
//! grabbed, when the pointer comes home, the rescue watched for in the grabbed stream —
//! does not depend on how the platform reads its devices. Those rules live here, so
//! that every master (evdev on Linux today, the Windows hook and the portal later)
//! applies the same ones and is tested against the same ones.

// Only the evdev backend is a master so far: on Windows nothing calls into this yet,
// and it is compiled there all the same, so that it stays platform-neutral.
#![cfg_attr(not(target_os = "linux"), allow(dead_code))]

use std::time::Instant;

use lbm_ipc::input::InputEvent as Wire;

use crate::geometry::Point;
use crate::shared::Shared;
use crate::zones::{HostPoint, ZonesLayout};

use keys::{HeldSet, RescueKeys, Verdict};

pub(crate) mod keys;

/// What one event does to a key or a button. Codes are Linux `EV_KEY` codes whatever
/// the platform: they are what the input stream carries (`lbm_ipc::input`), and a
/// master on another platform translates to them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Press {
    Up,
    Down,
    /// Autorepeat of a key held down.
    Repeat,
}

impl Press {
    /// From the value of a Linux `EV_KEY` event: 0 releases, 1 presses, anything
    /// else (2) repeats.
    pub(crate) fn from_ev_key(value: i32) -> Press {
        match value {
            0 => Press::Up,
            1 => Press::Down,
            _ => Press::Repeat,
        }
    }
}

/// Where a routed position is, as far as the next frame is concerned.
#[derive(Debug, PartialEq)]
pub(crate) enum Place {
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

/// Where `pos`, a position the engine routed to, leaves the pointer. `inside_here`:
/// `pos` lies on this machine's own desktop. `driving`: the machine the pointer is
/// on, while it is on another one.
///
/// Only a position outside this desktop is looked up: the other machines' desktops lie
/// outside it, so a position inside is this machine's without asking.
pub(crate) fn locate(
    layout: &ZonesLayout,
    pos: Point<i32>,
    inside_here: bool,
    driving: Option<&str>,
) -> Place {
    if driving.is_none() && inside_here {
        return Place::Local;
    }
    match layout.host_point(pos) {
        Some(HostPoint { host: None, .. }) => Place::Local,
        Some(HostPoint {
            host: Some(there),
            pixel,
        }) => match driving {
            Some(host) if there == host => Place::Remote(pixel),
            Some(_) => Place::Away,
            None => Place::Enter {
                pixel,
                host: there.to_owned(),
            },
        },
        None if driving.is_some() => Place::Away,
        None => Place::Local,
    }
}

/// A button or a key of the frame being dispatched, as the crossing rules see it.
/// Codes are Linux `EV_KEY` codes (see [`Press`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Input {
    /// A mouse button (`BTN_LEFT..=BTN_TASK`).
    Button(u16, Press),
    /// A keyboard key.
    Key(u16, Press),
}

/// What a master's platform does for the crossing rules: the input stream to the
/// machine being driven, this machine's keyboards and cursor, and its pointer with
/// the frame being dispatched. The rules call these; they never touch a device.
pub(crate) trait Platform {
    // --- the input stream ---

    /// The input stream's client to drive another machine through, if it has one. A
    /// client that replaced another has another value.
    fn remote_client(&self) -> Option<u64>;
    /// Whether the input stream has a client to drive another machine through.
    fn remote_connected(&self) -> bool {
        self.remote_client().is_some()
    }
    /// Queues one encoded batch without waiting; `false` when it will never be sent.
    fn send_remote(&mut self, batch: Vec<u8>) -> bool;

    // --- the keyboards ---

    /// Grabs every keyboard that has no key held and is not grabbed yet.
    fn grab_keyboards(&mut self);
    /// Lets every keyboard go.
    fn release_keyboards(&mut self);

    // --- the local cursor ---

    /// The pointer went to another machine from `from`, where it is frozen: hide this
    /// machine's cursor.
    fn hide_cursor(&mut self, from: Point<i32>);
    /// Where the frozen pointer is to go to be under what hides the cursor, once that
    /// is in place; `None` until then, and when nothing hides it.
    fn cursor_hiding_spot(&mut self) -> Option<Point<i32>>;
    /// The pointer is home: show this machine's cursor.
    fn show_cursor(&mut self);

    // --- this machine's pointer, and the frame being dispatched ---

    /// Where the pointer is on this machine, as the platform last committed it.
    fn pointer(&self) -> Point<i32>;
    /// Puts the pointer back at `home`, whatever the routing thought, and has the
    /// routing start afresh from there before it routes again. Emits nothing.
    fn return_pointer(&mut self, home: Point<i32>);
    /// Emits the frame's pointer part — its buttons and wheels, if any — with this
    /// machine's pointer at `at`.
    fn emit_pointer(&mut self, at: Point<i32>);
    /// Whether the frame still has something for this machine's pointer.
    fn pointer_pending(&self) -> bool;
    /// Emits the frame's keyboard part here, if it has one.
    fn emit_keyboard(&mut self);
    /// Hands the frame's buttons and keys to `take`, removes from the frame those it
    /// took (it returns `true`), drops what means nothing over there (wheels, which
    /// are returned instead, and whatever describes this machine's devices), and
    /// returns the frame's wheels in 120ths of a detent, horizontal then vertical.
    fn take_remote_events(&mut self, take: impl FnMut(Input) -> bool) -> (i32, i32);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Side {
    Local,
    Remote,
}

/// The crossing state of a master: where the pointer is, what was sent where, and
/// what is held on the machine being driven.
pub(crate) struct Crossing {
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
    rescue: RescueKeys,
    /// The batch being composed; reused.
    wire: Vec<Wire>,
}

impl Crossing {
    /// The pointer here, at `home`.
    pub(crate) fn new(home: Point<i32>) -> Crossing {
        Crossing {
            side: Side::Local,
            home,
            sent: home,
            parked: None,
            host: String::new(),
            client: None,
            remote_held: HeldSet::default(),
            rescue: RescueKeys::default(),
            wire: Vec::with_capacity(16),
        }
    }

    pub(crate) fn is_remote(&self) -> bool {
        self.side == Side::Remote
    }

    /// The last position the pointer had on this machine.
    pub(crate) fn home(&self) -> Point<i32> {
        self.home
    }

    /// The pointer moved here without being routed — the engine was busy.
    pub(crate) fn set_home(&mut self, home: Point<i32>) {
        self.home = home;
    }

    /// Where this machine's pointer is while the pointer is away: on what hides the
    /// cursor once it is there, where it left until then. A frame emitted here then —
    /// a release of a button pressed here — must not take it off what hides it.
    pub(crate) fn frozen_at(&self) -> Point<i32> {
        self.parked.unwrap_or(self.home)
    }

    /// The machine the pointer is on, while it is on another one: what
    /// [`locate`] is to be told.
    pub(crate) fn driving(&self) -> Option<&str> {
        self.is_remote().then_some(self.host.as_str())
    }

    /// The start of a cycle: catch up with the input stream's client, and come home if
    /// it went or was replaced. Returns whether the stream has a client, as it was
    /// found.
    pub(crate) fn begin_cycle(&mut self, shared: &'static Shared, m: &mut impl Platform) -> bool {
        let client = m.remote_client();
        let connected = client.is_some();
        if self.is_remote() && !connected {
            eprintln!(
                "[LittleBigMouse.Hook] multi-PC: the input stream has no client, the pointer comes home"
            );
            self.go_home(m);
        } else if self.is_remote() && client != self.client {
            // The agent closed the stream and opened another between two cycles — it
            // does after the other machine dropped input. The new client never read
            // the `Enter`: what it is sent now would be ignored, with the keyboards
            // still grabbed here. Its `Leave` is one it has nothing to do with.
            eprintln!(
                "[LittleBigMouse.Hook] multi-PC: the input stream has another client, the pointer comes home"
            );
            self.go_home(m);
        }
        self.check_rescue(shared, Instant::now(), m);
        connected
    }

    /// The layout the engine routes over, read at the start of a cycle with `pos`
    /// where the platform's pointer is: when the pointer is away and the layout no
    /// longer has the machine it is on there, it comes home.
    pub(crate) fn check_layout(
        &mut self,
        layout: &ZonesLayout,
        pos: Point<i32>,
        m: &mut impl Platform,
    ) {
        if !self.is_remote() {
            return;
        }
        let here = layout
            .host_point(pos)
            .is_some_and(|p| p.host == Some(self.host.as_str()));
        if !here {
            eprintln!(
                "[LittleBigMouse.Hook] multi-PC: the layout no longer has the machine the pointer is on, the pointer comes home"
            );
            self.go_home(m);
        }
    }

    /// The end of a cycle: a keyboard that had a key held when the pointer left can be
    /// grabbed once it has none; the frozen pointer goes onto what hides it once that
    /// is in place; and a rescue held long enough fires.
    pub(crate) fn end_cycle(&mut self, shared: &'static Shared, m: &mut impl Platform) {
        if self.is_remote() {
            m.grab_keyboards();
            if self.parked.is_none() {
                if let Some(spot) = m.cursor_hiding_spot() {
                    eprintln!(
                        "[LittleBigMouse.Hook] multi-PC: the frozen pointer moves to ({},{}), onto what hides it",
                        spot.x(),
                        spot.y()
                    );
                    self.parked = Some(spot);
                    m.emit_pointer(spot);
                }
            }
        }
        self.check_rescue(shared, Instant::now(), m);
    }

    /// The master is going away — Stop, pause, suspend, the rescue's unhook, quit: the
    /// pointer comes home while the devices are still there to say so.
    pub(crate) fn shut_down(&mut self, m: &mut impl Platform) {
        if self.is_remote() {
            self.go_home(m);
        }
    }

    /// Sends the frame where `place` says the pointer now is, crossing to or from
    /// another machine first when it has just done so. `None`: it did not move.
    pub(crate) fn dispatch(
        &mut self,
        shared: &Shared,
        place: Option<Place>,
        m: &mut impl Platform,
    ) {
        match (self.side, place) {
            (Side::Local, Some(Place::Enter { pixel, host })) => {
                if !(m.remote_connected() && self.enter(shared, pixel, host, m)) {
                    // Nobody to take it — the client left between the cycle's start
                    // and this report: the border is a wall.
                    self.go_home(m);
                    self.local_leftovers(m);
                }
            }
            (Side::Local, _) => self.local_frame(m),
            (Side::Remote, None) => self.remote_frame(None, m),
            (Side::Remote, Some(Place::Remote(pixel))) => self.remote_frame(Some(pixel), m),
            (Side::Remote, Some(Place::Local)) => {
                // The engine brought it back across a border: it landed where it is.
                self.wire.clear();
                self.wire.push(Wire::Leave);
                self.send(m);
                self.leave(m);
                self.local_frame(m);
            }
            (Side::Remote, Some(Place::Enter { .. } | Place::Away)) => {
                self.go_home(m);
                self.local_leftovers(m);
            }
        }
    }

    /// One key of a grabbed keyboard: the compositor does not see it, so it is this
    /// process's to forward. Collected until [`Self::flush_keys`].
    pub(crate) fn grabbed_key(&mut self, code: u16, press: Press, now: Instant) {
        if self.is_remote() {
            // Not taken: the release of a key this stream never pressed, which has
            // nowhere to go — the compositor cannot receive from a grabbed keyboard.
            route_key(
                &mut self.wire,
                &mut self.remote_held,
                &mut self.rescue,
                code,
                press,
                now,
            );
        }
    }

    /// Sends the keys [`Self::grabbed_key`] collected from one keyboard's drain.
    pub(crate) fn flush_keys(&mut self, shared: &'static Shared, m: &mut impl Platform) {
        if self.wire.is_empty() {
            return;
        }
        if !self.send(m) {
            eprintln!(
                "[LittleBigMouse.Hook] multi-PC: the input stream took no more, the pointer comes home"
            );
            self.go_home(m);
        }
        self.check_rescue(shared, Instant::now(), m);
    }

    /// Fires the rescue if its shortcut has been held long enough in the grabbed
    /// stream: home first, so that the `Leave` releases the modifiers over there.
    pub(crate) fn check_rescue(
        &mut self,
        shared: &'static Shared,
        now: Instant,
        m: &mut impl Platform,
    ) {
        if self.is_remote() && self.rescue.due(now) {
            self.go_home(m);
            crate::daemon::rescue_fired(shared);
        }
    }

    /// Brings the pointer back to the last position it had here, whatever the routing
    /// thought, and says so over there if it was there.
    pub(crate) fn go_home(&mut self, m: &mut impl Platform) {
        if self.is_remote() {
            self.wire.clear();
            self.wire.push(Wire::Leave);
            // When this fails the connection is closing, and its end is a Leave too.
            self.send(m);
            self.leave(m);
        }
        m.return_pointer(self.home);
        m.emit_pointer(self.home);
    }

    /// The ordinary frame: everything pending, to this machine's devices.
    fn local_frame(&mut self, m: &mut impl Platform) {
        self.home = m.pointer();
        m.emit_pointer(self.home);
        m.emit_keyboard();
    }

    /// Crossing out. `false` when the `Enter` could not be queued: the pointer stays.
    fn enter(
        &mut self,
        shared: &Shared,
        pixel: Point<i32>,
        host: String,
        m: &mut impl Platform,
    ) -> bool {
        // Read when it may be needed, and never waited for: a Load holding the lock
        // leaves the shortcut that was last read.
        if let Ok(text) = shared.rescue_shortcut.try_lock() {
            self.rescue.set(&text);
        }
        self.rescue.reset();
        self.side = Side::Remote;
        self.host = host;
        self.client = m.remote_client();
        self.sent = pixel;
        self.wire.clear();
        self.wire.push(Wire::Enter {
            x: pixel.x(),
            y: pixel.y(),
        });
        self.take_remote_events(Instant::now(), m);
        if !self.send(m) {
            self.side = Side::Local;
            self.remote_held.clear();
            return false;
        }
        eprintln!(
            "[LittleBigMouse.Hook] multi-PC: the pointer is on {}",
            self.host
        );
        self.parked = None;
        m.hide_cursor(self.home);
        m.grab_keyboards();
        self.local_leftovers(m);
        true
    }

    /// A frame while the pointer is away: its motion, and whatever it pressed or
    /// released there, to the other machine; what it released of this one, here.
    fn remote_frame(&mut self, pixel: Option<Point<i32>>, m: &mut impl Platform) {
        self.wire.clear();
        if let Some(pixel) = pixel.filter(|&p| p != self.sent) {
            self.sent = pixel;
            self.wire.push(Wire::Motion {
                x: pixel.x(),
                y: pixel.y(),
            });
        }
        self.take_remote_events(Instant::now(), m);
        if !self.send(m) {
            eprintln!(
                "[LittleBigMouse.Hook] multi-PC: the input stream took no more, the pointer comes home"
            );
            self.go_home(m);
        }
        self.local_leftovers(m);
    }

    /// Emits what a frame left for this machine while the pointer is away — releases
    /// of what was pressed here — if anything.
    fn local_leftovers(&mut self, m: &mut impl Platform) {
        if m.pointer_pending() {
            let at = match self.side {
                Side::Remote => self.frozen_at(),
                Side::Local => m.pointer(),
            };
            m.emit_pointer(at);
        }
        m.emit_keyboard();
    }

    /// Moves this frame's buttons, wheels and keys that belong to the other machine
    /// into the batch, leaving in the frame those that belong here.
    fn take_remote_events(&mut self, now: Instant, m: &mut impl Platform) {
        let Crossing {
            wire,
            remote_held,
            rescue,
            ..
        } = self;
        let (horizontal, vertical) = m.take_remote_events(|input| match input {
            Input::Button(code, press) => route_button(wire, remote_held, code, press),
            Input::Key(code, press) => route_key(wire, remote_held, rescue, code, press, now),
        });
        if (horizontal, vertical) != (0, 0) {
            wire.push(Wire::Wheel {
                horizontal,
                vertical,
            });
        }
    }

    /// Encodes and queues the batch being composed. An empty one is not sent.
    fn send(&mut self, m: &mut impl Platform) -> bool {
        if self.wire.is_empty() {
            return true;
        }
        let mut batch = Vec::with_capacity(self.wire.len() * 9);
        for event in &self.wire {
            event.encode(&mut batch);
        }
        self.wire.clear();
        m.send_remote(batch)
    }

    /// The other machine is no longer driven: what it holds pressed, its `Leave`
    /// released.
    fn leave(&mut self, m: &mut impl Platform) {
        self.side = Side::Local;
        self.remote_held.clear();
        self.rescue.reset();
        self.parked = None;
        m.show_cursor();
        m.release_keyboards();
        eprintln!("[LittleBigMouse.Hook] multi-PC: the pointer is back");
    }
}

/// A mouse button while the pointer is away: `true` when it went into the batch.
fn route_button(wire: &mut Vec<Wire>, held: &mut HeldSet, code: u16, press: Press) -> bool {
    match press {
        Press::Down => {
            held.insert(code);
            wire.push(Wire::Button {
                code,
                pressed: true,
            });
            true
        }
        // Released where it was pressed.
        Press::Up if held.remove(code) => {
            wire.push(Wire::Button {
                code,
                pressed: false,
            });
            true
        }
        Press::Up => false,
        // A repeat goes where its press went; a button's means nothing anywhere.
        Press::Repeat => held.contains(code),
    }
}

/// A key of the grabbed stream while the pointer is away: `true` when it was taken —
/// into the batch, or kept back as the rescue's final key.
fn route_key(
    wire: &mut Vec<Wire>,
    held: &mut HeldSet,
    rescue: &mut RescueKeys,
    code: u16,
    press: Press,
    now: Instant,
) -> bool {
    match rescue.filter(code, press, now) {
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
    match press {
        Press::Down => {
            held.insert(code);
            wire.push(Wire::Key {
                code,
                pressed: true,
            });
            true
        }
        Press::Up if held.remove(code) => {
            wire.push(Wire::Key {
                code,
                pressed: false,
            });
            true
        }
        Press::Up => false,
        // Autorepeat: the remote repeats an injected key only when told to.
        Press::Repeat if held.contains(code) => {
            wire.push(Wire::Key {
                code,
                pressed: true,
            });
            true
        }
        Press::Repeat => false,
    }
}
