//! Linux key codes as the keys Windows' `SendInput` understands.
//!
//! The input stream names physical keys by their Linux code (`lbm_ipc::input`), and
//! the receiver applies its own keyboard layout. On Windows that means injecting
//! *scancodes* (`KEYEVENTF_SCANCODE`), never virtual keys: a virtual key is what a
//! layout already produced, a scancode is the key's place on the keyboard. Scancode
//! set 1, the one Windows speaks, with the `E0` prefix carried by
//! `KEYEVENTF_EXTENDEDKEY`.
//!
//! Linux numbered its first 83 keys after set 1 itself, so that block is the
//! identity; the rest is a table. A pure function, compiled everywhere, so that it is
//! tested on every machine and not only on the one that can run `SendInput`.
//!
//! Left out, and dropped by the Windows injector: the Japanese and Korean input
//! method keys that have no single set-1 scancode Windows maps the same way on every
//! layout (`KEY_ZENKAKUHANKAKU` 85, `KEY_KATAKANA` 90, `KEY_HIRAGANA` 91,
//! `KEY_KPJPCOMMA` 95, `KEY_HANGEUL` 122, `KEY_HANJA` 123), keys no PC keyboard sends
//! (`KEY_LINEFEED` 101, `KEY_MACRO` 112, `KEY_KPPLUSMINUS` 118, `KEY_SCALE` 120), and
//! every code above 127 not in the media/browser/F13–F24 list below.

/// A key as `SendInput` takes it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WindowsKey {
    /// Injected with `KEYEVENTF_SCANCODE`, plus `KEYEVENTF_EXTENDEDKEY` for the keys
    /// whose make code is prefixed with `E0`.
    Scan { code: u16, extended: bool },
    /// Injected by virtual key, the scancode only riding along for whoever reads it.
    /// For the two keys whose scancode is ambiguous: Pause sends `E1 1D 45`, which
    /// `KEYEVENTF_SCANCODE` cannot express, and shares `45` with Num Lock, which Windows
    /// tells apart by the extended flag. Naming both by virtual key leaves nothing to
    /// how a given Windows version reads a bare `45`.
    Virtual { vk: u16, scan: u16, extended: bool },
}

const VK_PAUSE: u16 = 0x13;
const VK_NUMLOCK: u16 = 0x90;

const fn plain(code: u16) -> Option<WindowsKey> {
    Some(WindowsKey::Scan {
        code,
        extended: false,
    })
}

const fn extended(code: u16) -> Option<WindowsKey> {
    Some(WindowsKey::Scan {
        code,
        extended: true,
    })
}

