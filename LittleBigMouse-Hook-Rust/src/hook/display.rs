//! Display-change listener window — port of `HookerDisplayChanged.cpp`.
//!
//! A hidden **top-level** window (not message-only): `WM_DISPLAYCHANGE` and the
//! `WM_SETTINGCHANGE`/`SPI_SETWORKAREA` broadcasts are only delivered to
//! top-level windows, so a message-only (`HWND_MESSAGE`) window would never see
//! them. The window is created but never shown.

use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, RegisterClassExW, CW_USEDEFAULT,
    SPI_SETWORKAREA, WINDOW_EX_STYLE, WM_DISPLAYCHANGE, WM_SETTINGCHANGE, WNDCLASSEXW,
    WS_OVERLAPPEDWINDOW,
};

use crate::shared::SHARED;

const CLASS_NAME: PCWSTR = w!("HookerDisplayChange");

/// Register the class (idempotent — `do_hook` runs every loop iteration) and
/// create the hidden window. Returns a null `HWND` on failure.
pub fn create_window() -> HWND {
    unsafe {
        let hmodule = GetModuleHandleW(None).unwrap_or_default();
        let hinstance = HINSTANCE(hmodule.0);

        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(wnd_proc),
            hInstance: hinstance,
            lpszClassName: CLASS_NAME,
            ..Default::default()
        };
        // Second and later registrations fail with ERROR_CLASS_ALREADY_EXISTS,
        // which is harmless — the class stays registered and CreateWindow works.
        RegisterClassExW(&wc);

        CreateWindowExW(
            WINDOW_EX_STYLE(0),
            CLASS_NAME,
            CLASS_NAME,
            WS_OVERLAPPEDWINDOW,
            CW_USEDEFAULT,
            0,
            CW_USEDEFAULT,
            0,
            None,
            None,
            hinstance,
            None,
        )
        .unwrap_or_default()
    }
}

pub fn destroy_window(hwnd: HWND) {
    if hwnd != HWND::default() {
        unsafe {
            let _ = DestroyWindow(hwnd);
        }
    }
}

unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_DISPLAYCHANGE => {
            crate::hook::guard(|| {
                if let Some(shared) = SHARED.get() {
                    crate::hook::on_display_changed(shared);
                }
            });
            LRESULT(0)
        }
        WM_SETTINGCHANGE if wparam.0 as u32 == SPI_SETWORKAREA.0 => {
            crate::hook::guard(|| {
                if let Some(shared) = SHARED.get() {
                    crate::hook::on_setting_changed(shared);
                }
            });
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}
