//! The panic shortcut, as it travels on the wire.
//!
//! A global shortcut is written the way a user reads it — `Ctrl+Alt+Shift+M` — and
//! parsed here into the modifier bits and virtual-key code `RegisterHotKey` wants.
//! Keeping the grammar in one place means the UI's recorder and the daemon's
//! registrar cannot drift apart: whatever the UI writes, this is what decides.

/// Modifier bits. Same values as the Win32 `MOD_*` constants, so the Windows
/// registrar passes them straight through; duplicating the mapping there would only
/// create somewhere for the two to disagree.
pub const MOD_ALT: u32 = 0x0001;
pub const MOD_CONTROL: u32 = 0x0002;
pub const MOD_SHIFT: u32 = 0x0004;
pub const MOD_WIN: u32 = 0x0008;

/// What the daemon registers when the layout names no shortcut of its own. Three
/// modifiers: unreachable by accident, and unlikely to be owned by anything else.
pub const DEFAULT: &str = "Ctrl+Alt+Shift+M";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Shortcut {
    pub modifiers: u32,
    /// Win32 virtual-key code.
    pub key: u32,
}

impl Shortcut {
    /// Parse `Ctrl+Alt+Shift+M`. Case-insensitive, spaces ignored.
    ///
    /// Returns `None` for anything that would be unsafe or useless to register: no
    /// key, an unknown key name, or no modifier at all. A bare key registered
    /// globally would swallow that key for every application on the desktop, which
    /// is not a thing a mouse utility gets to do by accident.
    pub fn parse(text: &str) -> Option<Shortcut> {
        let mut modifiers = 0;
        let mut key = None;

        for part in text.split('+') {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }
            match part.to_ascii_lowercase().as_str() {
                "ctrl" | "control" => modifiers |= MOD_CONTROL,
                "alt" => modifiers |= MOD_ALT,
                "shift" | "maj" => modifiers |= MOD_SHIFT,
                "win" | "super" | "meta" | "cmd" => modifiers |= MOD_WIN,
                // A second key name is a malformed shortcut, not a replacement.
                _ if key.is_some() => return None,
                _ => key = Some(virtual_key(part)?),
            }
        }

        match (modifiers, key) {
            (0, _) => None,
            (_, Some(key)) => Some(Shortcut { modifiers, key }),
            (_, None) => None,
        }
    }
}

