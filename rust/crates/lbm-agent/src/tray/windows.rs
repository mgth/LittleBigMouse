//! The tray under Windows: an icon in the notification area (`Shell_NotifyIcon`), what
//! C#'s Avalonia `TrayIcon` drew.
//!
//! A hidden top-level window on a thread of its own owns the icon and receives its clicks:
//! a left click opens the frontend, a right click shows the menu. It is top-level rather
//! than message-only so that it hears `TaskbarCreated`, broadcast when Explorer restarts:
//! the icon is then added again. The agent's side tells the window when the state changed;
//! the window redraws from the model, which both share.

use std::cell::RefCell;
use std::sync::{Arc, Mutex};

use tokio::sync::mpsc::UnboundedSender;
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    CreateBitmap, CreateDIBSection, DeleteObject, GetDC, ReleaseDC, BITMAPINFO, BITMAPINFOHEADER,
    BI_RGB, DIB_RGB_COLORS,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_MODIFY,
    NOTIFYICONDATAW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreateIconIndirect, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyIcon,
    DestroyMenu, DestroyWindow, DispatchMessageW, GetCursorPos, GetMessageW, GetSystemMetrics,
    PostMessageW, PostQuitMessage, RegisterClassExW, RegisterWindowMessageW, SetForegroundWindow,
    TrackPopupMenu, TranslateMessage, CW_USEDEFAULT, HICON, ICONINFO, MF_SEPARATOR, MF_STRING, MSG,
    SM_CXSMICON, TPM_NONOTIFY, TPM_RETURNCMD, TPM_RIGHTBUTTON, WINDOW_EX_STYLE, WM_APP, WM_CLOSE,
    WM_DESTROY, WM_LBUTTONUP, WM_NULL, WM_RBUTTONUP, WNDCLASSEXW, WS_OVERLAPPEDWINDOW,
};

use super::{follow, Action, TrayModel};
use crate::api::{self, Call};
use crate::icons::{self, Pixmap, TrayIcon};

const CLASS_NAME: PCWSTR = w!("LittleBigMouseAgentTray");
/// The icon's messages to its window.
const WM_TRAY: u32 = WM_APP + 1;
/// The agent's: the state changed, redraw.
const WM_REDRAW: u32 = WM_APP + 2;
/// The icon's id within the window.
const ICON_ID: u32 = 1;

/// What the window procedure reads, on the tray's thread.
struct Tray {
    model: Arc<Mutex<TrayModel>>,
    /// One icon per state, made once.
    icons: [HICON; 4],
    /// Explorer's "the taskbar is back" broadcast.
    taskbar_created: u32,
}

thread_local! {
    static TRAY: RefCell<Option<Tray>> = const { RefCell::new(None) };
}

/// Puts the icon up and keeps it on the agent's state until the agent goes. Without a
/// notification area (a service session, a test runner) it says so and returns: the agent
/// runs on without one.
pub async fn run(calls: UnboundedSender<Call>, open: Arc<dyn Fn() + Send + Sync>) {
    let (client, frames) = api::in_process();
    let model = Arc::new(Mutex::new(TrayModel::new(
        calls.clone(),
        client.clone(),
        open,
    )));
    let (ready, window) = tokio::sync::oneshot::channel();
    let shared = model.clone();
    let spawned = std::thread::Builder::new()
        .name("tray".to_owned())
        .spawn(move || window_thread(shared, ready));
    if let Err(error) = spawned {
        eprintln!("[lbm-agent] no tray: {error}");
        return;
    }
    let Ok(Some(window)) = window.await else {
        eprintln!("[lbm-agent] no tray: the notification area refused the icon");
        return;
    };
    let post = |msg| {
        // SAFETY: a message to the tray's window, which checks what it is sent.
        unsafe { PostMessageW(HWND(window as _), msg, WPARAM(0), LPARAM(0)) }.is_ok()
    };
    follow(&calls, client, frames, |state| {
        model.lock().unwrap_or_else(|p| p.into_inner()).show(state);
        post(WM_REDRAW)
    })
    .await;
    post(WM_CLOSE);
}

