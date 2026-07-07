//! Low-level mouse hook callback — port of `Hooker::MouseCallback`.
//!
//! This is the one genuinely hot, genuinely `unsafe` function. Phase 1 keeps it
//! a pass-through: it dedups by previous location (like the C++ `static
//! previousLocation`) but, with no zone engine yet, never marks an event handled,
//! so the cursor is unaffected. Phase 3 feeds the deduped position to the engine
//! under a non-blocking `try_lock`.

use std::cell::Cell;
use std::sync::atomic::{AtomicU64, Ordering};

use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{CallNextHookEx, HHOOK, MSLLHOOKSTRUCT, WM_MOUSEMOVE};

thread_local! {
    /// C++ `static previousLocation`. Thread-local because the callback only ever
    /// runs on the pump thread.
    static PREV: Cell<Option<(i32, i32)>> = const { Cell::new(None) };
}

/// Count of deduped mouse-move events the hook has processed. Lightweight
/// instrumentation (one relaxed increment) used to observe that the hook is
/// installed and still being called — i.e. not silently removed by the
/// `LowLevelHooksTimeout`.
pub static MOUSE_EVENTS: AtomicU64 = AtomicU64::new(0);

/// # Safety
/// Invoked by Windows as a `WH_MOUSE_LL` hook procedure; never call it directly.
/// When `code >= 0`, `lparam` points to a valid `MSLLHOOKSTRUCT`, as guaranteed
/// by the OS.
pub unsafe extern "system" fn mouse_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    crate::hook::guard(|| {
        // Faithful to the C++ filter `(wParam & WM_MOUSEMOVE) != 0`.
        if code >= 0 && lparam.0 != 0 && (wparam.0 & WM_MOUSEMOVE as usize) != 0 {
            let ms = unsafe { &*(lparam.0 as *const MSLLHOOKSTRUCT) };
            let loc = (ms.pt.x, ms.pt.y);
            PREV.with(|prev| {
                if prev.get() != Some(loc) {
                    prev.set(Some(loc));
                    MOUSE_EVENTS.fetch_add(1, Ordering::Relaxed);
                    // Phase 3: hand `loc` to the engine here (try_lock, no alloc).
                }
            });
        }
    });

    // The first argument is ignored by modern Windows; pass a null handle.
    unsafe { CallNextHookEx(HHOOK::default(), code, wparam, lparam) }
}
