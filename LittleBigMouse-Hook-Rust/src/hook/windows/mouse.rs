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
use std::sync::atomic::Ordering;

use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, HHOOK, LLMHF_INJECTED, LLMHF_LOWER_IL_INJECTED, MSLLHOOKSTRUCT, WM_MOUSEMOVE,
};

use crate::engine::event::MouseEventArg;
use crate::geometry::Point;
use crate::hook::{CROSSINGS, MOUSE_EVENTS};
use crate::platform::cursor::Win32Cursor;
use crate::shared::SHARED;

thread_local! {
    /// C++ `static previousLocation`. Thread-local because the callback only ever
    /// runs on the pump thread.
    static PREV: Cell<Option<(i32, i32)>> = const { Cell::new(None) };
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
        if prev.get() != Some(loc) {
            prev.set(Some(loc));
            true
        } else {
            false
        }
    });
    if !changed {
        return false;
    }
    MOUSE_EVENTS.fetch_add(1, Ordering::Relaxed);

    let Some(shared) = SHARED.get() else {
        return false;
    };
    // Non-blocking: a contended lock (Load in progress) just passes the event on. Recover from a
    // POISONED lock instead of giving up forever: if `on_mouse_move` ever panics (e.g. a debug-build
    // arithmetic overflow at extreme corner coordinates), the guard drops and poisons the mutex —
    // and without recovery every later event would fail this lock and crossing would stay dead until
    // a restart (exactly the "touch a corner -> no more crossing, only restart fixes it" bug).
    let mut engine = match shared.engine.try_lock() {
        Ok(g) => g,
        Err(std::sync::TryLockError::Poisoned(p)) => p.into_inner(),
        Err(std::sync::TryLockError::WouldBlock) => return false,
    };

    let mut env = Win32Cursor;
    let mut e = MouseEventArg::new(Point::new(loc.0, loc.1));
    e.injected = ms.flags & (LLMHF_INJECTED | LLMHF_LOWER_IL_INJECTED) != 0;
    engine.on_mouse_move(&mut env, &mut e);
    if e.handled {
        CROSSINGS.fetch_add(1, Ordering::Relaxed);
    }
    e.handled
}

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
