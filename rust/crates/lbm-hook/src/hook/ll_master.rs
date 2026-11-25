//! A multi-PC master fed by low-level hooks: one event at a time, each passed on to the
//! system or swallowed (docs/multi-pc.md). The Windows master is this, behind
//! `WH_MOUSE_LL` and `WH_KEYBOARD_LL` (`hook::windows::master`); kept free of any
//! system call, behind [`Desk`], so that it is tested on every system against the real
//! engine.
//!
//! What follows from where the pointer is — `Enter`, `Leave`, where a release goes, the
//! rescue, the pointer lock, coming home — is [`Crossing`]'s, as for the evdev master.
//! What this module adds is what a low-level hook does not give:
//!
//! # Motion while the pointer is away
//!
//! A low-level mouse hook reports where the system is about to put the cursor: the
//! cursor plus the displacement, accelerated. Let through, the system then puts the
//! cursor there, clamped to the desktop, and the displacement past the edge is lost. So
//! while the pointer is on another machine every move is swallowed, which leaves the
//! cursor where it is, and the report is read as a displacement from there:
//! `report - cursor` (tried on the vm124: a swallowed (7, 3) reported the cursor plus
//! (9..11, 4..5), the cursor staying put). The cursor is first frozen where it left,
//! then moved under what hides it, in the middle of a monitor (`remote::parking`), so
//! that it is never at an edge that would bound the next report. That is how the
//! Windows tools of the kind do it (Synergy's server warps to the centre of its
//! screen), and it keeps the system's pointer acceleration: the displacement is the one
//! the system was about to apply. Raw Input, the other way, gives the mouse's counts
//! before acceleration, which would have to be redone to feel the same, and arrives as
//! window messages, apart from the hook's buttons and keys.
//!
//! A pointer that reports absolute positions — a tablet, a remote desktop session, a
//! virtual machine's tablet — has no displacement to give: its report is where the pen
//! is, and the pointer over there jumps with it. Not handled: such a master drives the
//! other machine badly, and is what Synergy does not support either.
//!
//! # Crossing out at the desktop's edge
//!
//! The engine crosses a border once a reported position is past it, and the other
//! machines' zones lie past the edge of the desktop. The low-level hook reports past
//! it: tried on the vm124 (Windows 11, injected relative moves), a cursor at x = 0
//! pushed left reports x = -5, at the right edge 5759 pushed right 5763, at the top
//! y = -4 — the cursor plus the displacement, the cursor itself staying on the edge.
//! So nothing is added for it here.
//!
//! # A frame is one event
//!
//! A button or a wheel turn while the pointer is away goes to the rules alone, as the
//! frame of [`Platform`]: taken, it is swallowed; left for this machine — the release of
//! a button pressed here — it is passed on. Motion is never passed on while away.
//!
//! # The keyboard
//!
//! There is no per-device grab: "grabbed" means that the keyboard hook swallows every
//! key and hands it to [`Crossing::grabbed_key`]. As on Linux, the keyboard is grabbed
//! only once no key pressed here is held ([`Platform::grab_keyboards`]): a modifier held
//! while crossing went to this machine, its release has to go there too, and until it
//! comes the keys keep typing here. What is held here is what the hook passed on as a
//! press and has not seen released, less what the system no longer has down (a release
//! the hook never saw — the secure desktop took it).
//!
//! A release whose press this machine never had — pressed over there, released after
//! coming home — is swallowed: the `Leave` released it over there, and a lone release
//! here can still do something (the Windows key's opens the Start menu). One the system
//! has down all the same, pressed before the hook was installed, is passed on.

// The Windows master is its only user; compiled everywhere so that it is tested everywhere.
#![cfg_attr(not(windows), allow(dead_code))]

use std::sync::TryLockError;
use std::time::Instant;

use lbm_ipc::input::Locks;

use crate::engine::cursor::CursorEnv;
use crate::engine::event::MouseEventArg;
use crate::engine::MouseEngine;
use crate::geometry::{Point, Rect};
use crate::hook::crossing::keys::HeldSet;
use crate::hook::crossing::{self, Crossing, Input, Place, Platform, Press};
use crate::hook::extent::extents;
use crate::ipc::input::Driver;
use crate::shared::Shared;

/// What a low-level hook does with the event it was handed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Verdict {
    /// On to the system, as if there were no hook.
    Pass,
    /// Dropped: the system never sees it.
    Swallow,
}

/// The system a low-level master runs on: its cursor and clip, its keyboard state, and
/// what hides the cursor. Every call may be a system call; none waits.
pub(crate) trait Desk {
    /// Where the cursor is now. In a low-level hook, before the reported move.
    fn cursor(&self) -> Point<i32>;
    fn set_cursor(&mut self, at: Point<i32>);
    /// The confinement in force, the desktop when there is none.
    fn clip(&self) -> Rect<i32>;
    /// Confines the cursor to `clip`, or lets it go.
    fn set_clip(&mut self, clip: Option<Rect<i32>>);
    fn clip_is_subrect_of_virtual_screen(&self) -> bool;
    fn cursor_hidden(&self) -> bool;
    fn ctrl_down(&self) -> bool;
    fn buttons_down(&self) -> bool;
    fn tick_count(&self) -> u64;
    /// The whole desktop, every monitor.
    fn desktop(&self) -> Rect<i32>;
    /// The lock keys, as the pointer crosses out.
    fn locks(&self) -> Locks;
    /// Starts hiding the cursor: what hides it is in place soon, under
    /// [`Desk::hiding_spot`].
    fn hide_cursor(&mut self);
    /// Where the cursor is hidden, once it is.
    fn hiding_spot(&self) -> Option<Point<i32>>;
    fn show_cursor(&mut self);
    /// Whether the system still has the key of virtual-key code `vk` down.
    fn key_down(&self, vk: u16) -> bool;
    /// Whether this machine can drive another one now: the keys can be taken there
    /// along with the pointer. Not, and the other machines' borders are walls: a
    /// person who clicks in a field over there and types would type here.
    fn can_drive(&mut self) -> bool {
        true
    }
}

