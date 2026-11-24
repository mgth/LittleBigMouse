//! The multi-PC master's crossing rules, with no platform in sight (docs/multi-pc.md).
//!
//! A master drives another machine through the input stream while the pointer is
//! there. What it does then — which side a release goes to, when the keyboards are
//! grabbed, when the pointer comes home, the rescue watched for in the grabbed stream —
//! does not depend on how the platform reads its devices. Those rules live here, in
//! [`Crossing`], so that every master (evdev on Linux today, the Windows hook and the
//! portal later) applies the same ones and is tested against the same ones. A master
//! routes each report through the engine, [`locate`]s where it left the pointer, and
//! hands that to [`Crossing::dispatch`]; the rules act only through what the master
//! provides as a [`Platform`].
//!
//! # Crossing
//!
//! Crossing out: `Enter` first, at the landing pixel; then nothing more to this
//! machine's pointer, and the keyboards are grabbed so that typing follows the pointer.
//! Crossing back: `Leave`, the keyboards are let go, and emission resumes here from
//! where the engine landed.
//!
//! ## The local cursor
//!
//! The platform would go on drawing the frozen pointer, so it is hidden while the
//! pointer is away: [`Platform::hide_cursor`] once the `Enter` is queued,
//! [`Platform::show_cursor`] every time the pointer comes home, whichever way. Once
//! what hides it is in place, the frozen pointer is moved onto it
//! ([`Platform::cursor_hiding_spot`]) and stays there; coming home starts from where it
//! left, not from there.
//!
//! ## The lock keys
//!
//! Caps Lock, Num Lock and Scroll Lock are per machine: typing over there with the
//! state of here would type the wrong case. Right after the `Enter`, the batch says
//! which of them are on here ([`Platform::locks`]), and the other machine taps its own
//! into line (`remote::lock_taps`). A lock this machine cannot read is not sent; one
//! the other machine cannot read is left alone there. A tap while the pointer is over
//! there is a key like any other, and toggles the lock over there only.
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
//!   release that comes later is handed to this machine, which has nothing to release.
//! * A keyboard is grabbed only once none of its keys is held
//!   ([`Platform::grab_keyboards`]): a key held while crossing went to this machine,
//!   and its release has to go there too.
//! * Key autorepeat is forwarded as a repeated press, which the remote turns into its
//!   own repeat; button repeats do not exist and are dropped.
//!
//! ## Coming home
//!
//! When a hop of the input stream ends, the pointer is home (docs/multi-pc.md): the
//! pointer goes back to the last position it had here ([`Platform::return_pointer`]),
//! the keyboards are let go and the routing forgets it was elsewhere. That happens when
//! the client goes away or is replaced by another, when a batch cannot be queued (no
//! client, a connection that closed, or a full queue — which also closes the
//! connection, [`crate::ipc::input::Driver::send`]), when a reloaded layout no longer
//! has the machine the pointer is on, when the rescue shortcut fires, and when the
//! master goes away, which is how Stop, pause, suspend, the rescue's unhook and quit
//! all end ([`Crossing::shut_down`]). Without a client, the other machines' zones are
//! not in play and their borders are walls. `Leave` is sent whenever it can be; when it
//! cannot, the connection is closing, and its end means the same thing to the remote.
//!
//! ## The rescue
//!
//! With the keyboards grabbed the desktop's own shortcut never fires. The configured
//! shortcut is therefore watched for in the grabbed stream ([`keys::RescueKeys`]), with
//! the same hold, and fires the same `daemon::rescue_fired` after bringing the pointer
//! home. Its final key is kept back while it is held, so the other machine never sees
//! it; given up early, it is forwarded late, in order.
//!
//! ## The pointer lock
//!
//! A shortcut, off unless the user sets one (`Command::PointerLock`), locks the pointer
//! on the machine it is on, and unlocks it. It is watched for in every key this master
//! reads ([`keys::ToggleKeys`]), grabbed or not, and fires on the press. Over there,
//! its final key is never forwarded; here, the desktop sees it too, as it sees any key
//! of a keyboard that is not grabbed.
//!
//! * Locked over there, the pointer does not come back: a report that would take it
//!   here, or to a third machine, leaves it where it was on the machine it is on, and
//!   the routing starts again from there ([`Platform::return_pointer`]). What the
//!   report pressed still goes over there.
//! * Locked here, the other machines' borders are walls, as without a client.
//! * The lock never keeps the pointer from coming home: the rescue, the end of a hop
//!   of the stream and the master going away bring it home and let the lock go, and so
//!   does the stream's client going away or being replaced while it is locked here.
//! * Each change is said to the agent (`Event::PointerLocked`), at the end of the step
//!   that made it.
//! * The shortcut grammar (`lbm_ipc::shortcut`) has no lock key, so the shortcut never
//!   toggles a lock key the crossing aligns.
//!
//! ## One remote
//!
//! Batches carry no destination: the agent relays them to the one remote it drives. A
//! layout with zones of two machines is routed over both by the engine, but crossing
//! from one to the other is treated as leaving the one being driven, and the pointer
//! comes home.

