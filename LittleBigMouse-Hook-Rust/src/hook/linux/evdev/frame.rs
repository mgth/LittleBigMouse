//! Reading and routing one report: the poll set, the event drain, and the
//! composition of the frames the two virtual devices are handed.
//!
//! Everything here works on buffers [`PumpBuffers`] owns for the whole hooked
//! session. The pump runs up to once per mouse report — ~1000/s per grabbed
//! device — and the sizes never change from one cycle to the next, so the
//! routing path allocates nothing once the capacities have settled. That is part
//! of the module's "nothing potentially blocking" rule: a call into the global
//! allocator a thousand times a second, on the thread that owns the user's
//! grabbed mice, is a stall waiting to happen.
//!
//! No device and no uinput handle appear in this file, which is what lets
//! `benches/evdev_pump.rs` measure the routing path unprivileged and lets the
//! tests below cover it without opening anything.

use std::os::fd::RawFd;

use evdev::{AbsoluteAxisCode, EventType, InputEvent, RelativeAxisCode};

use super::cursor::EvdevCursor;
use super::{BTN_RANGE, BTN_TRIGGER_HAPPY_RANGE};

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

/// Every buffer the pump reuses, owned by the router for the whole hooked
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

    /// How many slots the poll set holds — mice first, then keyboards, which is
    /// how a slot index maps back to a device list.
    pub(super) fn slots(&self) -> usize {
        self.fds.len()
    }

    /// Wait up to `timeout_ms` for any slot to become readable. The only wait
    /// the routing thread is allowed (module rule), and the reason the poll set
    /// is a buffer and not a fresh `Vec`: this is the one place the raw fds are
    /// handed to the kernel.
    pub(super) fn poll(&mut self, timeout_ms: i32) {
        unsafe {
            libc::poll(
                self.fds.as_mut_ptr(),
                self.fds.len() as libc::nfds_t,
                timeout_ms,
            );
        }
    }

    /// What the last [`poll`](Self::poll) reported for a slot.
    pub(super) fn revents(&self, slot: usize) -> i16 {
        self.fds[slot].revents
    }

    /// Remember that a slot has to leave the device lists, once the cycle is
    /// done walking them. Recorded ascending, which is what
    /// [`purge_dead`](super::devices::purge_dead) relies on.
    pub(super) fn mark_dead(&mut self, slot: usize) {
        self.dead.push(slot);
    }

    /// The slots marked dead this cycle.
    pub(super) fn dead_slots(&self) -> &[usize] {
        &self.dead
    }

    /// Forget them, once they have been removed from the device lists — the
    /// indices mean nothing against the next poll set.
    pub(super) fn clear_dead(&mut self) {
        self.dead.clear();
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

    /// Which mouse produced the motion being accumulated, as a poll slot — the
    /// device whose acceleration curve the flush feeds. Slots below the mouse
    /// count are the grabbed mice, so this doubles as an index into them.
    pub(super) fn last_motion_dev(&self) -> usize {
        self.last_motion_dev
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

#[cfg(test)]
mod tests {
    use super::*;

    use evdev::KeyCode;

    use crate::engine::cursor::CursorEnv;
    use crate::geometry::{Point, Rect};

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
}