/// The input stream to the agent: [`Driver`] in the hook.
pub(crate) trait Link {
    /// Catches up with the endpoint's client. One atomic load when nothing changed.
    fn refresh(&mut self, shared: &Shared);
    fn client(&self) -> Option<u64>;
    fn send(&mut self, batch: Vec<u8>) -> bool;
}

impl Link for Driver {
    fn refresh(&mut self, shared: &Shared) {
        Driver::refresh(self, &shared.outlet);
    }

    fn client(&self) -> Option<u64> {
        Driver::client(self)
    }

    fn send(&mut self, batch: Vec<u8>) -> bool {
        Driver::send(self, batch)
    }
}

/// A key held on this machine: its Linux code, and the virtual key the system knows it
/// by.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct HeldHere {
    code: u16,
    vk: u16,
}

/// The master's state between events.
pub(crate) struct LlMaster<D, L> {
    pub(crate) desk: D,
    link: L,
    crossing: Crossing,
    /// Where the routing has the pointer: the cursor while it is here, the position on
    /// the other machine's zones while it is away.
    pos: Point<i32>,
    /// The engine's confinement, while it is not the system's: always while the pointer
    /// is away, and here for a rectangle the desktop does not contain (a path into
    /// another machine's zones), which the system would clamp to its edge.
    clip: Option<Rect<i32>>,
    /// Every zone in play: where the pointer may go while away and unconfined.
    bounds: Rect<i32>,
    /// The engine still tracks the pointer where it was before it came home.
    resync: bool,
    /// What the engine was last told of the other machines' zones.
    enterable: bool,
    /// The frame: the one button or wheel turn being dispatched, and whether it is to
    /// be passed on.
    pending: Option<Input>,
    wheel: (i32, i32),
    pass: bool,
    /// The keyboard hook swallows keys for the other machine.
    grabbed: bool,
    /// Keys passed on as pressed and not seen released.
    here: Vec<HeldHere>,
    /// Every key down, here or there: a second press is an autorepeat.
    down: HeldSet,
}

/// What the rules are lent for one call.
struct Machine<'a, D, L> {
    desk: &'a mut D,
    link: &'a mut L,
    pos: &'a mut Point<i32>,
    clip: &'a mut Option<Rect<i32>>,
    resync: &'a mut bool,
    pending: &'a mut Option<Input>,
    wheel: &'a mut (i32, i32),
    pass: &'a mut bool,
    grabbed: &'a mut bool,
    here: &'a mut Vec<HeldHere>,
}

impl<D: Desk, L: Link> Platform for Machine<'_, D, L> {
    fn remote_client(&self) -> Option<u64> {
        self.link.client()
    }

    fn send_remote(&mut self, batch: Vec<u8>) -> bool {
        self.link.send(batch)
    }

    fn locks(&self) -> Locks {
        self.desk.locks()
    }

    fn grab_keyboards(&mut self) {
        if *self.grabbed {
            return;
        }
        let desk = &*self.desk;
        self.here.retain(|key| desk.key_down(key.vk));
        if self.here.is_empty() {
            *self.grabbed = true;
        }
    }

    fn release_keyboards(&mut self) {
        *self.grabbed = false;
    }

    fn hide_cursor(&mut self, _from: Point<i32>) {
        // A confinement of the system's would keep what hides the cursor from moving it.
        *self.clip = None;
        self.desk.set_clip(None);
        self.desk.hide_cursor();
    }

    fn cursor_hiding_spot(&mut self) -> Option<Point<i32>> {
        self.desk.hiding_spot()
    }

    fn show_cursor(&mut self) {
        self.desk.show_cursor();
    }

    fn pointer(&self) -> Point<i32> {
        *self.pos
    }

    fn return_pointer(&mut self, home: Point<i32>) {
        *self.pos = home;
        *self.clip = None;
        *self.resync = true;
    }

    fn emit_pointer(&mut self, at: Point<i32>) {
        if self.pointer_pending() {
            *self.pending = None;
            *self.wheel = (0, 0);
            *self.pass = true;
        }
        if self.desk.cursor() != at {
            self.desk.set_cursor(at);
        }
    }

    fn pointer_pending(&self) -> bool {
        matches!(self.pending, Some(Input::Button(..))) || *self.wheel != (0, 0)
    }

    fn emit_keyboard(&mut self) {
        // Keys never travel in a pointer frame here: the keyboard hook has its own.
    }

    fn take_remote_events(&mut self, mut take: impl FnMut(Input) -> bool) -> (i32, i32) {
        if let Some(input) = *self.pending {
            if take(input) {
                *self.pending = None;
            }
        }
        std::mem::take(self.wheel)
    }
}

/// The engine's view of the cursor while it routes one report.
struct Env<'a, D> {
    desk: &'a mut D,
    pos: &'a mut Point<i32>,
    clip: &'a mut Option<Rect<i32>>,
    bounds: Rect<i32>,
    desktop: Rect<i32>,
    away: bool,
}

