//! Low-level mouse hook callback — port of `Hooker::MouseCallback`.
//!
//! The one genuinely hot, genuinely `unsafe` function. It dedups by previous
//! location (C++ `static previousLocation`), then hands the position to the
//! engine under a **non-blocking** `try_lock`: if the lock isn't free (a `Load`
//! is swapping the layout), the event passes straight through — never blocking,
//! so the callback stays well under the `LowLevelHooksTimeout`. The whole body
//! is wrapped in `catch_unwind` so a panic can't unwind across the FFI boundary.

use std::cell::Cell;
use std::panic::{catch_unwind, AssertUnwindSafe};

use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, HHOOK, MSLLHOOKSTRUCT, WM_MOUSEMOVE,
};

use crate::geometry::Point;
use crate::hook::hot_path::{count_event, route_move, MoveDedup};
use crate::platform::cursor::Win32Cursor;
use crate::shared::SHARED;

thread_local! {
    /// C++ `static previousLocation`. Thread-local because the callback only ever
    /// runs on the pump thread. The dedup logic itself lives in the neutral
    /// `hot_path` core, so the branch below is exactly what the benchmark measures.
    static PREV: Cell<MoveDedup> = const { Cell::new(MoveDedup::new()) };
}

/// # Safety
/// Invoked by Windows as a `WH_MOUSE_LL` hook procedure; never call it directly.
/// When `code >= 0`, `lparam` points to a valid `MSLLHOOKSTRUCT`, as guaranteed
/// by the OS.
pub unsafe extern "system" fn mouse_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let handled = catch_unwind(AssertUnwindSafe(|| process(code, wparam, lparam))).unwrap_or(false);

    if handled {
        LRESULT(1) // block the event so the cursor sticks to the border
    } else {
        unsafe { CallNextHookEx(HHOOK::default(), code, wparam, lparam) }
    }
}

fn process(code: i32, wparam: WPARAM, lparam: LPARAM) -> bool {
    if !is_mouse_move_message(code, wparam, lparam) {
        return false;
    }

    let ms = unsafe { &*(lparam.0 as *const MSLLHOOKSTRUCT) };
    let loc = (ms.pt.x, ms.pt.y);

    let changed = PREV.with(|prev| {
        let mut dedup = prev.get();
        let changed = dedup.accept(loc);
        prev.set(dedup);
        changed
    });
    if !changed {
        return false;
    }
    count_event();

    let Some(shared) = SHARED.get() else {
        return false;
    };

    // The whole non-blocking route — contended lock passes through, poisoned lock
    // is recovered, crossing counted — lives in the neutral `hot_path` core, so
    // the Windows callback and the benchmark exercise the same decision.
    let mut env = Win32Cursor;
    route_move(&shared.engine, &mut env, Point::new(loc.0, loc.1)).handled()
}

// The C++ hook filtered with `(wParam & WM_MOUSEMOVE) != 0`, but every mouse
// message carries bit 0x200 (WM_LBUTTONDOWN = 0x201, WM_MOUSEWHEEL = 0x20A…):
// clicks and wheel events entered the engine as moves, and a click landing on
// a border mid-crossing could be swallowed by the LRESULT(1) return.
fn is_mouse_move_message(code: i32, wparam: WPARAM, lparam: LPARAM) -> bool {
    code >= 0 && lparam.0 != 0 && wparam.0 == WM_MOUSEMOVE as usize
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::UI::WindowsAndMessaging::{
        WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MBUTTONDOWN, WM_MBUTTONUP, WM_MOUSEHWHEEL, WM_MOUSEWHEEL,
        WM_RBUTTONDOWN, WM_RBUTTONUP, WM_XBUTTONDOWN, WM_XBUTTONUP,
    };

    #[test]
    fn only_mouse_move_can_enter_the_routing_engine() {
        let pointer = LPARAM(1);
        assert!(is_mouse_move_message(
            0,
            WPARAM(WM_MOUSEMOVE as usize),
            pointer
        ));

        for message in [
            WM_LBUTTONDOWN,
            WM_LBUTTONUP,
            WM_RBUTTONDOWN,
            WM_RBUTTONUP,
            WM_MBUTTONDOWN,
            WM_MBUTTONUP,
            WM_MOUSEWHEEL,
            WM_MOUSEHWHEEL,
            WM_XBUTTONDOWN,
            WM_XBUTTONUP,
        ] {
            assert!(!is_mouse_move_message(0, WPARAM(message as usize), pointer));
        }
    }
}
