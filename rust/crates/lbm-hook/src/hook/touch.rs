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
///
/// Promotion only speaks for its own gesture. The hook callback can overtake the
/// queued `WM_INPUT` of the same tap, so each promoted button press is paired with
/// one raw contact, before or after it. A contact left over once they are all
/// paired starts a gesture of its own.
#[derive(Clone, Copy, Default)]
pub struct NativeTouchResume {
    saved: Option<(i32, i32)>,
    bypass: bool,
    promoted: bool,
    contact: bool,
    /// A raw contact began and no promoted press has claimed it yet.
    unclaimed_contact: bool,
    /// Promoted presses whose raw contact has not been reported yet.
    awaited_contacts: u8,
}

impl NativeTouchResume {
    pub const fn new() -> Self {
        Self {
            saved: None,
            bypass: false,
            promoted: false,
            contact: false,
            unclaimed_contact: false,
            awaited_contacts: 0,
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
            self.begin_contact();
        }
        self.contact = contact;
        self.observe(cursor, allowed);
    }

    fn begin_contact(&mut self) {
        if self.awaited_contacts > 0 {
            // The report of a tap already promoted, arriving late.
            self.awaited_contacts -= 1;
        } else {
            self.promoted = false;
            self.unclaimed_contact = true;
        }
    }

