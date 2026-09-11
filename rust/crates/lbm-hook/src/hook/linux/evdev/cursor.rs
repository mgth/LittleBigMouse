//! The authoritative cursor: where LBM believes the pointer is, and the state
//! the engine reads about it.
//!
//! Unlike every other backend's environment, this one answers from its own
//! fields rather than from the compositor. LBM is the sole source of pointer
//! motion here, so the position it last emitted *is* the position — there is
//! nothing to query and nothing that can disagree.

use std::time::Instant;

use evdev::{EventType, InputEvent, KeyCode};

use crate::engine::cursor::CursorEnv;
use crate::geometry::{Point, Rect};

use super::BTN_MOUSE_RANGE;

/// CursorEnv over the authoritative virtual position. `set_mouse_location` is a
/// pure state update (the caller emits the absolute point afterwards); the clip
/// is the emulated Win32 ClipCursor the engine's travel path relies on.
///
/// Public (with private fields) for the same reason as [`super::PumpBuffers`]:
/// the classification benchmark tracks buttons and modifiers through the real
/// cursor rather than a stand-in.
pub struct EvdevCursor {
    /// Where the pointer is. The router writes it (from the engine's verdict or
    /// its own clamp) and emits it; nothing else in the process knows better.
    pub(super) virtual_pos: Point<i32>,
    clip: Option<Rect<i32>>,
    /// The ABS range the virtual pointer was built with. Rebuilt by the router
    /// when a new layout changes the desktop under us.
    pub(super) desktop: Rect<i32>,
    pub(super) started: Instant,
    /// Modifier state fed by the observed keyboards and the grabbed combined
    /// nodes; left/right tracked apart so releasing one keeps the other held.
    ctrl_left: bool,
    ctrl_right: bool,
    /// Bitmask of held [`BTN_MOUSE_RANGE`] buttons, one bit per code. Tracked
    /// from the grabbed devices' own stream, so it costs nothing to read, and
    /// seeded by the router at arm time from `EVIOCGKEY`.
    pub(super) buttons: u8,
}

impl EvdevCursor {
    /// A cursor sitting at `start` on `desktop`, nothing held. The grabbed
    /// devices' real button state is seeded by the caller (`EVIOCGKEY` at arm
    /// time — a press that predates the grab never reached the pump).
    pub fn new(desktop: Rect<i32>, start: Point<i32>) -> EvdevCursor {
        EvdevCursor {
            virtual_pos: start,
            clip: None,
            desktop,
            started: Instant::now(),
            ctrl_left: false,
            ctrl_right: false,
            buttons: 0,
        }
    }

    pub(super) fn track_ctrl(&mut self, code: u16, value: i32) {
        // value: 1 press, 2 autorepeat, 0 release.
        if code == KeyCode::KEY_LEFTCTRL.0 {
            self.ctrl_left = value != 0;
        } else if code == KeyCode::KEY_RIGHTCTRL.0 {
            self.ctrl_right = value != 0;
        }
    }

