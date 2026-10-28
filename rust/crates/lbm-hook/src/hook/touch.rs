//! State for resuming the mouse after Windows promotes touch to mouse messages.
//! Touch events are never swallowed or routed through the monitor-crossing engine.
//! No allocation, OS access, or locks: the Windows adapter owns cursor restoration.

pub fn is_touch(extra: usize) -> bool {
    // https://learn.microsoft.com/windows/win32/tablet/system-events-and-mouse-messages
    extra & 0xffff_ff80 == 0xff51_5780
}

pub fn is_pen(extra: usize) -> bool {
    extra & 0xffff_ff80 == 0xff51_5700
}

/// Fallback for native touch that has no promoted mouse messages. The adapter
/// must supply positive touchscreen evidence; a large mouse delta is not enough.
/// Keep the original anchor across a burst and let promoted events own policy
/// when they are available. An override anywhere in a native burst cancels it.
#[derive(Clone, Copy, Default)]
pub struct NativeTouchResume {
    saved: Option<(i32, i32)>,
    bypass: bool,
    promoted: bool,
    contact: bool,
}

impl NativeTouchResume {
    pub const fn new() -> Self {
        Self {
            saved: None,
            bypass: false,
            promoted: false,
            contact: false,
        }
    }

    pub fn observe(&mut self, cursor: Option<(i32, i32)>, allowed: bool) {
        self.bypass |= !allowed;
        if !self.promoted && !self.bypass && self.saved.is_none() {
            self.saved = cursor;
        }
    }

    /// Only a new contact can re-enable restoration after an override. Releasing
    /// the modifier during the same drag or its trailing reports cannot do so.
    pub fn observe_contact(&mut self, cursor: Option<(i32, i32)>, allowed: bool, contact: bool) {
        if !contact && !self.contact {
            return;
        }
        if contact && !self.contact {
            self.bypass = false;
        }
        self.contact = contact;
        self.observe(cursor, allowed);
    }

    pub fn promoted(&mut self) {
        self.promoted = true;
        self.saved = None;
    }

    pub fn target(&self, selected_display: bool) -> Option<(i32, i32)> {
        if self.bypass || self.promoted || !selected_display {
            None
        } else {
            self.saved
        }
    }

    pub fn pending(&self) -> bool {
        self.target(true).is_some()
    }
}

#[derive(Clone, Copy, Default)]
pub struct TouchResume {
    saved: Option<(i32, i32)>,
    contact: bool,
}

impl TouchResume {
    pub const fn new() -> Self {
        Self {
            saved: None,
            contact: false,
        }
    }

    pub fn touch(&mut self, cursor_before_touch: Option<(i32, i32)>, down: bool, up: bool) {
        if self.saved.is_none() {
            self.saved = cursor_before_touch;
        }
        if down {
            self.contact = true;
        }
        if up {
            self.contact = false;
        }
    }

    pub fn resume(&mut self) -> Option<(i32, i32)> {
        if self.contact {
            None
        } else {
            self.saved.take()
        }
    }

    pub fn in_contact(&self) -> bool {
        self.contact
    }

    pub fn pending(&self) -> bool {
        self.saved.is_some() || self.contact
    }

    pub fn reset(&mut self) {
        *self = Self::new();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_taps_keep_the_original_anchor_until_physical_input() {
        let mut state = NativeTouchResume::new();
        assert_eq!(state.target(true), None);
        state.observe(Some((1600, 873)), true);
        state.observe(Some((2662, 2547)), true);
        assert_eq!(state.target(true), Some((1600, 873)));
        assert_eq!(state.target(false), None);
        state = NativeTouchResume::new(); // physical movement, click, reset, or pen
        assert_eq!(state.target(true), None);
    }

    #[test]
    fn native_override_cannot_rearm_when_modifier_is_released() {
        let mut state = NativeTouchResume::new();
        state.observe(Some((10, 20)), true);
        state.observe(Some((10, 20)), false);
        state.observe(Some((10, 20)), true);
        assert_eq!(state.target(true), None);
    }

    #[test]
    fn native_next_contact_rearms_after_override_without_physical_input() {
        let mut state = NativeTouchResume::new();
        let anchor = Some((10, 20));
        state.observe_contact(anchor, true, false); // idle report is not a tap
        assert!(!state.pending());
        state.observe(anchor, false);
        state.observe_contact(anchor, false, true);
        state.observe_contact(anchor, true, true); // modifier released mid-drag
        assert!(!state.pending());
        state.observe_contact(anchor, true, false);
        state.observe(anchor, true); // trailing raw report
        assert!(!state.pending());
        state.observe_contact(anchor, true, true); // next normal tap
        state.observe_contact(anchor, true, false);
        assert_eq!(state.target(true), anchor);
    }

    #[test]
    fn promoted_policy_wins_regardless_of_raw_message_order() {
        for raw_first in [false, true] {
            let mut state = NativeTouchResume::new();
            if raw_first {
                state.observe(Some((10, 20)), true);
            }
            state.promoted();
            state.observe(Some((10, 20)), true);
            assert_eq!(state.target(true), None);
        }
    }

    #[test]
    fn signatures_distinguish_mouse_pen_and_touch() {
        for id in 0..128 {
            assert!(is_touch(0xff51_5780 | id));
            assert!(!is_pen(0xff51_5780 | id));
            assert!(is_pen(0xff51_5700 | id));
            assert!(!is_touch(0xff51_5700 | id));
        }
        assert!(!is_touch(0));
        assert!(!is_touch(0x80));
    }

    #[test]
    fn repeated_taps_preserve_original_position_and_restore_once() {
        let mut state = TouchResume::new();
        state.touch(Some((-500, 200)), true, false);
        assert_eq!(state.resume(), None); // never warp during a touch drag
        state.touch(Some((1500, 700)), false, true);
        state.touch(Some((1600, 800)), true, false);
        state.touch(Some((1600, 800)), false, true);
        assert_eq!(state.resume(), Some((-500, 200)));
        assert_eq!(state.resume(), None);
    }

    #[test]
    fn reset_discards_pending_restore_and_contact() {
        let mut state = TouchResume::new();
        state.touch(Some((10, 20)), true, false);
        state.reset();
        assert_eq!(state.resume(), None);
        state.touch(Some((30, 40)), false, false);
        assert_eq!(state.resume(), Some((30, 40)));
    }

    #[test]
    fn failed_cursor_read_does_not_invent_a_position() {
        let mut state = TouchResume::new();
        state.touch(None, true, false);
        state.touch(None, false, true);
        assert_eq!(state.resume(), None);
    }
}
