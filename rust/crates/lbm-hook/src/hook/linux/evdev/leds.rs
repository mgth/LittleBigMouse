//! The lock keys of this machine, as the LEDs of its keyboards show them.
//!
//! The compositor (or the X server) lights the LEDs of every keyboard from its one
//! keyboard state, so a keyboard's LEDs are that state wherever it was changed. Read
//! at a crossing, by a master as it sends the pointer over and by a remote as it is
//! told the master's: one `EVIOCGLED` per keyboard, never watched.

use evdev::{Device, LedCode};
use lbm_ipc::input::Locks;

/// Caps Lock, Num Lock and Scroll Lock as one keyboard shows them: `None` for a lock
/// it has no LED for. `None` altogether when it says nothing about its LEDs.
pub(crate) fn leds_of(device: &Device) -> Option<[Option<bool>; 3]> {
    let supported = device.supported_leds()?;
    let lit = device.get_led_state().ok()?;
    let led = |code: LedCode| supported.contains(code).then(|| lit.contains(code));
    Some([
        led(LedCode::LED_CAPSL),
        led(LedCode::LED_NUML),
        led(LedCode::LED_SCROLLL),
    ])
}

/// The locks several keyboards show: on when any shows it on, unknown when none has
/// the LED. Any rather than all, because a keyboard that missed an update stays dark.
pub(crate) fn locks_of(keyboards: impl Iterator<Item = [Option<bool>; 3]>) -> Locks {
    let mut seen = [None; 3];
    for keyboard in keyboards {
        for (seen, lit) in seen.iter_mut().zip(keyboard) {
            if let Some(lit) = lit {
                *seen = Some(seen.unwrap_or(false) || lit);
            }
        }
    }
    let [caps, num, scroll] = seen;
    Locks { caps, num, scroll }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_lock_is_on_when_a_keyboard_shows_it_and_unknown_when_none_can() {
        let none = locks_of(std::iter::empty());
        assert_eq!(none, Locks::default());
        let laptop = [Some(true), Some(false), None];
        let usb = [Some(false), Some(false), None];
        let locks = locks_of([usb, laptop].into_iter());
        assert_eq!(
            (locks.caps, locks.num, locks.scroll),
            (Some(true), Some(false), None)
        );
    }
}