fn contains_rect(outer: Rect<i32>, inner: Rect<i32>) -> bool {
    inner.left() >= outer.left()
        && inner.top() >= outer.top()
        && inner.right() <= outer.right()
        && inner.bottom() <= outer.bottom()
}

fn clamp(p: Point<i32>, r: Rect<i32>) -> Point<i32> {
    Point::new(
        p.x().clamp(r.left(), r.right() - 1),
        p.y().clamp(r.top(), r.bottom() - 1),
    )
}

impl<D: Desk> Env<'_, D> {
    fn limit(&self) -> Rect<i32> {
        self.clip.unwrap_or(self.bounds)
    }
}

impl<D: Desk> CursorEnv for Env<'_, D> {
    fn get_mouse_location(&self) -> Point<i32> {
        if self.away {
            clamp(*self.pos, self.limit())
        } else {
            self.desk.cursor()
        }
    }

    fn set_mouse_location(&mut self, location: Point<i32>) {
        *self.pos = location;
        // Away, the cursor is frozen and the rules move it; here, a position on another
        // machine is a crossing, which the cursor does not follow.
        if !self.away && self.desktop.contains(location) {
            self.desk.set_cursor(location);
        }
    }

    fn get_clip(&self) -> Rect<i32> {
        match *self.clip {
            Some(clip) => clip,
            None if self.away => self.bounds,
            None => self.desk.clip(),
        }
    }

    fn set_clip(&mut self, r: Rect<i32>) {
        if self.away || !contains_rect(self.desktop, r) {
            *self.clip = Some(r);
            if !self.away {
                self.desk.set_clip(None);
            }
        } else {
            *self.clip = None;
            self.desk.set_clip(Some(r));
        }
    }

    fn ctrl_down(&self) -> bool {
        self.desk.ctrl_down()
    }

    fn buttons_down(&self) -> bool {
        self.desk.buttons_down()
    }

    fn cursor_hidden(&self) -> bool {
        // Away, the cursor is hidden by the master itself, which is no game.
        !self.away && self.desk.cursor_hidden()
    }

    fn clip_is_subrect_of_virtual_screen(&self) -> bool {
        // An emulated clip is the engine's own, never a game's.
        !self.away && self.clip.is_none() && self.desk.clip_is_subrect_of_virtual_screen()
    }

    fn tick_count(&self) -> u64 {
        self.desk.tick_count()
    }
}

impl<D: Desk, L: Link> LlMaster<D, L> {
    pub(crate) fn new(desk: D, link: L) -> Self {
        let home = desk.cursor();
        LlMaster {
            bounds: desk.desktop(),
            desk,
            link,
            crossing: Crossing::new(home),
            pos: home,
            clip: None,
            resync: false,
            enterable: false,
            pending: None,
            wheel: (0, 0),
            pass: false,
            grabbed: false,
            here: Vec::new(),
            down: HeldSet::default(),
        }
    }

    /// The rules, and what they are lent.
    fn split(&mut self) -> (&mut Crossing, Machine<'_, D, L>) {
        let LlMaster {
            desk,
            link,
            crossing,
            pos,
            clip,
            resync,
            pending,
            wheel,
            pass,
            grabbed,
            here,
            ..
        } = self;
        (
            crossing,
            Machine {
                desk,
                link,
                pos,
                clip,
                resync,
                pending,
                wheel,
                pass,
                grabbed,
                here,
            },
        )
    }

    pub(crate) fn is_remote(&self) -> bool {
        self.crossing.is_remote()
    }

    /// Whether the events are this master's to see: while there is a client to drive
    /// another machine through, while the pointer is away, and once more after the
    /// client went, to take the other machines out of play. Otherwise the hook routes as
    /// it always did. One atomic load when nothing changed.
    pub(crate) fn active(&mut self, shared: &Shared) -> bool {
        self.link.refresh(shared);
        self.crossing.is_remote() || self.enterable || self.link.client().is_some()
    }

    /// Whether a keyboard hook is needed: there is a client, or the pointer is away.
    pub(crate) fn wants_keyboard(&mut self, shared: &Shared) -> bool {
        self.link.refresh(shared);
        self.crossing.is_remote() || self.link.client().is_some()
    }

    /// One reported move to `report`. `None` when the master is not [`Self::active`]:
    /// the hook routes it as it always did.
    pub(crate) fn on_move(
        &mut self,
        shared: &'static Shared,
        report: Point<i32>,
    ) -> Option<Verdict> {
        if !self.active(shared) {
            return None;
        }
        let connected = {
            let (crossing, mut m) = self.split();
            crossing.begin_cycle(shared, &mut m)
        };
        let verdict = self.route(shared, connected, report);
        let (crossing, mut m) = self.split();
        crossing.end_cycle(shared, &mut m);
        Some(verdict)
    }

