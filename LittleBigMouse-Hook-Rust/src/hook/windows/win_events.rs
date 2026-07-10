//! WinEvent callbacks — port of `Hooker::WindowChangeHook` / `DesktopChangeHook`.
//!
//! Registered `WINEVENT_OUTOFCONTEXT`, so the system posts them to the pump
//! thread's queue and they run during `DispatchMessage` — never re-entrant with
//! `do_hook`/`do_unhook`.

use std::cell::Cell;

use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Accessibility::HWINEVENTHOOK;

use crate::shared::SHARED;

thread_local! {
    /// C++ `static lastHwnd` — dedups repeated focus events for the same window.
    static LAST_HWND: Cell<isize> = const { Cell::new(0) };
}

/// # Safety
/// Invoked by Windows as a `WINEVENTPROC` for `EVENT_OBJECT_FOCUS`; never call
/// it directly.
pub unsafe extern "system" fn focus_proc(
    _hook: HWINEVENTHOOK,
    _event: u32,
    hwnd: HWND,
    _id_object: i32,
    _id_child: i32,
    _thread: u32,
    _time: u32,
) {
    crate::hook::guard(|| {
        let Some(shared) = SHARED.get() else { return };

        let key = hwnd.0 as isize;
        let changed = LAST_HWND.with(|last| {
            if last.get() != key {
                last.set(key);
                true
            } else {
                false
            }
        });
        if !changed {
            return;
        }

        if let Some(path) = crate::platform::process::exe_path_from_window(hwnd) {
            if !path.is_empty() {
                crate::hook::on_focus_changed(shared, path);
            }
        }
    });
}

/// # Safety
/// Invoked by Windows as a `WINEVENTPROC` for `EVENT_SYSTEM_DESKTOPSWITCH`;
/// never call it directly.
pub unsafe extern "system" fn desktop_proc(
    _hook: HWINEVENTHOOK,
    _event: u32,
    _hwnd: HWND,
    _id_object: i32,
    _id_child: i32,
    _thread: u32,
    _time: u32,
) {
    crate::hook::guard(|| {
        if let Some(shared) = SHARED.get() {
            crate::hook::on_desktop_changed(shared);
        }
    });
}