    /// The [`BTN_MOUSE_RANGE`] codes currently held, lowest code first.
    fn held_buttons(&self) -> impl Iterator<Item = u16> + '_ {
        let start = *BTN_MOUSE_RANGE.start();
        (0..=(BTN_MOUSE_RANGE.end() - BTN_MOUSE_RANGE.start()) as u8)
            .filter(move |i| self.buttons & (1u8 << i) != 0)
            .map(move |i| start + i as u16)
    }

    /// The frame the compositor is still owed when the virtual pointer goes
    /// away: one release per button left held. Empty in the ordinary case, which
    /// is why `Drop` can emit it unconditionally.
    pub(super) fn release_frame(&self) -> Vec<InputEvent> {
        self.held_buttons()
            .map(|code| InputEvent::new(EventType::KEY.0, code, 0))
            .collect()
    }

    pub(super) fn track_button(&mut self, code: u16, value: i32) {
        if !BTN_MOUSE_RANGE.contains(&code) {
            return;
        }
        let bit = 1u8 << (code - BTN_MOUSE_RANGE.start());
        if value != 0 {
            self.buttons |= bit;
        } else {
            self.buttons &= !bit;
        }
    }

    pub(super) fn clamp(&self, p: Point<i32>) -> Point<i32> {
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

    fn buttons_down(&self) -> bool {
        self.buttons != 0
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

#[cfg(test)]
mod tests {
    use super::*;

    fn cursor() -> EvdevCursor {
        EvdevCursor::new(Rect::new(0, 0, 1920, 1080), Point::new(0, 0))
    }

    #[test]
    fn nothing_held_owes_no_release() {
        assert_eq!(cursor().held_buttons().count(), 0);
    }

    #[test]
    fn a_held_button_is_owed_a_release() {
        let mut c = cursor();
        c.track_button(KeyCode::BTN_LEFT.0, 1);
        assert_eq!(
            c.held_buttons().collect::<Vec<_>>(),
            vec![KeyCode::BTN_LEFT.0]
        );
    }

    #[test]
    fn releasing_clears_the_debt() {
        let mut c = cursor();
        c.track_button(KeyCode::BTN_LEFT.0, 1);
        c.track_button(KeyCode::BTN_LEFT.0, 0);
        assert_eq!(c.held_buttons().count(), 0);
    }

    /// The regression this module's `Drop` exists for: dropping the router
    /// mid-press owes exactly the buttons still down, and only those.
    #[test]
    fn only_the_still_held_buttons_are_owed() {
        let mut c = cursor();
        c.track_button(KeyCode::BTN_LEFT.0, 1);
        c.track_button(KeyCode::BTN_RIGHT.0, 1);
        c.track_button(KeyCode::BTN_LEFT.0, 0);
        assert_eq!(
            c.held_buttons().collect::<Vec<_>>(),
            vec![KeyCode::BTN_RIGHT.0]
        );
    }

    /// What `Drop` actually hands to uinput: EV_KEY, the held code, value 0.
    /// A press (or a wrong event type) here would leave the seat exactly as
    /// stuck as emitting nothing.
    #[test]
    fn the_owed_frame_is_a_key_release() {
        let mut c = cursor();
        c.track_button(KeyCode::BTN_LEFT.0, 1);

        let frame = c.release_frame();

        assert_eq!(frame.len(), 1);
        assert_eq!(frame[0].event_type(), EventType::KEY);
        assert_eq!(frame[0].code(), KeyCode::BTN_LEFT.0);
        assert_eq!(frame[0].value(), 0, "a press would not unstick the button");
    }

    /// The ordinary teardown: nothing held, nothing emitted. `Drop` skips the
    /// uinput write entirely on an empty frame.
    #[test]
    fn an_idle_teardown_owes_an_empty_frame() {
        assert!(cursor().release_frame().is_empty());
    }

    /// Autorepeat (value 2) is a press, not a second one: the mask must not
    /// double-count it, and the button must still be owed a single release.
    #[test]
    fn autorepeat_still_counts_as_held() {
        let mut c = cursor();
        c.track_button(KeyCode::BTN_LEFT.0, 1);
        c.track_button(KeyCode::BTN_LEFT.0, 2);
        assert_eq!(
            c.held_buttons().collect::<Vec<_>>(),
            vec![KeyCode::BTN_LEFT.0]
        );
    }

    /// Every code the virtual pointer declares must round-trip through the
    /// bitmask: BTN_TASK is bit 7, so a narrower mask would silently drop it.
    #[test]
    fn the_whole_declared_range_round_trips() {
        for code in BTN_MOUSE_RANGE {
            let mut c = cursor();
            c.track_button(code, 1);
            assert_eq!(
                c.held_buttons().collect::<Vec<_>>(),
                vec![code],
                "code {code:#x} did not round-trip"
            );
        }
    }

    /// Codes outside the pointer's declared set (tablet/touch, joystick) must
    /// not touch the mask — shifting by their offset would overflow the u8.
    #[test]
    fn codes_outside_the_range_are_ignored() {
        let mut c = cursor();
        c.track_button(KeyCode::BTN_TOOL_PEN.0, 1);
        c.track_button(KeyCode::BTN_TOUCH.0, 1);
        c.track_button(KeyCode::BTN_0.0, 1);
        assert_eq!(c.held_buttons().count(), 0);
    }
}
