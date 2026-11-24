//! The multi-PC master's crossing rules, with no platform in sight (docs/multi-pc.md).
//!
//! A master drives another machine through the input stream while the pointer is
//! there. What it does then — which side a release goes to, when the keyboards are
//! grabbed, when the pointer comes home, the rescue watched for in the grabbed stream —
//! does not depend on how the platform reads its devices. Those rules live here, so
//! that every master (evdev on Linux today, the Windows hook and the portal later)
//! applies the same ones and is tested against the same ones.

// Only the evdev backend is a master so far: on Windows nothing calls into this yet,
// and it is compiled there all the same, so that it stays platform-neutral.
#![cfg_attr(not(target_os = "linux"), allow(dead_code))]

pub(crate) mod keys;

/// What one event does to a key or a button. Codes are Linux `EV_KEY` codes whatever
/// the platform: they are what the input stream carries (`lbm_ipc::input`), and a
/// master on another platform translates to them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Press {
    Up,
    Down,
    /// Autorepeat of a key held down.
    Repeat,
}

impl Press {
    /// From the value of a Linux `EV_KEY` event: 0 releases, 1 presses, anything
    /// else (2) repeats.
    pub(crate) fn from_ev_key(value: i32) -> Press {
        match value {
            0 => Press::Up,
            1 => Press::Down,
            _ => Press::Repeat,
        }
    }
}
