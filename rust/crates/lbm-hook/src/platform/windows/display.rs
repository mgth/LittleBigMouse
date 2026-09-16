//! What the desktop is made of right now — the monitor rectangles Windows itself
//! reports, in the same physical, per-monitor-DPI-aware coordinates the UI computes
//! zone bounds in (see [`super::init`]).

use windows::Win32::Foundation::{BOOL, LPARAM, RECT};
use windows::Win32::Graphics::Gdi::{EnumDisplayMonitors, HDC, HMONITOR};

use crate::geometry::Rect;

/// The rectangle of every attached monitor, or `None` when Windows reports none —
/// a session with no display to reason about is not one where a layout can be
/// judged, so the caller must treat it as "unknown", never as "everything is stale".
pub fn monitors_now() -> Option<Vec<Rect<i32>>> {
    let mut monitors: Vec<Rect<i32>> = Vec::new();

    unsafe extern "system" fn collect(
        _monitor: HMONITOR,
        _hdc: HDC,
        rect: *mut RECT,
        data: LPARAM,
    ) -> BOOL {
        // SAFETY: `data` is the `&mut Vec` passed below, alive for the whole enumeration,
        // and `rect` is the monitor rectangle Windows hands to every callback.
        let monitors = unsafe { &mut *(data.0 as *mut Vec<Rect<i32>>) };
        let r = unsafe { *rect };
        monitors.push(Rect::new(r.left, r.top, r.right - r.left, r.bottom - r.top));
        BOOL(1)
    }

    let ok = unsafe {
        EnumDisplayMonitors(
            HDC::default(),
            None,
            Some(collect),
            LPARAM(&mut monitors as *mut Vec<Rect<i32>> as isize),
        )
    };

    if !ok.as_bool() || monitors.is_empty() {
        return None;
    }
    Some(monitors)
}
