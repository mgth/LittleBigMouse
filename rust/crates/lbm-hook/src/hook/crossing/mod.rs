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

use std::time::Instant;

use lbm_ipc::input::InputEvent as Wire;

use crate::geometry::Point;
use crate::zones::{HostPoint, ZonesLayout};

use keys::{HeldSet, RescueKeys, Verdict};

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

/// Where a routed position is, as far as the next frame is concerned.
#[derive(Debug, PartialEq)]
pub(crate) enum Place {
    Local,
    /// On another machine, the pointer coming from here.
    Enter {
        pixel: Point<i32>,
        host: String,
    },
    /// On the machine being driven.
    Remote(Point<i32>),
    /// Neither here nor on the machine being driven.
    Away,
}

/// Where `pos`, a position the engine routed to, leaves the pointer. `inside_here`:
/// `pos` lies on this machine's own desktop. `driving`: the machine the pointer is
/// on, while it is on another one.
///
/// Only a position outside this desktop is looked up: the other machines' desktops lie
/// outside it, so a position inside is this machine's without asking.
pub(crate) fn locate(
    layout: &ZonesLayout,
    pos: Point<i32>,
    inside_here: bool,
    driving: Option<&str>,
) -> Place {
    if driving.is_none() && inside_here {
        return Place::Local;
    }
    match layout.host_point(pos) {
        Some(HostPoint { host: None, .. }) => Place::Local,
        Some(HostPoint {
            host: Some(there),
            pixel,
        }) => match driving {
            Some(host) if there == host => Place::Remote(pixel),
            Some(_) => Place::Away,
            None => Place::Enter {
                pixel,
                host: there.to_owned(),
            },
        },
        None if driving.is_some() => Place::Away,
        None => Place::Local,
    }
}

/// A mouse button while the pointer is away: `true` when it went into the batch.
pub(crate) fn route_button(
    wire: &mut Vec<Wire>,
    held: &mut HeldSet,
    code: u16,
    press: Press,
) -> bool {
    match press {
        Press::Down => {
            held.insert(code);
            wire.push(Wire::Button {
                code,
                pressed: true,
            });
            true
        }
        // Released where it was pressed.
        Press::Up if held.remove(code) => {
            wire.push(Wire::Button {
                code,
                pressed: false,
            });
            true
        }
        Press::Up => false,
        // A repeat goes where its press went; a button's means nothing anywhere.
        Press::Repeat => held.contains(code),
    }
}

/// A key of the grabbed stream while the pointer is away: `true` when it was taken —
/// into the batch, or kept back as the rescue's final key.
pub(crate) fn route_key(
    wire: &mut Vec<Wire>,
    held: &mut HeldSet,
    rescue: &mut RescueKeys,
    code: u16,
    press: Press,
    now: Instant,
) -> bool {
    match rescue.filter(code, press, now) {
        Verdict::Hold => return true,
        Verdict::Release(kept) => {
            held.insert(kept);
            wire.push(Wire::Key {
                code: kept,
                pressed: true,
            });
        }
        Verdict::Pass => {}
    }
    match press {
        Press::Down => {
            held.insert(code);
            wire.push(Wire::Key {
                code,
                pressed: true,
            });
            true
        }
        Press::Up if held.remove(code) => {
            wire.push(Wire::Key {
                code,
                pressed: false,
            });
            true
        }
        Press::Up => false,
        // Autorepeat: the remote repeats an injected key only when told to.
        Press::Repeat if held.contains(code) => {
            wire.push(Wire::Key {
                code,
                pressed: true,
            });
            true
        }
        Press::Repeat => false,
    }
}
