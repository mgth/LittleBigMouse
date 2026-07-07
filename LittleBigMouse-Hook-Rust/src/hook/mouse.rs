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
use std::sync::atomic::{AtomicU64, Ordering};

use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{CallNextHookEx, HHOOK, MSLLHOOKSTRUCT, WM_MOUSEMOVE};

use crate::engine::event::MouseEventArg;
use crate::geometry::Point;
use crate::platform::cursor::Win32Cursor;
use crate::shared::SHARED;

thread_local! {
    /// C++ `static previousLocation`. Thread-local because the callback only ever
    /// runs on the pump thread.
    static PREV: Cell<Option<(i32, i32)>> = const { Cell::new(None) };
}

/// Count of deduped mouse-move events the hook has processed. Lightweight
/// instrumentation (one relaxed increment) to observe the hook staying alive.
pub static MOUSE_EVENTS: AtomicU64 = AtomicU64::new(0);

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
    // Faithful to the C++ filter `(wParam & WM_MOUSEMOVE) != 0`.
    if !(code >= 0 && lparam.0 != 0 && (wparam.0 & WM_MOUSEMOVE as usize) != 0) {
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
    // Non-blocking: a contended lock (Load in progress) just passes the event on.
    let Ok(mut engine) = shared.engine.try_lock() else {
        return false;
    };

    let mut env = Win32Cursor;
    let mut e = MouseEventArg::new(Point::new(loc.0, loc.1));
    engine.on_mouse_move(&mut env, &mut e);
    e.handled
}