/// Name to Win32 virtual-key code, over the set a shortcut may sensibly use.
/// Deliberately closed: an open mapping would let the UI record a key the daemon
/// cannot register, and the failure would surface far from its cause.
fn virtual_key(name: &str) -> Option<u32> {
    let lower = name.to_ascii_lowercase();

    if lower.len() == 1 {
        let c = lower.as_bytes()[0];
        return match c {
            b'a'..=b'z' => Some(0x41 + (c - b'a') as u32),
            b'0'..=b'9' => Some(0x30 + (c - b'0') as u32),
            _ => None,
        };
    }

    if let Some(digits) = lower.strip_prefix('f') {
        if let Ok(n) = digits.parse::<u32>() {
            if (1..=24).contains(&n) {
                return Some(0x70 + n - 1);
            }
        }
        // Fall through: "f" prefixes nothing else in this table.
    }

    if let Some(digit) = lower.strip_prefix("numpad") {
        if let Ok(n) = digit.parse::<u32>() {
            if n <= 9 {
                return Some(0x60 + n);
            }
        }
    }

    if let Some(index) = lower.strip_prefix("oem") {
        // OEM keys are positions, not characters: VK_OEM_3 is ` on a QWERTY board and
        // ² on an AZERTY one. Registering the position is what makes a shortcut mean
        // the same physical key whatever the layout says it prints.
        if let Ok(n) = index.parse::<u32>() {
            return match n {
                1 => Some(0xBA),
                2 => Some(0xBF),
                3 => Some(0xC0),
                4 => Some(0xDB),
                5 => Some(0xDC),
                6 => Some(0xDD),
                7 => Some(0xDE),
                8 => Some(0xDF),
                102 => Some(0xE2),
                _ => None,
            };
        }
    }

    Some(match lower.as_str() {
        "oemplus" => 0xBB,
        "oemcomma" => 0xBC,
        "oemminus" => 0xBD,
        "oemperiod" => 0xBE,
        "numpadmultiply" => 0x6A,
        "numpadadd" => 0x6B,
        "numpadsubtract" => 0x6D,
        "numpaddecimal" => 0x6E,
        "numpaddivide" => 0x6F,
        "space" => 0x20,
        "escape" | "esc" => 0x1B,
        "enter" | "return" => 0x0D,
        "tab" => 0x09,
        "backspace" | "back" => 0x08,
        "insert" | "ins" => 0x2D,
        "delete" | "del" => 0x2E,
        "home" => 0x24,
        "end" => 0x23,
        "pageup" | "prior" => 0x21,
        "pagedown" | "next" => 0x22,
        "left" => 0x25,
        "up" => 0x26,
        "right" => 0x27,
        "down" => 0x28,
        "pause" => 0x13,
        "printscreen" | "print" => 0x2C,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_default() {
        let s = Shortcut::parse(DEFAULT).expect("the default must always parse");
        assert_eq!(s.modifiers, MOD_CONTROL | MOD_ALT | MOD_SHIFT);
        assert_eq!(s.key, 0x4D); // 'M'
    }

    #[test]
    fn is_case_and_space_insensitive() {
        let spelled_out = Shortcut::parse("control + ALT + maj + m").expect("parse");
        assert_eq!(spelled_out, Shortcut::parse(DEFAULT).unwrap());
    }

    #[test]
    fn reads_the_key_families_it_offers() {
        assert_eq!(Shortcut::parse("Ctrl+F12").unwrap().key, 0x7B);
        assert_eq!(Shortcut::parse("Ctrl+F1").unwrap().key, 0x70);
        assert_eq!(Shortcut::parse("Ctrl+7").unwrap().key, 0x37);
        assert_eq!(Shortcut::parse("Win+Space").unwrap().key, 0x20);
        assert_eq!(Shortcut::parse("Alt+PageDown").unwrap().key, 0x22);
    }

    #[test]
    fn reads_punctuation_and_the_numeric_keypad() {
        // The half a French keyboard is made of. OEM keys name a position, not a
        // character: Oem3 is ` on QWERTY and ² on AZERTY, and both register the same
        // physical key. Every name here has to match ShortcutBox.KeyName exactly — that
        // agreement is the contract, and nothing but these tests enforces it.
        assert_eq!(Shortcut::parse("Ctrl+Oem1").unwrap().key, 0xBA);
        assert_eq!(Shortcut::parse("Ctrl+Oem3").unwrap().key, 0xC0);
        assert_eq!(Shortcut::parse("Ctrl+Oem7").unwrap().key, 0xDE);
        assert_eq!(Shortcut::parse("Ctrl+Oem8").unwrap().key, 0xDF);
        assert_eq!(Shortcut::parse("Ctrl+Oem102").unwrap().key, 0xE2);
        assert_eq!(Shortcut::parse("Ctrl+OemPlus").unwrap().key, 0xBB);
        assert_eq!(Shortcut::parse("Ctrl+OemComma").unwrap().key, 0xBC);
        assert_eq!(Shortcut::parse("Ctrl+OemMinus").unwrap().key, 0xBD);
        assert_eq!(Shortcut::parse("Ctrl+OemPeriod").unwrap().key, 0xBE);

        assert_eq!(Shortcut::parse("Ctrl+NumPad0").unwrap().key, 0x60);
        assert_eq!(Shortcut::parse("Ctrl+NumPad9").unwrap().key, 0x69);
        assert_eq!(Shortcut::parse("Ctrl+NumPadMultiply").unwrap().key, 0x6A);
        assert_eq!(Shortcut::parse("Ctrl+NumPadAdd").unwrap().key, 0x6B);
        assert_eq!(Shortcut::parse("Ctrl+NumPadSubtract").unwrap().key, 0x6D);
        assert_eq!(Shortcut::parse("Ctrl+NumPadDecimal").unwrap().key, 0x6E);
        assert_eq!(Shortcut::parse("Ctrl+NumPadDivide").unwrap().key, 0x6F);

        assert!(Shortcut::parse("Ctrl+Oem9").is_none());
        assert!(Shortcut::parse("Ctrl+NumPad10").is_none());
    }

    #[test]
    fn refuses_a_shortcut_with_no_modifier() {
        // Registered globally, this would take the key away from every application
        // on the desktop.
        assert!(Shortcut::parse("M").is_none());
        assert!(Shortcut::parse("F12").is_none());
    }

    #[test]
    fn refuses_what_it_cannot_register() {
        assert!(Shortcut::parse("").is_none());
        assert!(Shortcut::parse("Ctrl+Alt").is_none(), "modifiers are not a key");
        assert!(Shortcut::parse("Ctrl+NoSuchKey").is_none());
        assert!(Shortcut::parse("Ctrl+F0").is_none());
        assert!(Shortcut::parse("Ctrl+F25").is_none());
        assert!(Shortcut::parse("Ctrl+A+B").is_none(), "two keys is malformed");
    }
}