// Only the evdev backend is a master so far: on Windows nothing calls into this yet,
// and it is compiled there all the same, so that it stays platform-neutral.
#![cfg_attr(not(target_os = "linux"), allow(dead_code))]

use std::sync::atomic::Ordering;
use std::time::Instant;

use lbm_ipc::input::{InputEvent as Wire, Locks};
use lbm_ipc::protocol::Event;

use crate::geometry::Point;
use crate::shared::Shared;
use crate::zones::{HostPoint, ZonesLayout};

use keys::{HeldSet, RescueKeys, Toggle, ToggleKeys, Verdict};

pub(crate) mod keys;
#[cfg(test)]
mod tests;

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

    /// This machine's lock keys, read as the pointer crosses out: the machine it goes
    /// to aligns its own on them. A lock the platform cannot read is unknown, and the
    /// other machine leaves its own alone.
    fn locks(&self) -> Locks {
        Locks::default()
    }

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
    /// routing start afresh from [`Crossing::resume_at`] before it routes again — the
    /// same point, unless the pointer is locked on another machine. Emits nothing.
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
    /// The pointer stays on the machine it is on.
    locked: bool,
    /// Whether the agent was last told it is locked.
    reported: bool,
    /// The pointer lock's shortcut was pressed since the keys were last flushed.
    toggled: bool,
    toggle: ToggleKeys,
    /// The revision of the shared shortcut `toggle` was set from.
    toggle_revision: Option<u32>,
    /// Where the routing last had the pointer on the machine being driven, in the
    /// routing's coordinates: where a lock holds it.
    remote_pos: Point<i32>,
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
            locked: false,
            reported: false,
            toggled: false,
            toggle: ToggleKeys::default(),
            toggle_revision: None,
            remote_pos: home,
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

    /// Where the routing starts afresh after [`Platform::return_pointer`]: home, or,
    /// while the pointer is locked over there, where it was there.
    pub(crate) fn resume_at(&self) -> Point<i32> {
        if self.is_remote() {
            self.remote_pos
        } else {
            self.home
        }
    }

    /// Whether the pointer is locked on the machine it is on.
    #[cfg(test)]
    pub(crate) fn is_locked(&self) -> bool {
        self.locked
    }

    /// Whether the other machines' borders are walls because the pointer is locked
    /// here: the routing is to treat them as it does without a client.
    pub(crate) fn walled_in(&self) -> bool {
        self.locked && !self.is_remote()
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
        if self.locked && client != self.client {
            eprintln!(
                "[LittleBigMouse.Hook] multi-PC: the input stream's client changed, the pointer lock is let go"
            );
            self.locked = false;
        }
        let revision = shared.shortcuts_revision.load(Ordering::SeqCst);
        if self.toggle_revision != Some(revision) {
            // Never waited for: a contended read is tried again next cycle.
            if let (Ok(text), Ok(rescue), Ok(letters)) = (
                shared.pointer_lock_shortcut.try_lock(),
                shared.rescue_shortcut.try_lock(),
                shared.letters.try_lock(),
            ) {
                // At once, even with the pointer over there: a rescue changed by a
                // reload in the middle of a crossing is the one held down now.
                self.rescue.set_on(&rescue, *letters);
                let relaid = self.toggle.relayout(*letters);
                let rescue_changed = self.toggle.avoid(&rescue);
                let changed = self.toggle.set(&text) || rescue_changed || relaid;
                self.toggle_revision = Some(revision);
                if changed {
                    self.shortcut_changed();
                }
            }
        }
        self.check_rescue(shared, Instant::now(), m);
        self.report(shared);
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
        self.report(shared);
    }

    /// Tells the agent the lock changed, if it did.
    fn report(&mut self, shared: &Shared) {
        if self.locked != self.reported {
            self.reported = self.locked;
            shared.broadcast(&Event::PointerLocked {
                locked: self.locked,
            });
        }
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
            (Side::Local, Some(Place::Enter { .. })) if self.locked => {
                // Locked here: the border is a wall.
                self.go_home(m);
                self.local_leftovers(m);
            }
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
            (Side::Remote, Some(Place::Local | Place::Enter { .. } | Place::Away))
                if self.locked =>
            {
                // Locked over there: it stays where it was there.
                m.return_pointer(self.remote_pos);
                self.remote_frame(None, m);
            }
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
                &mut self.toggle,
                &mut self.toggled,
                code,
                press,
                now,
            );
        }
    }

    /// One key of a keyboard that is not grabbed: the compositor has it, and only the
    /// shortcuts are looked for in it — the pointer lock's, and the rescue's, which
    /// the keyboards read here hold without the desktop's global shortcuts
    /// (see [`Self::check_rescue`]).
    pub(crate) fn observed_key(&mut self, code: u16, press: Press, now: Instant) {
        if self.toggle.filter(code, press) == Toggle::Fired {
            self.toggled = true;
        }
        if !self.is_remote() {
            // Nothing to keep back: the compositor has the key already.
            self.rescue.filter(code, press, now);
        }
    }

    /// Sends the keys [`Self::grabbed_key`] collected from one keyboard's drain.
    pub(crate) fn flush_keys(&mut self, shared: &'static Shared, m: &mut impl Platform) {
        if std::mem::take(&mut self.toggled) {
            self.toggle_lock(m);
            self.report(shared);
        }
        if !self.wire.is_empty() && !self.send(m) {
            eprintln!(
                "[LittleBigMouse.Hook] multi-PC: the input stream took no more, the pointer comes home"
            );
            self.go_home(m);
        }
        self.check_rescue(shared, Instant::now(), m);
    }

    /// The pointer lock's shortcut is another, or none: a lock it took can no longer be
    /// let go by it, so it goes now.
    fn shortcut_changed(&mut self) {
        if self.locked {
            eprintln!(
                "[LittleBigMouse.Hook] multi-PC: the pointer lock's shortcut changed, the lock is let go"
            );
            self.locked = false;
        }
    }

    /// The pointer lock's shortcut was pressed: locks the pointer where it is, or lets
    /// it go. Here, only with a machine to lock it away from.
    fn toggle_lock(&mut self, m: &mut impl Platform) {
        if self.locked {
            self.locked = false;
        } else if self.is_remote() {
            self.locked = true;
        } else if let Some(client) = m.remote_client() {
            self.locked = true;
            self.client = Some(client);
        } else {
            return;
        }
        eprintln!(
            "[LittleBigMouse.Hook] multi-PC: the pointer is {}",
            match (self.locked, self.is_remote()) {
                (false, _) => "no longer locked",
                (true, true) => "locked over there",
                (true, false) => "locked here",
            }
        );
    }

    /// Fires the rescue if its shortcut has been held long enough in the keys read
    /// here. In the grabbed stream, home first, so that the `Leave` releases the
    /// modifiers over there. In the keys the compositor has too, only while the
    /// desktop has no binding of its own for it: that one fires it then.
    pub(crate) fn check_rescue(
        &mut self,
        shared: &'static Shared,
        now: Instant,
        m: &mut impl Platform,
    ) {
        if !self.rescue.due(now) {
            return;
        }
        if self.is_remote() {
            self.go_home(m);
        } else if shared.rescue_bound_by_desktop.load(Ordering::SeqCst) {
            return;
        } else {
            eprintln!("[LittleBigMouse.Hook] rescue: held on the keyboards this daemon reads");
        }
        crate::daemon::rescue_fired(shared);
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
            // Not locked: the pointer is only now crossing out of here, where a lock
            // would have walled it in.
            self.toggle.avoid(&text);
        }
        self.rescue.reset();
        self.side = Side::Remote;
        self.host = host;
        self.client = m.remote_client();
        self.sent = pixel;
        self.remote_pos = m.pointer();
        self.wire.clear();
        self.wire.push(Wire::Enter {
            x: pixel.x(),
            y: pixel.y(),
        });
        // Read before the keyboards are grabbed: the compositor still lights them.
        let locks = m.locks();
        if locks != Locks::default() {
            self.wire.push(Wire::Locks(locks));
        }
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
        if pixel.is_some() {
            self.remote_pos = m.pointer();
        }
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
            toggle,
            toggled,
            ..
        } = self;
        let (horizontal, vertical) = m.take_remote_events(|input| match input {
            Input::Button(code, press) => route_button(wire, remote_held, code, press),
            Input::Key(code, press) => {
                route_key(wire, remote_held, rescue, toggle, toggled, code, press, now)
            }
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
        self.locked = false;
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
/// into the batch, kept back as the rescue's final key, or the pointer lock's
/// (`toggled` then says it fired).
#[allow(clippy::too_many_arguments)]
fn route_key(
    wire: &mut Vec<Wire>,
    held: &mut HeldSet,
    rescue: &mut RescueKeys,
    toggle: &mut ToggleKeys,
    toggled: &mut bool,
    code: u16,
    press: Press,
    now: Instant,
) -> bool {
    match toggle.filter(code, press) {
        Toggle::Fired => {
            *toggled = true;
            return true;
        }
        Toggle::Swallow => return true,
        Toggle::Pass => {}
    }
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
