//! Keys on a master while the pointer is on another machine: which ones are held, and
//! the rescue shortcut recognised in the grabbed stream.
//!
//! While the keyboards are grabbed the compositor sees no key at all, and the desktop's
//! global shortcuts — the portal the rescue listens through (`hook::linux::rescue`) —
//! go deaf with it. The way out of a trapped pointer must not depend on where the
//! pointer is, so the stream this process now reads is watched for the same
//! combination, held for the same time.

use std::time::Instant;

use lbm_ipc::shortcut::{Shortcut, MOD_ALT, MOD_CONTROL, MOD_SHIFT, MOD_WIN};

/// The same hold as through the portal: one rescue, wherever the pointer is.
pub(crate) use crate::hook::linux::rescue::HOLD;

/// A set of `EV_KEY` codes — buttons and keys alike, they share one code space. A
/// bitset over every code the kernel defines (`KEY_MAX` = 0x2ff), so tracking a press
/// never allocates; a code beyond it is never held.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct HeldSet([u64; 12]);

impl HeldSet {
    fn slot(code: u16) -> Option<(usize, u64)> {
        let index = code as usize / 64;
        (index < 12).then(|| (index, 1u64 << (code % 64)))
    }

    pub(crate) fn contains(&self, code: u16) -> bool {
        Self::slot(code).is_some_and(|(i, bit)| self.0[i] & bit != 0)
    }

    pub(crate) fn insert(&mut self, code: u16) {
        if let Some((i, bit)) = Self::slot(code) {
            self.0[i] |= bit;
        }
    }

    /// Whether it was held.
    pub(crate) fn remove(&mut self, code: u16) -> bool {
        let held = self.contains(code);
        if let Some((i, bit)) = Self::slot(code) {
            self.0[i] &= !bit;
        }
        held
    }

    /// Follows one `EV_KEY` event: 1 presses, 0 releases, 2 (autorepeat) changes
    /// nothing.
    pub(crate) fn track(&mut self, code: u16, value: i32) {
        match value {
            0 => {
                self.remove(code);
            }
            1 => self.insert(code),
            _ => {}
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.0.iter().all(|&word| word == 0)
    }

    pub(crate) fn clear(&mut self) {
        self.0 = [0; 12];
    }
}

/// What to do with one key event of the grabbed stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Verdict {
    /// Forward it.
    Pass,
    /// Keep it: the final key of the shortcut, down with exactly its modifiers. It is
    /// forwarded late if the hold is given up, never if the rescue fires.
    Hold,
    /// The hold was given up: forward the press of this code that was kept, then the
    /// event itself.
    Release(u16),
}

/// The rescue shortcut, watched for in the grabbed key stream.
#[derive(Debug, Default)]
pub(crate) struct RescueKeys {
    /// The modifier bits and the Linux key code of the final key; `None` when the
    /// configured text names a key with no fixed place on a keyboard.
    wanted: Option<(u32, u16)>,
    /// The text `wanted` was parsed from, so a re-read that finds the same text does
    /// not parse it again.
    text: String,
    /// Modifier keys held in the grabbed stream.
    modifiers: HeldSet,
    /// Since when the final key has been held, kept back.
    since: Option<Instant>,
}

impl RescueKeys {
    /// Adopts the shortcut as the layout or the agent spells it.
    pub(crate) fn set(&mut self, text: &str) {
        if text == self.text {
            return;
        }
        self.text = text.to_owned();
        self.wanted = Shortcut::parse(text)
            .and_then(|shortcut| Some((shortcut.modifiers, key_of_virtual_key(shortcut.key)?)));
    }

    /// The grabbed stream starts or stops: nothing is held in it any more.
    pub(crate) fn reset(&mut self) {
        self.modifiers.clear();
        self.since = None;
    }

    fn held_modifiers(&self) -> u32 {
        const CLASSES: [(u32, [u16; 2]); 4] = [
            (MOD_CONTROL, [KEY_LEFTCTRL, KEY_RIGHTCTRL]),
            (MOD_SHIFT, [KEY_LEFTSHIFT, KEY_RIGHTSHIFT]),
            (MOD_ALT, [KEY_LEFTALT, KEY_RIGHTALT]),
            (MOD_WIN, [KEY_LEFTMETA, KEY_RIGHTMETA]),
        ];
        CLASSES
            .iter()
            .filter(|(_, codes)| codes.iter().any(|&c| self.modifiers.contains(c)))
            .fold(0, |bits, (bit, _)| bits | bit)
    }

