//! The tray under Windows: an icon in the notification area (`Shell_NotifyIcon`), what
//! C#'s Avalonia `TrayIcon` drew.
//!
//! A hidden top-level window on a thread of its own owns the icon and receives its clicks:
//! a left click opens the frontend, a right click shows the menu. It is top-level rather
//! than message-only so that it hears `TaskbarCreated`, broadcast when Explorer restarts:
//! the icon is then added again. The agent's side tells the window when the state changed;
//! the window redraws from the model, which both share.
//!
//! The update check's notices are the icon's balloon (a toast on Windows 10 and 11); a
//! click on one comes back as `NIN_BALLOONUSERCLICK`. Without an icon — the user hid it —
//! there is nowhere to show them, and they are only logged.

use std::cell::RefCell;
use std::sync::{Arc, Mutex};

use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    CreateBitmap, CreateDIBSection, DeleteObject, GetDC, MonitorFromPoint, ReleaseDC, BITMAPINFO,
    BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, HBITMAP, MONITOR_DEFAULTTONEAREST,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, GetSystemMetricsForDpi, MDT_EFFECTIVE_DPI};
use windows::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_ICON, NIF_INFO, NIF_MESSAGE, NIF_TIP, NIIF_INFO, NIIF_WARNING, NIM_ADD,
    NIM_DELETE, NIM_MODIFY, NIN_BALLOONUSERCLICK, NOTIFYICONDATAW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreateIconIndirect, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyIcon,
    DestroyMenu, DestroyWindow, DispatchMessageW, GetCursorPos, GetMessageW, GetSystemMetrics,
    PostMessageW, PostQuitMessage, RegisterClassExW, RegisterWindowMessageW, SetForegroundWindow,
    SetMenuItemInfoW, TrackPopupMenu, TranslateMessage, CW_USEDEFAULT, HICON, ICONINFO,
    MENUITEMINFOW, MF_SEPARATOR, MF_STRING, MIIM_BITMAP, MSG, SM_CXSMICON, TPM_NONOTIFY,
    TPM_RETURNCMD, TPM_RIGHTBUTTON, WINDOW_EX_STYLE, WM_APP, WM_CLOSE, WM_DESTROY, WM_LBUTTONUP,
    WM_NULL, WM_RBUTTONUP, WNDCLASSEXW, WS_OVERLAPPEDWINDOW,
};

use super::{follow, Action, TrayModel};
use crate::api::{self, Call};
use crate::icons::{self, Badge, MenuIcon, Pixmap, TrayIcon};
use crate::update::{Command, Notice};

const CLASS_NAME: PCWSTR = w!("LittleBigMouseAgentTray");
/// The icon's messages to its window.
const WM_TRAY: u32 = WM_APP + 1;
/// The agent's: the state changed, redraw.
const WM_REDRAW: u32 = WM_APP + 2;
/// The update check's: a notice waits in the model.
const WM_NOTICE: u32 = WM_APP + 3;
/// The icon's id within the window.
const ICON_ID: u32 = 1;

/// What the window procedure reads, on the tray's thread.
struct Tray {
    model: Arc<Mutex<TrayModel>>,
    /// One icon per state and badge, made once, in [`index`] order.
    icons: Vec<HICON>,
    /// Explorer's "the taskbar is back" broadcast.
    taskbar_created: u32,
    /// Whether the notification area is currently holding our icon. NIM_MODIFY on an
    /// icon that is not there fails, and NIM_ADD on one that is duplicates it.
    shown: bool,
}

thread_local! {
    static TRAY: RefCell<Option<Tray>> = const { RefCell::new(None) };
}