    fn route(&mut self, shared: &'static Shared, connected: bool, report: Point<i32>) -> Verdict {
        // Never wait for a Load: here the report passes as with no hook; away, the
        // pointer holds still for it.
        let mut engine = match shared.engine.try_lock() {
            Ok(guard) => guard,
            Err(TryLockError::Poisoned(p)) => p.into_inner(),
            Err(TryLockError::WouldBlock) => {
                return if self.crossing.is_remote() {
                    Verdict::Swallow
                } else {
                    self.crossing.set_home(report);
                    Verdict::Pass
                };
            }
        };
        // Only once the cycle found a client: asked of the system only then.
        let enterable = connected && !self.crossing.walled_in() && self.desk.can_drive();
        // Only the machines the agent drives, once it named them.
        self.crossing.put_in_play(&mut engine.layout, enterable);
        self.enterable = enterable;
        {
            let pos = self.pos;
            let (crossing, mut m) = self.split();
            crossing.check_layout(&engine.layout, pos, &mut m);
        }
        if self.resync {
            self.resync_with(&mut engine);
        }
        self.bounds = extents(&engine.layout).0;
        let desktop = self.desk.desktop();
        let away = self.crossing.is_remote();
        if !away {
            // A confinement the engine set while the pointer was away, landing here —
            // its zone of arrival — is the system's to apply now that it is here.
            if let Some(clip) = self.clip.filter(|&c| contains_rect(desktop, c)) {
                self.clip = None;
                self.desk.set_clip(Some(clip));
            }
        }

        let candidate = if away {
            let cursor = self.desk.cursor();
            Point::new(
                self.pos.x().saturating_add(report.x() - cursor.x()),
                self.pos.y().saturating_add(report.y() - cursor.y()),
            )
        } else {
            report
        };
        let mut env = Env {
            desk: &mut self.desk,
            pos: &mut self.pos,
            clip: &mut self.clip,
            bounds: self.bounds,
            desktop,
            away,
        };
        let mut e = MouseEventArg::new(candidate);
        engine.on_mouse_move(&mut env, &mut e);
        let pos = if e.handled {
            crate::hook::CROSSINGS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            *env.pos
        } else if away {
            clamp(candidate, env.limit())
        } else {
            report
        };
        let place = crossing::locate(
            &engine.layout,
            pos,
            desktop.contains(pos),
            self.crossing.driving(),
        );
        drop(engine);

        if !away {
            if let Place::Enter { .. } = place {
                self.pending = None;
                let (crossing, mut m) = self.split();
                crossing.dispatch(shared, Some(place), &mut m);
                // Crossed or walled in, the cursor stays where the rules left it.
                return Verdict::Swallow;
            }
            self.crossing.set_home(pos);
            return if e.handled {
                Verdict::Swallow
            } else {
                Verdict::Pass
            };
        }
        self.pos = pos;
        self.pending = None;
        let (crossing, mut m) = self.split();
        crossing.dispatch(shared, Some(place), &mut m);
        Verdict::Swallow
    }

    fn resync_with(&mut self, engine: &mut MouseEngine) {
        let desktop = self.desk.desktop();
        let mut env = Env {
            desk: &mut self.desk,
            pos: &mut self.pos,
            clip: &mut self.clip,
            bounds: self.bounds,
            desktop,
            away: self.crossing.is_remote(),
        };
        engine.begin_touch(&mut env);
        self.pos = self.crossing.resume_at();
        self.resync = false;
    }

    /// A mouse button. `None` while the pointer is here: the system has it.
    pub(crate) fn on_button(
        &mut self,
        shared: &'static Shared,
        code: u16,
        press: Press,
    ) -> Option<Verdict> {
        self.pointer_frame(shared, Some(Input::Button(code, press)), (0, 0))
    }

    /// A wheel turn, in 120ths of a detent. `None` while the pointer is here.
    pub(crate) fn on_wheel(
        &mut self,
        shared: &'static Shared,
        horizontal: i32,
        vertical: i32,
    ) -> Option<Verdict> {
        self.pointer_frame(shared, None, (horizontal, vertical))
    }

    fn pointer_frame(
        &mut self,
        shared: &'static Shared,
        input: Option<Input>,
        wheel: (i32, i32),
    ) -> Option<Verdict> {
        self.link.refresh(shared);
        if !self.crossing.is_remote() {
            return None;
        }
        let (crossing, mut m) = self.split();
        crossing.begin_cycle(shared, &mut m);
        if !crossing.is_remote() {
            crossing.end_cycle(shared, &mut m);
            return Some(Verdict::Pass);
        }
        *m.pending = input;
        *m.wheel = wheel;
        *m.pass = false;
        crossing.dispatch(shared, None, &mut m);
        let verdict = if std::mem::take(m.pass) {
            Verdict::Pass
        } else {
            Verdict::Swallow
        };
        *m.pending = None;
        *m.wheel = (0, 0);
        crossing.end_cycle(shared, &mut m);
        Some(verdict)
    }

    /// A key, `code` its Linux code (`None`: no key the stream can carry), `vk` its
    /// virtual key, `down` whether it is a press.
    pub(crate) fn on_key(
        &mut self,
        shared: &'static Shared,
        code: Option<u16>,
        vk: u16,
        down: bool,
        now: Instant,
    ) -> Verdict {
        let Some(code) = code else {
            return Verdict::Pass;
        };
        let press = match (down, self.down.contains(code)) {
            (true, true) => Press::Repeat,
            (true, false) => Press::Down,
            (false, _) => Press::Up,
        };
        self.down.track(code, press);
        self.link.refresh(shared);

        let (crossing, mut m) = self.split();
        crossing.begin_cycle(shared, &mut m);
        let verdict = if crossing.is_remote() && *m.grabbed {
            crossing.grabbed_key(code, press, now);
            Verdict::Swallow
        } else {
            crossing.observed_key(code, press, now);
            let held = m.here.iter().position(|key| key.code == code);
            match (press, held) {
                (Press::Down, None) => {
                    m.here.push(HeldHere { code, vk });
                    Verdict::Pass
                }
                (Press::Up, Some(i)) => {
                    m.here.swap_remove(i);
                    Verdict::Pass
                }
                // Pressed over there, released here: nothing to release here, unless
                // the system has it down all the same.
                (Press::Up, None) if !m.desk.key_down(vk) => Verdict::Swallow,
                _ => Verdict::Pass,
            }
        };
        crossing.flush_keys(shared, &mut m);
        crossing.end_cycle(shared, &mut m);
        verdict
    }

