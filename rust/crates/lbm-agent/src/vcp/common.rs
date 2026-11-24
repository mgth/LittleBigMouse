//! What the platforms' DDC/CI paths share: a reading made into a control the property
//! panel offers, and the labels of the values a monitor does not name.
#![cfg_attr(not(any(target_os = "linux", windows)), allow(dead_code))]

use lbm_ipc::vcp::{self, Choice, Feature};

pub(super) const PRESET: u8 = 0x14;
/// The controls of a colour preset: changing one can switch the monitor to "custom".
pub(super) const RGB: [u8; 6] = [0x16, 0x18, 0x1a, 0x6c, 0x6e, 0x70];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Reading {
    pub value: u16,
    pub maximum: u16,
    /// A plain continuous value, not a structured one: ddcutil's `C`, and always on
    /// Windows, whose API hands back two numbers and nothing of the reply's shape.
    pub plain: bool,
}

/// A control as the panel offers it. `None` when the reading makes no sense for it.
pub(super) fn feature(code: u8, reading: Reading, choices: Vec<Choice>) -> Option<Feature> {
    // Only the older, plain volume is a level from 0 up.
    let structured_volume = code == vcp::VOLUME && !reading.plain;
    let feature = if structured_volume {
        Feature {
            code,
            value: reading.value,
            minimum: vcp::VOLUME_LEVELS.0,
            // MCCS defines the levels: a reply may well leave its maximum at zero.
            maximum: match reading.maximum {
                0 => vcp::VOLUME_LEVELS.1,
                maximum => maximum.min(vcp::VOLUME_LEVELS.1),
            },
            choices: vcp::volume_choices(),
        }
    } else {
        Feature {
            code,
            value: reading.value,
            minimum: 0,
            maximum: reading.maximum,
            choices,
        }
    };
    let usable = !vcp::continuous(code)
        || (feature.maximum > 0
            && feature.maximum >= feature.minimum
            && feature.accepts(feature.value));
    usable.then_some(feature)
}

pub(super) fn choice_label(code: u8, value: u16, reported: &str) -> String {
    match (code, value) {
        (0x8d, 1) => "Muted".into(),
        (0x8d, 2) => "Sound on".into(),
        (0xd6, 1) => "On".into(),
        (0xd6, 2) => "Standby".into(),
        (0xd6, 3) => "Suspend".into(),
        (0xd6, 4 | 5) => format!("Off (0x{value:02X})"),
        _ if reported.is_empty() || reported.starts_with("Unrecognized") => {
            match (code, input_name(value)) {
                (0x60, Some(name)) => name.into(),
                _ => format!("0x{value:02X}"),
            }
        }
        _ => reported.into(),
    }
}

/// The MCCS names of the input values, for a monitor that lists values without names:
/// the capability string Windows reads never names them, where ddcutil does.
fn input_name(value: u16) -> Option<&'static str> {
    Some(match value {
        0x01 => "VGA-1",
        0x02 => "VGA-2",
        0x03 => "DVI-1",
        0x04 => "DVI-2",
        0x0f => "DisplayPort-1",
        0x10 => "DisplayPort-2",
        0x11 => "HDMI-1",
        0x12 => "HDMI-2",
        _ => return None,
    })
}