/// Puts the icon up and keeps it on the agent's state until the agent goes. Without a
/// notification area (a service session, a test runner) it says so and returns: the agent
/// runs on without one.
///
/// `updates` is the update check's end: where the menu's entry and the notices' clicks
/// go, and where its notices come from.
pub async fn run(
    calls: UnboundedSender<Call>,
    open: Arc<dyn Fn() + Send + Sync>,
    updates: Option<(UnboundedSender<Command>, UnboundedReceiver<Notice>)>,
) {
    let (client, frames) = api::in_process();
    let mut tray = TrayModel::new(calls.clone(), client.clone(), open);
    let mut notices = None;
    if let Some((commands, receiver)) = updates {
        tray = tray.with_updates(commands);
        notices = Some(receiver);
    }
    let model = Arc::new(Mutex::new(tray));
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
    let post = move |msg| {
        // SAFETY: a message to the tray's window, which checks what it is sent.
        unsafe { PostMessageW(HWND(window as _), msg, WPARAM(0), LPARAM(0)) }.is_ok()
    };
    // The notices, handed to the window as they come; this task ends with the check.
    let relay = notices.map(|mut notices| {
        let model = model.clone();
        tokio::spawn(async move {
            while let Some(notice) = notices.recv().await {
                model.lock().unwrap_or_else(|p| p.into_inner()).tell(notice);
                if !post(WM_NOTICE) {
                    return;
                }
            }
        })
    });
    follow(&calls, client, frames, |state| {
        model.lock().unwrap_or_else(|p| p.into_inner()).show(state);
        post(WM_REDRAW)
    })
    .await;
    if let Some(relay) = relay {
        relay.abort();
    }
    post(WM_CLOSE);
}

/// The tray's thread: the window, the icon, the message loop. Sends back the window (as
/// a number: a handle does not cross threads in the `windows` types) once the icon is up.
fn window_thread(model: Arc<Mutex<TrayModel>>, ready: tokio::sync::oneshot::Sender<Option<isize>>) {
    let icons: Vec<HICON> = STATES
        .iter()
        .flat_map(|&icon| BADGES.map(|badge| (icon, badge)))
        .map(|(icon, badge)| make_icon(best_size(icons::badged(icon, badge))).unwrap_or_default())
        .collect();
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
            shown: false,
        })
    });
    if !refresh(window) {
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
    let icons = TRAY.with(|tray| tray.borrow_mut().take().map(|t| t.icons));
    for icon in icons.into_iter().flatten() {
        // SAFETY: made above, no longer shown.
        unsafe {
            let _ = DestroyIcon(icon);
        }
    }
}

/// Puts the notification area in step with the model: the icon added, redrawn or taken
/// away, depending on what it shows and on whether the user asked for an icon at all.
/// Answers false only when something that had to happen did not.
fn refresh(window: HWND) -> bool {
    let (hidden, shown) = TRAY.with(|tray| {
        tray.borrow().as_ref().map_or((true, false), |tray| {
            let model = tray.model.lock().unwrap_or_else(|p| p.into_inner());
            (model.hidden(), tray.shown)
        })
    });
    let message = match (hidden, shown) {
        (true, false) => return true, // nothing to take away
        (true, true) => NIM_DELETE,
        (false, true) => NIM_MODIFY,
        (false, false) => NIM_ADD,
    };
    let done = notify(window, message);
    if done {
        TRAY.with(|tray| {
            if let Some(tray) = tray.borrow_mut().as_mut() {
                tray.shown = !hidden;
            }
        });
    }
    done
}

/// Shows the notice waiting in the model as the icon's balloon.
fn show_notice(window: HWND) {
    TRAY.with(|tray| {
        let tray = tray.borrow();
        let Some(tray) = tray.as_ref() else {
            return;
        };
        let mut model = tray.model.lock().unwrap_or_else(|p| p.into_inner());
        let Some(notice) = model.take_notice() else {
            return;
        };
        if !tray.shown {
            eprintln!(
                "[lbm-agent] {}: {} (no tray icon to say it)",
                notice.title, notice.text
            );
            return;
        }
        let mut data = NOTIFYICONDATAW {
            cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: window,
            uID: ICON_ID,
            uFlags: NIF_INFO,
            dwInfoFlags: if notice.warning {
                NIIF_WARNING
            } else {
                NIIF_INFO
            },
            ..Default::default()
        };
        fill(&mut data.szInfoTitle, &notice.title);
        fill(&mut data.szInfo, &notice.text);
        // SAFETY: a filled NOTIFYICONDATAW of this window's icon.
        if !unsafe { Shell_NotifyIconW(NIM_MODIFY, &data) }.as_bool() {
            eprintln!("[lbm-agent] {}: {} (not shown)", notice.title, notice.text);
        }
    });
}