    /// Reads one key event of the grabbed stream.
    pub(crate) fn filter(&mut self, code: u16, value: i32, now: Instant) -> Verdict {
        if is_modifier(code) {
            self.modifiers.track(code, value);
        }
        let Some((modifiers, key)) = self.wanted else {
            return Verdict::Pass;
        };
        if self.since.is_some() {
            // Autorepeat, of the key or of a modifier: still the same hold.
            if value == 2 {
                return Verdict::Hold;
            }
            // Let go early, a modifier let go, or another key pressed: the user was
            // typing, not asking for a rescue. What was kept goes first, so the other
            // machine sees the keys in the order they were pressed.
            self.since = None;
            return Verdict::Release(key);
        }
        if code == key && value == 1 && self.held_modifiers() == modifiers {
            self.since = Some(now);
            return Verdict::Hold;
        }
        Verdict::Pass
    }

    /// Whether the kept key has now been held long enough to fire. Fires once.
    pub(crate) fn due(&mut self, now: Instant) -> bool {
        match self.since {
            Some(since) if now.duration_since(since) >= HOLD => {
                self.since = None;
                true
            }
            _ => false,
        }
    }
}

const KEY_LEFTCTRL: u16 = 29;
const KEY_RIGHTCTRL: u16 = 97;
const KEY_LEFTSHIFT: u16 = 42;
const KEY_RIGHTSHIFT: u16 = 54;
const KEY_LEFTALT: u16 = 56;
const KEY_RIGHTALT: u16 = 100;
const KEY_LEFTMETA: u16 = 125;
const KEY_RIGHTMETA: u16 = 126;

fn is_modifier(code: u16) -> bool {
    matches!(
        code,
        KEY_LEFTCTRL
            | KEY_RIGHTCTRL
            | KEY_LEFTSHIFT
            | KEY_RIGHTSHIFT
            | KEY_LEFTALT
            | KEY_RIGHTALT
            | KEY_LEFTMETA
            | KEY_RIGHTMETA
    )
}

