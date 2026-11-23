//! The two devices this process creates: the virtual pointer every routed motion
//! comes out of, and the virtual keyboard the keyboard usages of grabbed mice
//! are re-emitted on.
//!
//! The same two builders also make the pair a multi-PC remote injects through
//! (`crate::remote`), under names of their own.
//!
//! They are separate on purpose. Mixing a full keyboard into an ABS pointer
//! risks a libinput/KWin reclassification of the pointer, and the pointer cannot
//! carry the KEY_* codes anyway: the kernel silently drops events whose (type,
//! code) a uinput device did not declare.

use evdev::{
    uinput::VirtualDevice, AbsInfo, AbsoluteAxisCode, AttributeSet, BusType, InputId, KeyCode,
    RelativeAxisCode, UinputAbsSetup,
};

use crate::geometry::Rect;

use super::{BTN_RANGE, BTN_TRIGGER_HAPPY_RANGE};

/// Every device this process creates is named with this prefix, and the device
/// scan never takes one of them for a physical mouse or keyboard (see [`is_ours`]).
const OURS: &str = "LittleBigMouse virtual";

const VIRTUAL_NAME: &str = "LittleBigMouse virtual pointer";
const VIRTUAL_KBD_NAME: &str = "LittleBigMouse virtual keyboard";
pub(crate) const REMOTE_POINTER_NAME: &str = "LittleBigMouse virtual pointer (remote input)";
pub(crate) const REMOTE_KEYBOARD_NAME: &str = "LittleBigMouse virtual keyboard (remote input)";

/// Whether a device of that name is one this process created. The evdev backend
/// must never grab its own pointer — it would route its own output — nor read the
/// remote keyboard for the ctrl-override: that is the master's ctrl, not the ctrl
/// of this machine's user.
pub(super) fn is_ours(name: &str) -> bool {
    name.starts_with(OURS)
}

/// The routing pointer of the evdev backend; see [`build_pointer`].
pub(super) fn build_virtual(desktop: Rect<i32>) -> std::io::Result<VirtualDevice> {
    build_pointer(VIRTUAL_NAME, 0x4d55, desktop)
}

/// An absolute virtual pointer whose ABS range equals the desktop size, plus
/// buttons and (relative) wheels. KWin maps the ABS range 1:1 onto the whole
/// desktop, so no acceleration is applied and the emitted point is the position.
pub(crate) fn build_pointer(
    name: &str,
    product: u16,
    desktop: Rect<i32>,
) -> std::io::Result<VirtualDevice> {
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

    VirtualDevice::builder()?
        .name(name)
        .input_id(InputId::new(BusType::BUS_VIRTUAL, 0x4c42, product, 1))
        .with_keys(&keys)?
        .with_relative_axes(&wheels)?
        .with_absolute_axis(&ax)?
        .with_absolute_axis(&ay)?
        .build()
}

/// The keyboard usages of grabbed mice; see [`build_keyboard`].
pub(super) fn build_virtual_keyboard() -> std::io::Result<VirtualDevice> {
    build_keyboard(VIRTUAL_KBD_NAME, 0x4d56)
}

/// A full-range virtual keyboard. Declaring (almost) every KEY_* code up front means the device never has to
/// be rebuilt to match a given mouse's capabilities. EV_REP is deliberately
/// absent: key repeat belongs to the compositor/xkb, as with a real keyboard.
pub(crate) fn build_keyboard(name: &str, product: u16) -> std::io::Result<VirtualDevice> {
    let mut keys = AttributeSet::<KeyCode>::new();
    // 0x2ff = KEY_MAX; skip the mouse/joystick button blocks routed to the pointer.
    for code in 1..=0x2ffu16 {
        if BTN_RANGE.contains(&code) || BTN_TRIGGER_HAPPY_RANGE.contains(&code) {
            continue;
        }
        keys.insert(KeyCode::new(code));
    }

    VirtualDevice::builder()?
        .name(name)
        .input_id(InputId::new(BusType::BUS_VIRTUAL, 0x4c42, product, 1))
        .with_keys(&keys)?
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_device_this_process_creates_is_recognised_as_its_own() {
        for name in [
            VIRTUAL_NAME,
            VIRTUAL_KBD_NAME,
            REMOTE_POINTER_NAME,
            REMOTE_KEYBOARD_NAME,
        ] {
            assert!(is_ours(name), "{name}");
        }
        assert!(!is_ours("Logitech USB Receiver"));
    }
}