/// `text` into a fixed, NUL-terminated buffer of the shell's, cut to fit.
fn fill(buffer: &mut [u16], text: &str) {
    let text: Vec<u16> = text.encode_utf16().take(buffer.len() - 1).collect();
    buffer[..text.len()].copy_from_slice(&text);
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
            hIcon: tray.icons[index(model.icon(), model.badge())],
            ..Default::default()
        };
        fill(
            &mut data.szTip,
            &format!("LittleBigMouse — {}", model.state()),
        );
        // SAFETY: a filled NOTIFYICONDATAW of this window.
        unsafe { Shell_NotifyIconW(message, &data) }.as_bool()
    })
}

const STATES: [TrayIcon; 4] = [
    TrayIcon::On,
    TrayIcon::Off,
    TrayIcon::Dead,
    TrayIcon::Paused,
];
const BADGES: [Badge; 3] = [Badge::None, Badge::Away, Badge::Driven];

/// Where the icon of `icon` under `badge` is in [`Tray::icons`].
fn index(icon: TrayIcon, badge: Badge) -> usize {
    let state = STATES.iter().position(|&s| s == icon).unwrap_or(0);
    let badge = BADGES.iter().position(|&b| b == badge).unwrap_or(0);
    state * BADGES.len() + badge
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
            NIN_BALLOONUSERCLICK => TRAY.with(|tray| {
                if let Some(tray) = tray.borrow().as_ref() {
                    tray.model
                        .lock()
                        .unwrap_or_else(|p| p.into_inner())
                        .notice_clicked();
                }
            }),
            _ => {}
        },
        WM_REDRAW => {
            refresh(window);
        }
        WM_NOTICE => show_notice(window),
        _ if Some(msg) == taskbar_created => {
            // Explorer restarted: it has forgotten the icon, whatever we believed.
            TRAY.with(|tray| {
                if let Some(tray) = tray.borrow_mut().as_mut() {
                    tray.shown = false;
                }
            });
            refresh(window);
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
///
/// Each entry has its pictogram as an item bitmap, made for this showing at the size of
/// a small icon on the screen the menu opens on (screens may differ in scale, and change)
/// and freed once the menu is gone: a menu does not own its items' bitmaps.
fn menu(window: HWND) {
    let mut at = POINT::default();
    // SAFETY: a position query.
    let _ = unsafe { GetCursorPos(&mut at) };
    let size = small_icon_at(at);
    let bitmaps: Vec<Option<HBITMAP>> = MenuIcon::ALL
        .iter()
        .map(|icon| make_bitmap(icons::at_least(icon.pixmaps(), size)).ok())
        .collect();
    // SAFETY: a popup menu owned here, destroyed below, before the bitmaps it shows.
    let chosen = unsafe {
        let Ok(menu) = CreatePopupMenu() else {
            free(bitmaps);
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
                    if let Some(bitmap) = bitmaps[action.icon() as usize] {
                        let item = MENUITEMINFOW {
                            cbSize: std::mem::size_of::<MENUITEMINFOW>() as u32,
                            fMask: MIIM_BITMAP,
                            hbmpItem: bitmap,
                            ..Default::default()
                        };
                        // By command id: the id given just above.
                        let _ = SetMenuItemInfoW(menu, (i + 1) as u32, false, &item);
                    }
                }
                None => {
                    let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
                }
            }
        }
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
    free(bitmaps);
    if let Some(Some(action)) = chosen.checked_sub(1).and_then(|i| Action::MENU.get(i)) {
        act(*action);
    }
}

/// The size the notification area draws (the small icon), or the next one up.
fn best_size(pixmaps: &[Pixmap]) -> &Pixmap {
    // SAFETY: a metrics query.
    let wanted = unsafe { GetSystemMetrics(SM_CXSMICON) }.max(16) as u32;
    icons::at_least(pixmaps, wanted)
}