/// The Linux code of the key a Win32 virtual-key code names, over the closed set the
/// shortcut grammar accepts (`lbm_ipc::shortcut`).
///
/// Letters and punctuation are taken at their place on a US keyboard, which is what a
/// Linux code names. A virtual key is a character of the active layout instead, so on
/// another layout the two can be different keys: `M` here is the key right of `N`,
/// wherever the layout prints `M`. The keyboard of a master is read below any layout,
/// and there is no layout to ask; the portal, which binds by character, may pick
/// another physical key for the same text.
pub(crate) fn key_of_virtual_key(vk: u32) -> Option<u16> {
    const LETTERS: [u16; 26] = [
        30, 48, 46, 32, 18, 33, 34, 35, 23, 36, 37, 38, 50, 49, 24, 25, 16, 19, 31, 20, 22, 47, 17,
        45, 21, 44,
    ];
    const NUMPAD: [u16; 10] = [82, 79, 80, 81, 75, 76, 77, 71, 72, 73];
    Some(match vk {
        0x41..=0x5A => LETTERS[(vk - 0x41) as usize],
        0x30 => 11,
        0x31..=0x39 => (vk - 0x31 + 2) as u16,
        0x70..=0x79 => (vk - 0x70 + 59) as u16,
        0x7A => 87,
        0x7B => 88,
        0x7C..=0x87 => (vk - 0x7C + 183) as u16,
        0x60..=0x69 => NUMPAD[(vk - 0x60) as usize],
        0x6A => 55,
        0x6B => 78,
        0x6D => 74,
        0x6E => 83,
        0x6F => 98,
        0xBA => 39,
        0xBB => 13,
        0xBC => 51,
        0xBD => 12,
        0xBE => 52,
        0xBF => 53,
        0xC0 => 41,
        0xDB => 26,
        0xDC => 43,
        0xDD => 27,
        0xDE => 40,
        0xE2 => 86,
        0x20 => 57,
        0x1B => 1,
        0x0D => 28,
        0x09 => 15,
        0x08 => 14,
        0x2D => 110,
        0x2E => 111,
        0x24 => 102,
        0x23 => 107,
        0x21 => 104,
        0x22 => 109,
        0x25 => 105,
        0x26 => 103,
        0x27 => 106,
        0x28 => 108,
        0x13 => 119,
        0x2C => 99,
        // VK_OEM_8 has no place of its own on a US keyboard.
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY_M: u16 = 50;
    const KEY_A: u16 = 30;

    fn rescue() -> RescueKeys {
        let mut keys = RescueKeys::default();
        keys.set(lbm_ipc::shortcut::DEFAULT);
        keys
    }

    #[test]
    fn a_held_set_tracks_presses_and_releases_and_ignores_autorepeat() {
        let mut held = HeldSet::default();
        held.track(0x110, 1);
        held.track(KEY_A, 1);
        held.track(KEY_A, 2);
        assert!(held.contains(0x110) && held.contains(KEY_A));
        held.track(KEY_A, 0);
        assert!(!held.contains(KEY_A));
        assert!(held.remove(0x110));
        assert!(held.is_empty());
        held.insert(u16::MAX);
        assert!(held.is_empty(), "a code past KEY_MAX is never held");
    }

    #[test]
    fn the_default_shortcut_is_ctrl_alt_shift_and_the_key_right_of_n() {
        assert_eq!(
            rescue().wanted,
            Some((MOD_CONTROL | MOD_ALT | MOD_SHIFT, KEY_M))
        );
        assert_eq!(key_of_virtual_key(0x70), Some(59), "F1");
        assert_eq!(key_of_virtual_key(0x7B), Some(88), "F12");
        assert_eq!(key_of_virtual_key(0x87), Some(194), "F24");
        assert_eq!(key_of_virtual_key(0x30), Some(11), "0");
        assert_eq!(key_of_virtual_key(0x31), Some(2), "1");
        assert_eq!(key_of_virtual_key(0x65), Some(76), "numpad 5");
    }

    fn press_modifiers(keys: &mut RescueKeys, now: Instant) {
        for code in [KEY_LEFTCTRL, KEY_LEFTALT, KEY_LEFTSHIFT] {
            assert_eq!(keys.filter(code, 1, now), Verdict::Pass);
        }
    }

    #[test]
    fn the_final_key_held_with_exactly_its_modifiers_is_kept_and_fires_after_the_hold() {
        let mut keys = rescue();
        let start = Instant::now();
        press_modifiers(&mut keys, start);
        assert_eq!(keys.filter(KEY_M, 1, start), Verdict::Hold);
        assert_eq!(keys.filter(KEY_M, 2, start + HOLD / 2), Verdict::Hold);
        assert!(!keys.due(start + HOLD / 2));
        assert!(keys.due(start + HOLD));
        assert!(!keys.due(start + HOLD * 2), "fires once");
    }

    #[test]
    fn letting_go_early_hands_the_kept_press_back_before_the_release() {
        let mut keys = rescue();
        let start = Instant::now();
        press_modifiers(&mut keys, start);
        assert_eq!(keys.filter(KEY_M, 1, start), Verdict::Hold);
        assert_eq!(keys.filter(KEY_M, 0, start), Verdict::Release(KEY_M));
        assert!(!keys.due(start + HOLD));
    }

    #[test]
    fn letting_a_modifier_go_or_pressing_another_key_gives_the_hold_up() {
        let start = Instant::now();
        let mut keys = rescue();
        press_modifiers(&mut keys, start);
        assert_eq!(keys.filter(KEY_M, 1, start), Verdict::Hold);
        assert_eq!(
            keys.filter(KEY_LEFTSHIFT, 0, start),
            Verdict::Release(KEY_M)
        );
        assert!(!keys.due(start + HOLD));

        let mut keys = rescue();
        press_modifiers(&mut keys, start);
        assert_eq!(keys.filter(KEY_M, 1, start), Verdict::Hold);
        assert_eq!(keys.filter(KEY_A, 1, start), Verdict::Release(KEY_M));
    }

    #[test]
    fn the_final_key_with_other_modifiers_is_only_typing() {
        let start = Instant::now();
        let mut keys = rescue();
        keys.filter(KEY_LEFTCTRL, 1, start);
        assert_eq!(keys.filter(KEY_M, 1, start), Verdict::Pass);

        let mut keys = rescue();
        press_modifiers(&mut keys, start);
        keys.filter(KEY_LEFTMETA, 1, start);
        assert_eq!(keys.filter(KEY_M, 1, start), Verdict::Pass);
    }
}
