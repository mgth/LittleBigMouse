//! What the desktop is made of right now — the Linux counterpart of
//! `platform::windows::display`.

use crate::geometry::Rect;

/// Not known on Linux yet: the backends (X11, portal, evdev) each see the outputs
/// differently, and none of them is asked here. `None` means "unknown", which leaves
/// the daemon hooking whatever it is given, exactly as before — never "no monitor".
pub fn monitors_now() -> Option<Vec<Rect<i32>>> {
    None
}
