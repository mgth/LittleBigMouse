//! The two devices this process creates: the virtual pointer every routed motion
//! comes out of, and the virtual keyboard the keyboard usages of grabbed mice
//! are re-emitted on.
//!
//! They are separate on purpose. Mixing a full keyboard into an ABS pointer
//! risks a libinput/KWin reclassification of the pointer, and the pointer cannot
//! carry the KEY_* codes anyway: the kernel silently drops events whose (type,
//! code) a uinput device did not declare.

use evdev::{
    uinput::{VirtualDevice, VirtualDeviceBuilder},
    AbsInfo, AbsoluteAxisCode, AttributeSet, BusType, InputId, KeyCode, RelativeAxisCode,
    UinputAbsSetup,
};

use crate::geometry::Rect;

use super::{BTN_RANGE, BTN_TRIGGER_HAPPY_RANGE};

pub(super) const VIRTUAL_NAME: &str = "LittleBigMouse virtual pointer";
const VIRTUAL_KBD_NAME: &str = "LittleBigMouse virtual keyboard";

/// An absolute virtual pointer whose ABS range equals the desktop size, plus
/// buttons and (relative) wheels. KWin maps the ABS range 1:1 onto the whole
/// desktop, so no acceleration is applied and the emitted point is the position.
pub(super) fn build_virtual(desktop: Rect<i32>) -> std::io::Result<VirtualDevice> {
    let mut keys = AttributeSet::<KeyCode>::new();
    for k in [
        KeyCode::BTN_LEFT,
        KeyCode::BTN_RIGHT,
        KeyCode::BTN_MIDDLE,
        KeyCode::BTN_SIDE,
        KeyCode::BTN_EXTRA,
        KeyCode::BTN_FORWARD,
        KeyCode::BTN_BACK,
        KeyCode::BTN_TASK,
    ] {
        keys.insert(k);
    }

    let mut wheels = AttributeSet::<RelativeAxisCode>::new();
    for a in [
        RelativeAxisCode::REL_WHEEL,
        RelativeAxisCode::REL_HWHEEL,
        RelativeAxisCode::REL_WHEEL_HI_RES,
        RelativeAxisCode::REL_HWHEEL_HI_RES,
    ] {
        wheels.insert(a);
    }

    let w = (desktop.width().max(1)) - 1;
    let h = (desktop.height().max(1)) - 1;
    let ax = UinputAbsSetup::new(AbsoluteAxisCode::ABS_X, AbsInfo::new(0, 0, w, 0, 0, 0));
    let ay = UinputAbsSetup::new(AbsoluteAxisCode::ABS_Y, AbsInfo::new(0, 0, h, 0, 0, 0));

    VirtualDeviceBuilder::new()?
        .name(VIRTUAL_NAME)
        .input_id(InputId::new(BusType::BUS_VIRTUAL, 0x4c42, 0x4d55, 1))
        .with_keys(&keys)?
        .with_relative_axes(&wheels)?
        .with_absolute_axis(&ax)?
        .with_absolute_axis(&ay)?
        .build()
}

/// A full-range virtual keyboard for the keyboard usages of grabbed mice.
/// Declaring (almost) every KEY_* code up front means the device never has to
/// be rebuilt to match a given mouse's capabilities. EV_REP is deliberately
/// absent: key repeat belongs to the compositor/xkb, as with a real keyboard.
pub(super) fn build_virtual_keyboard() -> std::io::Result<VirtualDevice> {
    let mut keys = AttributeSet::<KeyCode>::new();
    // 0x2ff = KEY_MAX; skip the mouse/joystick button blocks routed to the pointer.
    for code in 1..=0x2ffu16 {
        if BTN_RANGE.contains(&code) || BTN_TRIGGER_HAPPY_RANGE.contains(&code) {
            continue;
        }
        keys.insert(KeyCode::new(code));
    }

    VirtualDeviceBuilder::new()?
        .name(VIRTUAL_KBD_NAME)
        .input_id(InputId::new(BusType::BUS_VIRTUAL, 0x4c42, 0x4d56, 1))
        .with_keys(&keys)?
        .build()
}