/// The Windows key for a Linux `KEY_*` code, or `None` for one left out (see the
/// module documentation).
pub fn windows_key(linux: u16) -> Option<WindowsKey> {
    match linux {
        // KEY_NUMLOCK.
        69 => Some(WindowsKey::Virtual {
            vk: VK_NUMLOCK,
            scan: 0x45,
            extended: true,
        }),
        // KEY_ESC (1) … KEY_KPDOT (83): Linux kept set 1's numbering.
        1..=83 => plain(linux),
        86 => plain(0x56), // KEY_102ND, the extra key left of Z on ISO keyboards
        87 => plain(0x57), // KEY_F11
        88 => plain(0x58), // KEY_F12
        89 => plain(0x73), // KEY_RO
        92 => plain(0x79), // KEY_HENKAN
        93 => plain(0x70), // KEY_KATAKANAHIRAGANA
        94 => plain(0x7b), // KEY_MUHENKAN
        96 => extended(0x1c), // KEY_KPENTER
        97 => extended(0x1d), // KEY_RIGHTCTRL
        98 => extended(0x35), // KEY_KPSLASH
        99 => extended(0x37), // KEY_SYSRQ, the Print Screen key
        100 => extended(0x38), // KEY_RIGHTALT
        102 => extended(0x47), // KEY_HOME
        103 => extended(0x48), // KEY_UP
        104 => extended(0x49), // KEY_PAGEUP
        105 => extended(0x4b), // KEY_LEFT
        106 => extended(0x4d), // KEY_RIGHT
        107 => extended(0x4f), // KEY_END
        108 => extended(0x50), // KEY_DOWN
        109 => extended(0x51), // KEY_PAGEDOWN
        110 => extended(0x52), // KEY_INSERT
        111 => extended(0x53), // KEY_DELETE
        113 => extended(0x20), // KEY_MUTE
        114 => extended(0x2e), // KEY_VOLUMEDOWN
        115 => extended(0x30), // KEY_VOLUMEUP
        116 => extended(0x5e), // KEY_POWER
        117 => plain(0x59), // KEY_KPEQUAL
        119 => Some(WindowsKey::Virtual {
            vk: VK_PAUSE,
            scan: 0x45,
            extended: false,
        }), // KEY_PAUSE
        121 => plain(0x7e), // KEY_KPCOMMA
        124 => plain(0x7d), // KEY_YEN
        125 => extended(0x5b), // KEY_LEFTMETA, the left Windows key
        126 => extended(0x5c), // KEY_RIGHTMETA
        127 => extended(0x5d), // KEY_COMPOSE, the menu key
        128 => extended(0x68), // KEY_STOP (browser)
        140 => extended(0x21), // KEY_CALC
        142 => extended(0x5f), // KEY_SLEEP
        143 => extended(0x63), // KEY_WAKEUP
        155 => extended(0x6c), // KEY_MAIL
        156 => extended(0x66), // KEY_BOOKMARKS
        157 => extended(0x6b), // KEY_COMPUTER
        158 => extended(0x6a), // KEY_BACK (browser)
        159 => extended(0x69), // KEY_FORWARD (browser)
        163 => extended(0x19), // KEY_NEXTSONG
        164 => extended(0x22), // KEY_PLAYPAUSE
        165 => extended(0x10), // KEY_PREVIOUSSONG
        166 => extended(0x24), // KEY_STOPCD
        172 => extended(0x32), // KEY_HOMEPAGE
        173 => extended(0x67), // KEY_REFRESH
        // KEY_F13 (183) … KEY_F23 (193) are 0x64 … 0x6e; KEY_F24 breaks the run.
        183..=193 => plain(0x64 + (linux - 183)),
        194 => plain(0x76),    // KEY_F24
        217 => extended(0x65), // KEY_SEARCH
        226 => extended(0x6d), // KEY_MEDIA
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn the_main_block_keeps_its_linux_numbering() {
        // KEY_ESC, KEY_A, KEY_LEFTSHIFT, KEY_SPACE, KEY_F1, KEY_KP0, KEY_KPDOT.
        for code in [1, 30, 42, 57, 59, 82, 83] {
            assert_eq!(
                windows_key(code),
                Some(WindowsKey::Scan {
                    code,
                    extended: false
                })
            );
        }
    }

    #[test]
    fn navigation_keys_and_right_modifiers_are_extended() {
        for (linux, scan) in [
            (103, 0x48), // up
            (105, 0x4b), // left
            (106, 0x4d), // right
            (108, 0x50), // down
            (97, 0x1d),  // right ctrl
            (100, 0x38), // right alt
            (125, 0x5b), // left Windows key
            (126, 0x5c), // right Windows key
            (110, 0x52), // insert
            (111, 0x53), // delete
            (102, 0x47), // home
            (107, 0x4f), // end
            (104, 0x49), // page up
            (109, 0x51), // page down
            (99, 0x37),  // print screen
        ] {
            assert_eq!(
                windows_key(linux),
                Some(WindowsKey::Scan {
                    code: scan,
                    extended: true
                }),
                "Linux key {linux}"
            );
        }
    }

    #[test]
    fn the_keypad_enter_and_slash_are_told_apart_from_their_main_block_twins() {
        // KEY_ENTER / KEY_KPENTER and KEY_SLASH / KEY_KPSLASH share a scancode.
        assert_eq!(windows_key(28), plain(0x1c));
        assert_eq!(windows_key(96), extended(0x1c));
        assert_eq!(windows_key(53), plain(0x35));
        assert_eq!(windows_key(98), extended(0x35));
        // And the keypad's own copies of the navigation keys are not extended:
        // KEY_KP8 is the keypad up arrow with Num Lock off.
        assert_eq!(windows_key(72), plain(0x48));
    }

    #[test]
    fn pause_and_num_lock_go_by_virtual_key() {
        assert_eq!(
            windows_key(119),
            Some(WindowsKey::Virtual {
                vk: VK_PAUSE,
                scan: 0x45,
                extended: false
            })
        );
        assert_eq!(
            windows_key(69),
            Some(WindowsKey::Virtual {
                vk: VK_NUMLOCK,
                scan: 0x45,
                extended: true
            })
        );
    }

    #[test]
    fn function_keys_up_to_f24_are_mapped() {
        assert_eq!(windows_key(87), plain(0x57)); // F11
        assert_eq!(windows_key(88), plain(0x58)); // F12
        assert_eq!(windows_key(183), plain(0x64)); // F13
        assert_eq!(windows_key(193), plain(0x6e)); // F23
        assert_eq!(windows_key(194), plain(0x76)); // F24
    }

    #[test]
    fn keys_left_out_and_codes_that_are_not_keys_map_to_nothing() {
        for code in [
            0, 84, 85, 90, 91, 95, 101, 112, 118, 120, 122, 123, 0x110, 0x2ff,
        ] {
            assert_eq!(windows_key(code), None, "Linux code {code}");
        }
    }

    #[test]
    fn no_two_linux_keys_become_the_same_windows_key() {
        let mut seen = HashMap::new();
        for linux in 0..=0x2ffu16 {
            if let Some(key) = windows_key(linux) {
                if let Some(first) = seen.insert(key, linux) {
                    panic!("Linux keys {first} and {linux} both give {key:?}");
                }
            }
        }
    }
}