    /// A promoted mouse message; `press` when it is the button press that opens
    /// the gesture, as moves and releases can trail behind it.
    pub fn promoted(&mut self, press: bool) {
        if press && !std::mem::take(&mut self.unclaimed_contact) {
            self.awaited_contacts = self.awaited_contacts.saturating_add(1);
        }
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

/// How long a contact may stay silent before a moving physical mouse ends it.
/// Mouse routing is suspended during a contact, so a release that never reaches
/// the hook (a gesture taken over by the system, say) must not leave it off.
pub const CONTACT_SILENCE_MS: u32 = 2_000;

#[derive(Clone, Copy, Default)]
pub struct TouchResume {
    saved: Option<(i32, i32)>,
    contact: bool,
    last_report: u32,
}

impl TouchResume {
    pub const fn new() -> Self {
        Self {
            saved: None,
            contact: false,
            last_report: 0,
        }
    }

    /// `time` is the report's `MSLLHOOKSTRUCT::time`, a wrapping millisecond tick.
    pub fn touch(
        &mut self,
        cursor_before_touch: Option<(i32, i32)>,
        down: bool,
        up: bool,
        time: u32,
    ) {
        self.last_report = time;
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

    /// End a contact nothing has reported on for [`CONTACT_SILENCE_MS`]. Returns
    /// whether it did, so the caller can forget the gesture too.
    pub fn expire_contact(&mut self, now: u32) -> bool {
        let lapsed = self.contact && now.wrapping_sub(self.last_report) >= CONTACT_SILENCE_MS;
        if lapsed {
            self.contact = false;
        }
        lapsed
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
            state.promoted(true);
            state.observe(Some((10, 20)), true);
            assert_eq!(state.target(true), None);
        }
    }

    #[test]
    fn a_promoted_override_does_not_outlive_its_gesture() {
        let anchor = Some((10, 20));
        let mut state = NativeTouchResume::new();
        // Override tap in an app that gets promoted mouse messages...
        state.observe_contact(anchor, false, true);
        state.promoted(true);
        state.observe_contact(anchor, false, false);
        state.promoted(false);
        assert!(!state.pending());
        // ...then a plain tap in an app consuming native touch.
        state.observe_contact(anchor, true, true);
        state.observe(anchor, true);
        state.observe_contact(anchor, true, false);
        assert_eq!(state.target(true), anchor);
    }

    #[test]
    fn promotion_after_the_release_still_belongs_to_that_tap() {
        let anchor = Some((10, 20));
        let mut state = NativeTouchResume::new();
        state.observe_contact(anchor, true, true);
        state.observe_contact(anchor, true, false);
        state.promoted(true);
        assert!(!state.pending());
        state.observe_contact(anchor, true, true);
        state.observe_contact(anchor, true, false);
        assert_eq!(state.target(true), anchor);
    }

    #[test]
    fn a_contact_reported_after_its_promotion_is_the_same_gesture() {
        // The hook callback can overtake the queued WM_INPUT of the same tap.
        let anchor = Some((10, 20));
        let mut state = NativeTouchResume::new();
        state.promoted(true);
        state.observe_contact(anchor, true, true);
        state.observe(anchor, true);
        state.observe_contact(anchor, true, false);
        assert!(!state.pending());
        state.observe_contact(anchor, true, true);
        state.observe_contact(anchor, true, false);
        assert_eq!(state.target(true), anchor);
    }

    #[test]
    fn a_second_promoted_tap_keeps_priority_over_its_late_contact() {
        let anchor = Some((10, 20));
        let mut state = NativeTouchResume::new();
        // First tap: its raw contact is paired with its promotion.
        state.observe_contact(anchor, true, true);
        state.promoted(true);
        state.observe_contact(anchor, true, false);
        state.promoted(false);
        // Second tap, with the override: mouse callbacks overtake its WM_INPUT.
        state.promoted(true);
        state.promoted(false);
        // Finger and key are up by the time the raw reports are handled.
        state.observe(anchor, true);
        state.observe_contact(anchor, true, true);
        state.observe(anchor, true);
        state.observe_contact(anchor, true, false);
        assert_eq!(state.target(true), None);
    }

    #[test]
    fn two_taps_promoted_ahead_of_their_contacts_are_both_paired() {
        let anchor = Some((10, 20));
        let mut state = NativeTouchResume::new();
        for _ in 0..2 {
            state.promoted(true);
            state.promoted(false);
        }
        for _ in 0..2 {
            state.observe_contact(anchor, true, true);
            state.observe_contact(anchor, true, false);
            assert!(!state.pending());
        }
        state.observe_contact(anchor, true, true);
        state.observe_contact(anchor, true, false);
        assert_eq!(state.target(true), anchor);
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
        state.touch(Some((-500, 200)), true, false, 0);
        assert_eq!(state.resume(), None); // never warp during a touch drag
        state.touch(Some((1500, 700)), false, true, 0);
        state.touch(Some((1600, 800)), true, false, 0);
        state.touch(Some((1600, 800)), false, true, 0);
        assert_eq!(state.resume(), Some((-500, 200)));
        assert_eq!(state.resume(), None);
    }

    #[test]
    fn reset_discards_pending_restore_and_contact() {
        let mut state = TouchResume::new();
        state.touch(Some((10, 20)), true, false, 0);
        state.reset();
        assert_eq!(state.resume(), None);
        state.touch(Some((30, 40)), false, false, 0);
        assert_eq!(state.resume(), Some((30, 40)));
    }

    #[test]
    fn a_contact_whose_release_was_lost_ends_after_a_silence() {
        let mut state = TouchResume::new();
        state.touch(Some((10, 20)), true, false, 1_000);
        assert!(!state.expire_contact(1_000 + CONTACT_SILENCE_MS - 1));
        assert!(state.in_contact());
        assert_eq!(state.resume(), None);
        assert!(state.expire_contact(1_000 + CONTACT_SILENCE_MS));
        assert!(!state.in_contact());
        assert!(!state.expire_contact(u32::MAX), "already over");
        assert_eq!(state.resume(), Some((10, 20)));
    }

    #[test]
    fn touch_reports_keep_a_contact_alive_across_the_tick_wrap() {
        let mut state = TouchResume::new();
        let start = u32::MAX - 500;
        state.touch(Some((10, 20)), true, false, start);
        let later = start.wrapping_add(CONTACT_SILENCE_MS - 1);
        state.touch(Some((30, 40)), false, false, later);
        assert!(!state.expire_contact(later.wrapping_add(CONTACT_SILENCE_MS - 1)));
        assert!(state.expire_contact(later.wrapping_add(CONTACT_SILENCE_MS)));
    }

    #[test]
    fn failed_cursor_read_does_not_invent_a_position() {
        let mut state = TouchResume::new();
        state.touch(None, true, false, 0);
        state.touch(None, false, true, 0);
        assert_eq!(state.resume(), None);
    }
}
