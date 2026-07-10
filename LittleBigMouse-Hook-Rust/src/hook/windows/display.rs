//! Display-change listener window — port of `HookerDisplayChanged.cpp`.
//!
//! A hidden **top-level** window (not message-only): `WM_DISPLAYCHANGE` and the
//! `WM_SETTINGCHANGE`/`SPI_SETWORKAREA` broadcasts are only delivered to
//! top-level windows, so a message-only (`HWND_MESSAGE`) window would never see
//! them. The window is created but never shown.

use windows::core::{w, GUID, PCWSTR};
use windows::Win32::Foundation::{HANDLE, HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Power::{RegisterPowerSettingNotification, POWERBROADCAST_SETTING};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, RegisterClassExW, CW_USEDEFAULT,
    DEVICE_NOTIFY_WINDOW_HANDLE, PBT_POWERSETTINGCHANGE, SPI_SETWORKAREA, WINDOW_EX_STYLE,
    WM_DISPLAYCHANGE, WM_POWERBROADCAST, WM_SETTINGCHANGE, WNDCLASSEXW, WS_OVERLAPPEDWINDOW,
};

use crate::shared::SHARED;

const CLASS_NAME: PCWSTR = w!("HookerDisplayChange");

/// GUID_CONSOLE_DISPLAY_STATE {6FE69556-704A-47A0-8F24-C28D936FDA47}: the display power state,
/// delivered as a `WM_POWERBROADCAST`/`PBT_POWERSETTINGCHANGE`. Data = 0 off, 1 on, 2 dimmed.
/// This is the signal that actually fires for this machine's "veille" (a session-state transition,
/// not a classic S3 suspend — so PBT_APMSUSPEND is never sent).
const GUID_CONSOLE_DISPLAY_STATE: GUID =
    GUID::from_values(0x6fe69556, 0x704a, 0x47a0, [0x8f, 0x24, 0xc2, 0x8d, 0x93, 0x6f, 0xda, 0x47]);

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

        let hwnd = CreateWindowExW(
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
        .unwrap_or_default();

        // Subscribe to display on/off. Bound to the window handle, so it is released automatically
        // when the window is destroyed — which happens on every hook/unhook cycle, along with the
        // window itself. On (re)registration Windows immediately re-pushes the current state; the
        // `suspended` dedup in `on_suspend`/`on_resume` absorbs that.
        if hwnd != HWND::default() {
            let _ = RegisterPowerSettingNotification(
                HANDLE(hwnd.0),
                &GUID_CONSOLE_DISPLAY_STATE,
                DEVICE_NOTIFY_WINDOW_HANDLE,
            );
        }

        hwnd
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
        WM_POWERBROADCAST if wparam.0 as u32 == PBT_POWERSETTINGCHANGE => {
            // lParam -> POWERBROADCAST_SETTING. For GUID_CONSOLE_DISPLAY_STATE, Data[0] is the
            // display state: 0 off (-> suspend), 1 on (-> resume), 2 dimmed (ignored).
            let setting = unsafe { &*(lparam.0 as *const POWERBROADCAST_SETTING) };
            if setting.PowerSetting == GUID_CONSOLE_DISPLAY_STATE {
                let state = setting.Data[0];
                crate::hook::guard(|| {
                    if let Some(shared) = SHARED.get() {
                        match state {
                            0 => crate::hook::on_suspend(shared),
                            1 => crate::hook::on_resume(shared),
                            _ => {}
                        }
                    }
                });
            }
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}
