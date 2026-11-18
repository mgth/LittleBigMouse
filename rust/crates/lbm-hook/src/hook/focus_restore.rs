//! Decision rules for a delayed, best-effort foreground restoration.
//! Idle timestamps must be sampled with GetLastInputInfo at both ends. A promoted
//! mouse report's timestamp is not the same observation as the session idle clock.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WindowIdentity {
    pub handle: isize,
    pub process: u32,
    pub thread: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FocusSnapshot {
    pub window: WindowIdentity,
    pub control: Option<WindowIdentity>,
}

/// Touch activation can precede the first promoted mouse report. In that case
/// use the focus captured during the preceding physical mouse activity.
pub fn before_touch(
    current: Option<FocusSnapshot>,
    previous: Option<FocusSnapshot>,
    touched: Option<WindowIdentity>,
) -> Option<FocusSnapshot> {
    match current {
        Some(current) if Some(current.window) != touched => Some(current),
        _ => previous.filter(|p| Some(p.window) != touched).or(current),
    }
}

#[derive(Clone, Copy)]
pub struct RestoreRequest {
    pub origin: WindowIdentity,
    pub control: Option<WindowIdentity>,
    pub touched: WindowIdentity,
    pub input_time: u32,
    pub generation: u32,
    pub sequence: u64,
}

impl RestoreRequest {
    pub fn should_restore(
        &self,
        origin_now: Option<WindowIdentity>,
        foreground_now: Option<WindowIdentity>,
        last_input: Option<u32>,
        active: bool,
        generation: u32,
        sequence: u64,
    ) -> bool {
        active
            && generation == self.generation
            && sequence == self.sequence
            && self.origin != self.touched
            && origin_now == Some(self.origin)
            && (foreground_now == Some(self.touched) || foreground_now == Some(self.origin))
            && last_input == Some(self.input_time)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn window(handle: isize) -> WindowIdentity {
        WindowIdentity {
            handle,
            process: 10,
            thread: 20,
        }
    }

    fn request() -> RestoreRequest {
        RestoreRequest {
            origin: window(1),
            control: Some(window(11)),
            touched: window(2),
            input_time: u32::MAX,
            generation: 3,
            sequence: 4,
        }
    }

    #[test]
    fn restores_only_if_touch_is_still_the_last_user_action() {
        let r = request();
        assert!(r.should_restore(Some(window(1)), Some(window(2)), Some(u32::MAX), true, 3, 4));
        for time in [None, Some(0), Some(100)] {
            assert!(!r.should_restore(Some(window(1)), Some(window(2)), time, true, 3, 4));
        }
    }

    #[test]
    fn stop_reload_new_touch_or_explicit_window_switch_cancels() {
        let r = request();
        for (active, generation, sequence) in [(false, 3, 4), (true, 5, 4), (true, 3, 6)] {
            assert!(!r.should_restore(
                Some(window(1)),
                Some(window(2)),
                Some(u32::MAX),
                active,
                generation,
                sequence
            ));
        }
        for foreground in [None, Some(window(3))] {
            assert!(!r.should_restore(Some(window(1)), foreground, Some(u32::MAX), true, 3, 4));
        }
    }

    #[test]
    fn closed_or_reused_origin_and_same_window_taps_are_ignored() {
        let mut r = request();
        for origin in [
            None,
            Some(WindowIdentity {
                process: 99,
                ..window(1)
            }),
        ] {
            assert!(!r.should_restore(origin, Some(window(2)), Some(u32::MAX), true, 3, 4));
        }
        r.touched = r.origin;
        assert!(!r.should_restore(Some(window(1)), Some(window(1)), Some(u32::MAX), true, 3, 4));
    }

    #[test]
    fn already_active_original_still_needs_its_editor_focus_restored() {
        let r = request();
        assert!(r.should_restore(Some(window(1)), Some(window(1)), Some(u32::MAX), true, 3, 4));
        assert_eq!(r.control, Some(window(11)));
    }

    #[test]
    fn touch_that_activates_before_mouse_promotion_keeps_the_previous_editor() {
        let editor = FocusSnapshot {
            window: window(1),
            control: Some(window(11)),
        };
        let touched = FocusSnapshot {
            window: window(2),
            control: Some(window(22)),
        };
        assert_eq!(
            before_touch(Some(touched), Some(editor), Some(window(2))),
            Some(editor)
        );
        // A deliberate switch to a third app beats the cached earlier app.
        let other = FocusSnapshot {
            window: window(3),
            control: None,
        };
        assert_eq!(
            before_touch(Some(other), Some(editor), Some(window(2))),
            Some(other)
        );
        assert_eq!(before_touch(None, None, Some(window(2))), None);
    }
}
