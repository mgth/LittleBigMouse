//! What the desktop is made of right now — the macOS counterpart of
//! `platform::windows::display`.

use super::cg;
use crate::geometry::Rect;

/// The bounds of every active display in global points (see the module doc of
/// [`super`]), or `None` when CoreGraphics reports none — "unknown", never "no
/// monitor", exactly like the Windows side.
pub fn monitors_now() -> Option<Vec<Rect<i32>>> {
    let monitors: Vec<Rect<i32>> = cg::active_display_bounds()
        .into_iter()
        .map(to_rect)
        .collect();
    if monitors.is_empty() {
        None
    } else {
        Some(monitors)
    }
}

/// Display bounds are whole points in practice; round rather than truncate so a
/// value that comes back as 1919.9999 still lands on 1920.
pub(crate) fn to_rect(r: cg::CGRect) -> Rect<i32> {
    Rect::new(
        r.origin.x.round() as i32,
        r.origin.y.round() as i32,
        r.size.width.round() as i32,
        r.size.height.round() as i32,
    )
}

/// The bounding box of all displays — what Win32 calls the virtual screen, and
/// what an unclipped cursor reports as its clip.
pub fn virtual_screen() -> Rect<i32> {
    let monitors = monitors_now().unwrap_or_default();
    let Some(first) = monitors.first() else {
        return Rect::empty();
    };
    let (mut left, mut top, mut right, mut bottom) =
        (first.left(), first.top(), first.right(), first.bottom());
    for m in &monitors[1..] {
        left = left.min(m.left());
        top = top.min(m.top());
        right = right.max(m.right());
        bottom = bottom.max(m.bottom());
    }
    Rect::new(left, top, right - left, bottom - top)
}