/// The width of a small icon on the screen at `at`. The agent is aware of each screen's
/// DPI (per-monitor v2), and `GetSystemMetrics` answers for the primary screen's: a menu
/// opened on a 200 % screen beside a 100 % primary would get half-size pictograms.
fn small_icon_at(at: POINT) -> u32 {
    // SAFETY: a monitor and a DPI query.
    let dpi = unsafe {
        let monitor = MonitorFromPoint(at, MONITOR_DEFAULTTONEAREST);
        let (mut x, mut y) = (0, 0);
        GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut x, &mut y).map(|()| x)
    };
    match dpi {
        Ok(dpi) => small_icon_for_dpi(dpi),
        // SAFETY: a metrics query.
        Err(_) => unsafe { GetSystemMetrics(SM_CXSMICON) }.max(16) as u32,
    }
}

/// The width of a small icon at `dpi`.
fn small_icon_for_dpi(dpi: u32) -> u32 {
    // SAFETY: a metrics query.
    unsafe { GetSystemMetricsForDpi(SM_CXSMICON, dpi) }.max(16) as u32
}

/// A menu item's bitmap from a pixmap: a 32-bit top-down DIB whose colours are
/// premultiplied, which is what a themed menu blends with its alpha.
pub fn make_bitmap(pixmap: &Pixmap) -> windows::core::Result<HBITMAP> {
    let info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: pixmap.width as i32,
            biHeight: -(pixmap.height as i32),
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };
    let rows = icons::premultiplied_bgra(pixmap);
    // SAFETY: a DIB of the pixmap's size, filled within it; the caller frees it.
    unsafe {
        let screen = GetDC(None);
        let mut bits = std::ptr::null_mut();
        let bitmap = CreateDIBSection(screen, &info, DIB_RGB_COLORS, &mut bits, None, 0);
        ReleaseDC(None, screen);
        let bitmap = bitmap?;
        std::ptr::copy_nonoverlapping(rows.as_ptr(), bits.cast::<u8>(), rows.len());
        Ok(bitmap)
    }
}

/// Frees what [`make_bitmap`] made.
fn free(bitmaps: Vec<Option<HBITMAP>>) {
    for bitmap in bitmaps.into_iter().flatten() {
        // SAFETY: made by make_bitmap, no longer shown.
        unsafe {
            let _ = DeleteObject(bitmap);
        }
    }
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
            for badge in BADGES {
                for pixmap in icons::badged(icon, badge) {
                    let made = make_icon(pixmap).expect("an icon");
                    // SAFETY: made just above.
                    unsafe { DestroyIcon(made) }.unwrap();
                }
            }
            assert!(best_size(icons::pixmaps(icon)).width >= 16);
        }
    }

    #[test]
    fn every_state_and_badge_has_an_icon_of_its_own() {
        let mut seen: Vec<usize> = STATES
            .iter()
            .flat_map(|&icon| BADGES.map(|badge| index(icon, badge)))
            .collect();
        seen.sort_unstable();
        assert_eq!(seen, (0..STATES.len() * BADGES.len()).collect::<Vec<_>>());
    }

    #[test]
    fn every_menu_icon_makes_a_bitmap() {
        for icon in MenuIcon::ALL {
            for pixmap in icon.pixmaps() {
                let made = make_bitmap(pixmap).expect("a bitmap");
                // SAFETY: made just above.
                assert!(unsafe { DeleteObject(made) }.as_bool());
            }
        }
    }

    #[test]
    fn the_pictograms_follow_the_scale_of_the_screen() {
        // A 200 % screen asks for twice the 100 % size, and gets the bitmap of that size.
        let (normal, double) = (small_icon_for_dpi(96), small_icon_for_dpi(192));
        assert_eq!((normal, double), (16, 32));
        let stop = MenuIcon::Stop.pixmaps();
        assert_eq!(icons::at_least(stop, double).width, 32);
        // Wherever the menu opens, a size the pixmaps cover.
        assert!(small_icon_at(POINT { x: 0, y: 0 }) >= 16);
        assert!(
            small_icon_at(POINT {
                x: -100_000,
                y: 100_000
            }) >= 16,
            "off every screen"
        );
    }

    #[test]
    fn the_menu_ids_are_the_models_entries() {
        // The ids TrackPopupMenu returns are the entries' positions plus one.
        assert_eq!(Action::MENU[0], Some(Action::Open));
        assert_eq!(Action::MENU[4], None);
        assert_eq!(Action::MENU[5], Some(Action::CheckForUpdate));
        assert_eq!(Action::MENU[6], Some(Action::Exit));
    }
}