/// The tray's thread: the window, the icon, the message loop. Sends back the window (as
/// a number: a handle does not cross threads in the `windows` types) once the icon is up.
fn window_thread(model: Arc<Mutex<TrayModel>>, ready: tokio::sync::oneshot::Sender<Option<isize>>) {
    let icons = [
        TrayIcon::On,
        TrayIcon::Off,
        TrayIcon::Dead,
        TrayIcon::Paused,
    ]
    .map(|icon| make_icon(best_size(icons::pixmaps(icon))).unwrap_or_default());
    // SAFETY: plain Win32 window setup on this thread, torn down below on this thread.
    let window = unsafe {
        let module = GetModuleHandleW(None).unwrap_or_default();
        let instance = HINSTANCE(module.0);
        let class = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(window_procedure),
            hInstance: instance,
            lpszClassName: CLASS_NAME,
            ..Default::default()
        };
        RegisterClassExW(&class);
        CreateWindowExW(
            WINDOW_EX_STYLE(0),
            CLASS_NAME,
            w!("LittleBigMouse"),
            WS_OVERLAPPEDWINDOW,
            CW_USEDEFAULT,
            0,
            CW_USEDEFAULT,
            0,
            None,
            None,
            instance,
            None,
        )
    };
    let Ok(window) = window else {
        let _ = ready.send(None);
        return;
    };
    TRAY.with(|tray| {
        *tray.borrow_mut() = Some(Tray {
            model,
            icons,
            // SAFETY: registers (or finds) a message name.
            taskbar_created: unsafe { RegisterWindowMessageW(w!("TaskbarCreated")) },
        })
    });
    if !notify(window, NIM_ADD) {
        // SAFETY: the window created above.
        unsafe {
            let _ = DestroyWindow(window);
        }
        let _ = ready.send(None);
        return;
    }
    let _ = ready.send(Some(window.0 as isize));

    let mut message = MSG::default();
    // SAFETY: this thread's message loop.
    unsafe {
        while GetMessageW(&mut message, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
    for icon in icons {
        // SAFETY: made above, no longer shown.
        unsafe {
            let _ = DestroyIcon(icon);
        }
    }
}

/// Adds, redraws or removes the icon from the model.
fn notify(window: HWND, message: windows::Win32::UI::Shell::NOTIFY_ICON_MESSAGE) -> bool {
    TRAY.with(|tray| {
        let tray = tray.borrow();
        let Some(tray) = tray.as_ref() else {
            return false;
        };
        let model = tray.model.lock().unwrap_or_else(|p| p.into_inner());
        let mut data = NOTIFYICONDATAW {
            cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: window,
            uID: ICON_ID,
            uFlags: NIF_MESSAGE | NIF_ICON | NIF_TIP,
            uCallbackMessage: WM_TRAY,
            hIcon: tray.icons[index(model.icon())],
            ..Default::default()
        };
        let tip: Vec<u16> = format!("LittleBigMouse — {}", model.state())
            .encode_utf16()
            .take(data.szTip.len() - 1)
            .collect();
        data.szTip[..tip.len()].copy_from_slice(&tip);
        // SAFETY: a filled NOTIFYICONDATAW of this window.
        unsafe { Shell_NotifyIconW(message, &data) }.as_bool()
    })
}

fn index(icon: TrayIcon) -> usize {
    match icon {
        TrayIcon::On => 0,
        TrayIcon::Off => 1,
        TrayIcon::Dead => 2,
        TrayIcon::Paused => 3,
    }
}

unsafe extern "system" fn window_procedure(
    window: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let taskbar_created = TRAY.with(|t| t.borrow().as_ref().map(|t| t.taskbar_created));
    match msg {
        WM_TRAY => match lparam.0 as u32 {
            WM_LBUTTONUP => act(Action::Open),
            WM_RBUTTONUP => menu(window),
            _ => {}
        },
        WM_REDRAW => {
            notify(window, NIM_MODIFY);
        }
        _ if Some(msg) == taskbar_created => {
            notify(window, NIM_ADD);
        }
        WM_CLOSE => {
            notify(window, NIM_DELETE);
            // SAFETY: this thread's window.
            unsafe {
                let _ = DestroyWindow(window);
            }
        }
        // SAFETY: ends this thread's message loop.
        WM_DESTROY => unsafe { PostQuitMessage(0) },
        // SAFETY: the default procedure, with what the window was sent.
        _ => return unsafe { DefWindowProcW(window, msg, wparam, lparam) },
    }
    LRESULT(0)
}

fn act(action: Action) {
    TRAY.with(|tray| {
        if let Some(tray) = tray.borrow().as_ref() {
            tray.model
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .act(action);
        }
    });
}

/// The menu at the cursor; the entry chosen, acted on.
fn menu(window: HWND) {
    // SAFETY: a popup menu owned here, destroyed below.
    let chosen = unsafe {
        let Ok(menu) = CreatePopupMenu() else {
            return;
        };
        let mut labels = Vec::new();
        for (i, entry) in Action::MENU.iter().enumerate() {
            match entry {
                Some(action) => {
                    labels.push(
                        action
                            .label()
                            .encode_utf16()
                            .chain(Some(0))
                            .collect::<Vec<u16>>(),
                    );
                    let label = PCWSTR(labels.last().unwrap().as_ptr());
                    let _ = AppendMenuW(menu, MF_STRING, i + 1, label);
                }
                None => {
                    let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
                }
            }
        }
        let mut at = POINT::default();
        let _ = GetCursorPos(&mut at);
        // Without the foreground, the menu would not close when clicking elsewhere.
        let _ = SetForegroundWindow(window);
        let chosen = TrackPopupMenu(
            menu,
            TPM_RETURNCMD | TPM_NONOTIFY | TPM_RIGHTBUTTON,
            at.x,
            at.y,
            0,
            window,
            None,
        );
        let _ = PostMessageW(window, WM_NULL, WPARAM(0), LPARAM(0));
        let _ = DestroyMenu(menu);
        chosen.0 as usize
    };
    if let Some(Some(action)) = chosen.checked_sub(1).and_then(|i| Action::MENU.get(i)) {
        act(*action);
    }
}

/// The size the notification area draws (the small icon), or the next one up.
fn best_size(pixmaps: &[Pixmap]) -> &Pixmap {
    // SAFETY: a metrics query.
    let wanted = unsafe { GetSystemMetrics(SM_CXSMICON) }.max(16) as u32;
    pixmaps
        .iter()
        .find(|p| p.width >= wanted)
        .unwrap_or_else(|| pixmaps.last().expect("icons of every size"))
}

/// An icon from a pixmap: a 32-bit top-down color bitmap, alpha included, and an empty
/// mask (the alpha channel is what Windows draws with).
pub fn make_icon(pixmap: &Pixmap) -> windows::core::Result<HICON> {
    let (width, height) = (pixmap.width as i32, pixmap.height as i32);
    let info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width,
            biHeight: -height,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };
    // SAFETY: the bitmaps are made, filled within their size, and freed here; the icon
    // copies them.
    unsafe {
        let screen = GetDC(None);
        let mut bits = std::ptr::null_mut();
        let color = CreateDIBSection(screen, &info, DIB_RGB_COLORS, &mut bits, None, 0);
        ReleaseDC(None, screen);
        let color = color?;
        // ARGB bytes (a, r, g, b) to the DIB's BGRA.
        let pixels = std::slice::from_raw_parts_mut(bits.cast::<u8>(), pixmap.argb.len());
        let (pixels, _) = pixels.as_chunks_mut::<4>();
        for (to, from) in pixels.iter_mut().zip(pixmap.argb.as_chunks::<4>().0) {
            *to = [from[3], from[2], from[1], from[0]];
        }
        let mask = CreateBitmap(width, height, 1, 1, None);
        let icon = CreateIconIndirect(&ICONINFO {
            fIcon: true.into(),
            xHotspot: 0,
            yHotspot: 0,
            hbmMask: mask,
            hbmColor: color,
        });
        let _ = DeleteObject(color);
        let _ = DeleteObject(mask);
        icon
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_state_makes_an_icon() {
        for icon in [
            TrayIcon::On,
            TrayIcon::Off,
            TrayIcon::Dead,
            TrayIcon::Paused,
        ] {
            for pixmap in icons::pixmaps(icon) {
                let made = make_icon(pixmap).expect("an icon");
                // SAFETY: made just above.
                unsafe { DestroyIcon(made) }.unwrap();
            }
            assert!(best_size(icons::pixmaps(icon)).width >= 16);
        }
    }

    #[test]
    fn the_menu_ids_are_the_models_entries() {
        // The ids TrackPopupMenu returns are the entries' positions plus one.
        assert_eq!(Action::MENU[0], Some(Action::Open));
        assert_eq!(Action::MENU[5], Some(Action::Exit));
        assert_eq!(Action::MENU[4], None);
    }
}