    /// Time passed with no event while the pointer is away: a rescue held long enough
    /// fires, a client that went is noticed.
    pub(crate) fn on_tick(&mut self, shared: &'static Shared) {
        self.link.refresh(shared);
        let (crossing, mut m) = self.split();
        crossing.begin_cycle(shared, &mut m);
        crossing.end_cycle(shared, &mut m);
    }

    /// The master is going away, or this desktop no longer gets the input (the secure
    /// desktop took it, with the releases of what was pressed over there): the pointer
    /// comes home.
    pub(crate) fn shut_down(&mut self) {
        let (crossing, mut m) = self.split();
        crossing.shut_down(&mut m);
    }
}

#[cfg(test)]
mod tests {
    //! The master driven the way the hooks drive it, with the real engine over two
    //! machines' zones, a recorded desk and a recorded link.

    use super::*;

    use std::sync::atomic::Ordering;

    use lbm_ipc::input::{by_host, decode, InputEvent as Wire};

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

    const BTN_LEFT: u16 = 0x110;
    const KEY_A: u16 = 30;
    const KEY_LEFTCTRL: u16 = 29;
    const VK_A: u16 = 0x41;
    const VK_CONTROL: u16 = 0xA2;
    /// Where the desk hides the cursor: the middle of the monitor.
    const SPOT: (i32, i32) = (960, 540);

    #[derive(Default)]
    struct FakeDesk {
        cursor: (i32, i32),
        clip: Option<Rect<i32>>,
        hiding: bool,
        /// What hides the cursor is in place.
        placed: bool,
        hides: usize,
        shows: usize,
        keys_down: Vec<u16>,
        locks: Locks,
        /// The keys cannot be taken over there.
        no_keyboard: bool,
    }

    impl Desk for FakeDesk {
        fn cursor(&self) -> Point<i32> {
            Point::new(self.cursor.0, self.cursor.1)
        }
        fn set_cursor(&mut self, at: Point<i32>) {
            // The system clamps to the desktop.
            let at = clamp(at, self.desktop());
            self.cursor = (at.x(), at.y());
        }
        fn clip(&self) -> Rect<i32> {
            self.clip.unwrap_or(self.desktop())
        }
        fn set_clip(&mut self, clip: Option<Rect<i32>>) {
            self.clip = clip;
        }
        fn clip_is_subrect_of_virtual_screen(&self) -> bool {
            false
        }
        fn cursor_hidden(&self) -> bool {
            false
        }
        fn ctrl_down(&self) -> bool {
            false
        }
        fn buttons_down(&self) -> bool {
            false
        }
        fn tick_count(&self) -> u64 {
            0
        }
        fn desktop(&self) -> Rect<i32> {
            Rect::new(0, 0, 1920, 1080)
        }
        fn locks(&self) -> Locks {
            self.locks
        }
        fn hide_cursor(&mut self) {
            self.hiding = true;
            self.hides += 1;
        }
        fn hiding_spot(&self) -> Option<Point<i32>> {
            (self.hiding && self.placed).then(|| Point::new(SPOT.0, SPOT.1))
        }
        fn show_cursor(&mut self) {
            self.hiding = false;
            self.shows += 1;
        }
        fn key_down(&self, vk: u16) -> bool {
            self.keys_down.contains(&vk)
        }
        fn can_drive(&mut self) -> bool {
            !self.no_keyboard
        }
    }

    #[derive(Default)]
    struct FakeLink {
        connected: bool,
        client: u64,
        sent: Vec<Wire>,
        /// Every event sent, with the machine the last `Host` marker named.
        routed: Vec<(Option<String>, Wire)>,
        marked: Option<String>,
    }

