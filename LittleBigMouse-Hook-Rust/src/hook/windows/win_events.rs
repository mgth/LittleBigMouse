//! Foreground and desktop WinEvent callbacks.
//!
//! `WINEVENT_OUTOFCONTEXT` posts callbacks to the pump thread, so these never
//! re-enter hook installation/removal.

use std::cell::RefCell;

use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Accessibility::HWINEVENTHOOK;

use crate::shared::SHARED;

thread_local! {
    /// Deduplicate only identical foreground process identities. Control-focus
    /// churn within an application is irrelevant to process exclusions.
    static LAST_PATH: RefCell<String> = const { RefCell::new(String::new()) };
}

/// # Safety
/// Invoked by Windows for `EVENT_SYSTEM_FOREGROUND`; never call directly.
pub unsafe extern "system" fn foreground_proc(
    _hook: HWINEVENTHOOK,
    _event: u32,
    _hwnd: HWND,
    _id_object: i32,
    _id_child: i32,
    _thread: u32,
    _time: u32,
) {
    crate::hook::guard(|| {
        let Some(shared) = SHARED.get() else { return };
        let Some(path) = crate::platform::process::foreground_process_path() else {
            return;
        };
        if path.is_empty() {
            return;
        }

        let duplicate = LAST_PATH.with(|last| {
            let mut last = last.borrow_mut();
            let same = crate::platform::process::path_contains(&last, &path)
                && crate::platform::process::path_contains(&path, &last);
            if !same {
                *last = path.clone();
            }
            same
        });
        if !duplicate {
            crate::hook::on_focus_changed(shared, path);
        }
    });
}

/// # Safety
/// Invoked by Windows for `EVENT_SYSTEM_DESKTOPSWITCH`; never call directly.
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