    impl Link for FakeLink {
        fn refresh(&mut self, _shared: &Shared) {}
        fn client(&self) -> Option<u64> {
            self.connected.then_some(self.client)
        }
        fn send(&mut self, batch: Vec<u8>) -> bool {
            if !self.connected {
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
    }

    struct Rig {
        shared: &'static Shared,
        master: LlMaster<FakeDesk, FakeLink>,
    }

    impl Rig {
        fn new(connected: bool) -> Rig {
            Rig::with(connected, LAYOUT)
        }

        fn with(connected: bool, layout: &str) -> Rig {
            let shared: &'static Shared = Box::leak(Box::new(Shared::new()));
            shared
                .engine
                .lock()
                .unwrap()
                .load(ZonesLayout::from_xml(layout).unwrap());
            shared.want_hook.store(true, Ordering::SeqCst);
            let desk = FakeDesk {
                cursor: (1900, 500),
                ..FakeDesk::default()
            };
            let link = FakeLink {
                connected,
                client: 1,
                ..FakeLink::default()
            };
            let mut master = LlMaster::new(desk, link);
            // The engine starts tracking where the cursor is.
            let mut env = Env {
                desk: &mut master.desk,
                pos: &mut master.pos,
                clip: &mut master.clip,
                bounds: Rect::new(0, 0, 1920, 1080),
                desktop: Rect::new(0, 0, 1920, 1080),
                away: false,
            };
            shared
                .engine
                .lock()
                .unwrap()
                .on_mouse_move(&mut env, &mut MouseEventArg::new(Point::new(1900, 500)));
            Rig { shared, master }
        }

        /// A move the system reports at `cursor + (dx, dy)`, past the desktop's edge
        /// if it is, and what it then does with the cursor: puts it there, clamped,
        /// unless the hook swallowed it.
        fn nudge(&mut self, dx: i32, dy: i32) -> Option<Verdict> {
            let (x, y) = self.master.desk.cursor;
            let report = Point::new(x + dx, y + dy);
            let verdict = self.master.on_move(self.shared, report);
            if verdict.is_none() || verdict == Some(Verdict::Pass) {
                let at = clamp(report, self.master.desk.desktop());
                self.master.desk.cursor = (at.x(), at.y());
            }
            verdict
        }

        fn sent(&mut self) -> Vec<Wire> {
            std::mem::take(&mut self.master.link.sent)
        }

        fn key(&mut self, code: u16, vk: u16, down: bool) -> Verdict {
            if down {
                self.master.desk.keys_down.push(vk);
            }
            let verdict = self
                .master
                .on_key(self.shared, Some(code), vk, down, Instant::now());
            if !down || verdict == Verdict::Swallow {
                self.master.desk.keys_down.retain(|&k| k != vk);
            }
            verdict
        }

        fn go_over(&mut self) {
            assert_eq!(self.nudge(40, 0), Some(Verdict::Swallow));
            assert!(self.master.is_remote(), "the pointer went over");
            assert_eq!(self.sent()[0], Wire::Enter { x: 0, y: 500 });
        }
    }

    #[test]
    fn without_a_client_the_hook_routes_as_it_always_did() {
        let mut rig = Rig::new(false);
        assert_eq!(rig.nudge(5, 0), None);
        assert_eq!(rig.nudge(40, 0), None);
        assert!(!rig.master.is_remote());
    }

    #[test]
    fn without_the_keyboard_the_other_machine_is_behind_a_wall() {
        let mut rig = Rig::new(true);
        rig.master.desk.no_keyboard = true;
        assert_eq!(rig.nudge(40, 0), Some(Verdict::Pass));
        assert!(!rig.master.is_remote(), "the pointer stays here");
        assert!(rig.sent().is_empty());
        // Once the keys can be taken, the border opens.
        rig.master.desk.no_keyboard = false;
        rig.nudge(-30, 0);
        rig.go_over();
    }

    #[test]
    fn crossing_out_freezes_the_cursor_and_motion_goes_over_as_displacement() {
        let mut rig = Rig::new(true);
        assert_eq!(
            rig.nudge(5, 0),
            Some(Verdict::Pass),
            "a move here is the system's"
        );
        rig.go_over();
        assert_eq!(rig.master.desk.cursor, (1905, 500), "frozen where it left");
        assert_eq!(rig.master.desk.hides, 1);

        // At the border, the system reports the border again: no motion is lost to it,
        // because the report is read against the cursor, which did not move.
        assert_eq!(rig.nudge(10, 3), Some(Verdict::Swallow));
        assert_eq!(rig.sent(), vec![Wire::Motion { x: 10, y: 503 }]);
        assert_eq!(rig.master.desk.cursor, (1905, 500));

        // Once what hides the cursor is in place, the cursor goes under it, in the
        // middle of the monitor, and the reports are read from there.
        rig.master.desk.placed = true;
        rig.nudge(1, 0);
        assert_eq!(rig.master.desk.cursor, SPOT);
        rig.sent();
        assert_eq!(rig.nudge(-7, 2), Some(Verdict::Swallow));
        assert_eq!(rig.sent(), vec![Wire::Motion { x: 4, y: 505 }]);
        assert_eq!(rig.master.desk.cursor, SPOT, "still frozen there");
    }

    #[test]
    fn coming_back_puts_the_cursor_where_the_engine_landed_and_shows_it() {
        let mut rig = Rig::new(true);
        rig.go_over();
        rig.master.desk.placed = true;
        rig.nudge(1, 0);
        rig.sent();
        assert_eq!(rig.nudge(-40, 0), Some(Verdict::Swallow));
        assert!(!rig.master.is_remote(), "the pointer came back");
        assert_eq!(rig.sent(), vec![Wire::Leave]);
        assert_eq!(rig.master.desk.shows, 1);
        let (x, y) = rig.master.desk.cursor;
        assert!(
            (1880..1920).contains(&x) && y == 500,
            "landed at the border, at ({x},{y})"
        );
        // And moves here are the system's again.
        assert_eq!(rig.nudge(-5, 0), Some(Verdict::Pass));
    }

    #[test]
    fn a_confinement_set_while_away_is_the_systems_once_home() {
        let mut rig = Rig::new(true);
        rig.go_over();
        rig.master.clip = Some(Rect::new(0, 0, 1920, 1080));
        rig.nudge(-60, 0);
        assert!(!rig.master.is_remote());
        rig.nudge(-1, 0);
        assert_eq!(rig.master.clip, None, "no longer the engine's own idea");
        assert_eq!(rig.master.desk.clip, Some(Rect::new(0, 0, 1920, 1080)));
    }

    #[test]
    fn buttons_and_wheels_over_there_are_swallowed_and_sent() {
        let mut rig = Rig::new(true);
        rig.go_over();
        assert_eq!(
            rig.master.on_button(rig.shared, BTN_LEFT, Press::Down),
            Some(Verdict::Swallow)
        );
        assert_eq!(
            rig.master.on_button(rig.shared, BTN_LEFT, Press::Up),
            Some(Verdict::Swallow)
        );
        assert_eq!(
            rig.master.on_wheel(rig.shared, 0, -120),
            Some(Verdict::Swallow)
        );
        assert_eq!(
            rig.sent(),
            vec![
                Wire::Button {
                    code: BTN_LEFT,
                    pressed: true
                },
                Wire::Button {
                    code: BTN_LEFT,
                    pressed: false
                },
                Wire::Wheel {
                    horizontal: 0,
                    vertical: -120
                },
            ]
        );
    }

    #[test]
    fn a_button_held_across_the_border_is_released_here() {
        let mut rig = Rig::new(true);
        // Pressed here: the system has it, the master does not look.
        assert_eq!(
            rig.master.on_button(rig.shared, BTN_LEFT, Press::Down),
            None
        );
        rig.go_over();
        assert_eq!(
            rig.master.on_button(rig.shared, BTN_LEFT, Press::Up),
            Some(Verdict::Pass),
            "released where it was pressed"
        );
        assert!(rig.sent().is_empty());
    }

    #[test]
    fn the_keyboard_is_grabbed_over_there_and_let_go_coming_back() {
        let mut rig = Rig::new(true);
        assert_eq!(rig.key(KEY_A, VK_A, true), Verdict::Pass);
        assert_eq!(rig.key(KEY_A, VK_A, false), Verdict::Pass);
        rig.go_over();
        assert_eq!(rig.key(KEY_A, VK_A, true), Verdict::Swallow);
        assert_eq!(rig.key(KEY_A, VK_A, true), Verdict::Swallow, "autorepeat");
        assert_eq!(rig.key(KEY_A, VK_A, false), Verdict::Swallow);
        assert_eq!(
            rig.sent(),
            vec![
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
        rig.nudge(-60, 0);
        assert!(!rig.master.is_remote());
        rig.sent();
        assert_eq!(
            rig.key(KEY_A, VK_A, true),
            Verdict::Pass,
            "typing here again"
        );
    }

    #[test]
    fn a_modifier_held_while_crossing_keeps_the_keyboard_here_until_released() {
        let mut rig = Rig::new(true);
        assert_eq!(rig.key(KEY_LEFTCTRL, VK_CONTROL, true), Verdict::Pass);
        rig.go_over();
        // Ctrl is down here: Ctrl+A is this machine's.
        assert_eq!(rig.key(KEY_A, VK_A, true), Verdict::Pass);
        assert_eq!(rig.key(KEY_A, VK_A, false), Verdict::Pass);
        assert_eq!(rig.key(KEY_LEFTCTRL, VK_CONTROL, false), Verdict::Pass);
        // Now nothing is held here: the next key goes over.
        assert_eq!(rig.key(KEY_A, VK_A, true), Verdict::Swallow);
        let sent = rig.sent();
        assert_eq!(
            &sent[sent.len() - 1..],
            &[Wire::Key {
                code: KEY_A,
                pressed: true
            }]
        );
    }

    #[test]
    fn a_release_whose_press_went_over_there_is_not_passed_on_here() {
        let mut rig = Rig::new(true);
        rig.go_over();
        assert_eq!(rig.key(KEY_A, VK_A, true), Verdict::Swallow);
        rig.nudge(-60, 0);
        assert!(!rig.master.is_remote());
        assert!(
            rig.sent().contains(&Wire::Leave),
            "Leave released it over there"
        );
        assert_eq!(rig.key(KEY_A, VK_A, false), Verdict::Swallow);
    }

    #[test]
    fn the_client_going_away_brings_the_pointer_home() {
        let mut rig = Rig::new(true);
        rig.go_over();
        rig.master.link.connected = false;
        rig.master.on_tick(rig.shared);
        assert!(!rig.master.is_remote());
        assert_eq!(rig.master.desk.cursor, (1900, 500), "where it left");
        assert_eq!(rig.master.desk.shows, 1);
        // The other machine's border is a wall now, and the routing is the hook's again
        // once the engine has been told.
        assert_eq!(rig.nudge(40, 0), Some(Verdict::Pass));
        assert!(!rig.master.is_remote());
        assert_eq!(rig.nudge(-5, 0), None);
    }

    #[test]
    fn the_desktop_switching_brings_the_pointer_home() {
        let mut rig = Rig::new(true);
        rig.go_over();
        assert_eq!(rig.key(KEY_LEFTCTRL, VK_CONTROL, true), Verdict::Swallow);
        rig.sent();
        rig.master.shut_down();
        assert!(!rig.master.is_remote());
        assert_eq!(
            rig.sent(),
            vec![Wire::Leave],
            "which releases Ctrl over there"
        );
    }

    #[test]
    fn the_lock_keys_go_over_with_the_enter() {
        let mut rig = Rig::new(true);
        rig.master.desk.locks = Locks {
            caps: Some(true),
            num: Some(false),
            scroll: None,
        };
        assert_eq!(rig.nudge(45, 0), Some(Verdict::Swallow));
        let sent = rig.sent();
        assert_eq!(sent[0], Wire::Enter { x: 0, y: 500 });
        assert_eq!(sent[1], Wire::Locks(rig.master.desk.locks));
    }

    #[test]
    fn the_rescue_held_over_there_brings_the_pointer_home() {
        let mut rig = Rig::new(true);
        *rig.shared.rescue_shortcut.lock().unwrap() = "Ctrl+Alt+Shift+M".to_owned();
        rig.go_over();
        // Pressed long enough ago for the hold to be over.
        let hold = crate::hook::crossing::keys::HOLD + std::time::Duration::from_millis(50);
        let start = Instant::now().checked_sub(hold).unwrap();
        for (code, vk) in [(29, 0xA2), (56, 0xA4), (42, 0xA0), (50, 0x4D)] {
            rig.master.desk.keys_down.push(vk);
            let verdict = rig.master.on_key(rig.shared, Some(code), vk, true, start);
            assert_eq!(verdict, Verdict::Swallow);
        }
        rig.master.on_tick(rig.shared);
        assert!(!rig.master.is_remote(), "the rescue brought it home");
        assert!(rig.sent().contains(&Wire::Leave));
    }

    #[test]
    fn on_a_french_keyboard_the_rescue_is_its_m_key_over_there() {
        let mut rig = Rig::new(true);
        *rig.shared.rescue_shortcut.lock().unwrap() = "Ctrl+Alt+Shift+M".to_owned();
        let mut places = [None; 26];
        places[(b'M' - b'A') as usize] = Some(39);
        *rig.shared.letters.lock().unwrap() =
            crate::hook::crossing::keys::Letters::from_places(places);
        rig.shared.shortcuts_revision.fetch_add(1, Ordering::SeqCst);
        rig.go_over();
        let hold = crate::hook::crossing::keys::HOLD + std::time::Duration::from_millis(50);
        let start = Instant::now().checked_sub(hold).unwrap();
        // The key marked M on that keyboard, where US has ';'.
        for (code, vk) in [(29, 0xA2), (56, 0xA4), (42, 0xA0), (39, 0x4D)] {
            rig.master.desk.keys_down.push(vk);
            assert_eq!(
                rig.master.on_key(rig.shared, Some(code), vk, true, start),
                Verdict::Swallow
            );
        }
        rig.master.on_tick(rig.shared);
        assert!(!rig.master.is_remote(), "the rescue brought it home");
    }

    #[test]
    fn a_key_the_stream_cannot_carry_is_left_to_the_system() {
        let mut rig = Rig::new(true);
        rig.go_over();
        assert_eq!(
            rig.master
                .on_key(rig.shared, None, 0xFF, true, Instant::now()),
            Verdict::Pass
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

    fn three_machines(drivable: Option<&[&str]>) -> Rig {
        let rig = Rig::with(true, THREE_MACHINES);
        *rig.shared.drivable.lock().unwrap() =
            drivable.map(|hosts| hosts.iter().map(|h| (*h).to_owned()).collect());
        rig.shared.drivable_revision.fetch_add(1, Ordering::SeqCst);
        rig
    }

    fn hosts_of(routed: &[(Option<String>, Wire)], wanted: fn(&Wire) -> bool) -> Vec<String> {
        routed
            .iter()
            .filter(|(_, e)| wanted(e))
            .map(|(h, _)| h.clone().unwrap_or_default())
            .collect()
    }

    #[test]
    fn the_pointer_goes_from_one_remote_straight_to_the_next_and_back_home() {
        let mut rig = three_machines(Some(&["abcd", "efgh"]));
        assert_eq!(rig.nudge(40, 0), Some(Verdict::Swallow));
        assert!(rig.master.is_remote());
        for _ in 0..6 {
            rig.nudge(500, 0);
        }
        let enters = hosts_of(&rig.master.link.routed, |e| matches!(e, Wire::Enter { .. }));
        let leaves = hosts_of(&rig.master.link.routed, |e| matches!(e, Wire::Leave));
        assert_eq!(enters, ["abcd", "efgh"], "{:?}", rig.master.link.routed);
        assert_eq!(leaves, ["abcd"]);
        assert_eq!(rig.master.desk.shows, 0, "still hidden on the way");
        assert_eq!(rig.master.desk.hides, 1);
        // Motion over B is in B's pixels.
        let last = rig.master.link.routed.last().unwrap().clone();
        assert_eq!(last.0.as_deref(), Some("efgh"));
        assert!(
            matches!(last.1, Wire::Motion { x, .. } if (0..1920).contains(&x)),
            "{last:?}"
        );

        rig.master.link.routed.clear();
        for _ in 0..12 {
            rig.nudge(-500, 0);
        }
        assert!(!rig.master.is_remote(), "home");
        let enters = hosts_of(&rig.master.link.routed, |e| matches!(e, Wire::Enter { .. }));
        let leaves = hosts_of(&rig.master.link.routed, |e| matches!(e, Wire::Leave));
        assert_eq!(enters, ["abcd"]);
        assert_eq!(leaves, ["efgh", "abcd"]);
        assert_eq!(rig.master.desk.shows, 1);
    }

    #[test]
    fn a_machine_the_agent_does_not_drive_is_behind_a_wall() {
        let mut rig = three_machines(Some(&["abcd"]));
        rig.nudge(40, 0);
        for _ in 0..6 {
            rig.nudge(500, 0);
        }
        assert!(rig.master.is_remote());
        let enters = hosts_of(&rig.master.link.routed, |e| matches!(e, Wire::Enter { .. }));
        assert_eq!(
            enters,
            ["abcd"],
            "B is a wall: {:?}",
            rig.master.link.routed
        );
        assert!(!rig
            .master
            .link
            .routed
            .iter()
            .any(|(_, e)| *e == Wire::Leave));
    }

    #[test]
    fn named_by_no_agent_the_next_remote_brings_the_pointer_home() {
        let mut rig = three_machines(None);
        rig.nudge(40, 0);
        for _ in 0..6 {
            rig.nudge(500, 0);
        }
        assert!(!rig.master.is_remote(), "an older agent drives one: home");
        assert!(rig.master.link.routed.iter().all(|(h, _)| h.is_none()));
    }
}
