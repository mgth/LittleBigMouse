//! Hiding a Windows remote's pointer while no master drives it, and a Windows
//! master's while its pointer is on another machine (`hook::windows::master`).
//!
//! A process cannot hide the system cursor everywhere: `ShowCursor` only counts for
//! the windows of the calling thread, and replacing the system cursors would outlive
//! a crash. What any process can do is decide the cursor over its own window. So, as
//! Deskflow's Windows client does, the pointer is parked on a window of ours — one
//! pixel, topmost, almost fully transparent, never activated, absent from Alt-Tab and
//! the taskbar — whose class has no cursor and which answers `WM_SETCURSOR` with no
//! cursor. Nothing global is changed: the moment the pointer is anywhere else, the
//! window under it sets its own cursor again, and if this process dies the window
//! goes with it.
//!
//! The pointer comes back:
//! - when a master enters again: [`Hider::unpark`] hides the window, waiting a bounded
//!   time for it to be done, before the injector sends the first motion;
//! - when this machine's own mouse moves: one pixel is enough to take the pointer off
//!   the window, which then hears `WM_MOUSELEAVE` (it asked for it with
//!   `TrackMouseEvent` once the pointer reached it) and takes itself away. A click or a
//!   wheel turn without a move lands on the window itself and does the same. When the
//!   low-level mouse hook happens to be installed, it reports a move that is not ours
//!   too ([`local_pointer_input`]); the window alone does not depend on it;
//! - when the hook leaves: the endpoint unparks on `stop`, and dropping the hider
//!   destroys the window.
//!
//! The window lives on a thread of its own that pumps its messages. The injection
//! thread only posts to it, except to unpark, where it waits at most
//! [`UNPARK_TIMEOUT`]; the routing thread never deals with it.

use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicU64, Ordering};
use std::sync::mpsc;
use std::time::Duration;

use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{COLORREF, HINSTANCE, HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::{
    SetThreadDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{TrackMouseEvent, TME_LEAVE, TRACKMOUSEEVENT};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetCursorPos, GetMessageW,
    PostMessageW, PostQuitMessage, RegisterClassExW, SendMessageTimeoutW, SetCursor, SetCursorPos,
    SetLayeredWindowAttributes, SetWindowPos, TranslateMessage, HCURSOR, HWND_TOPMOST, LWA_ALPHA,
    MA_NOACTIVATE, MSG, SMTO_ABORTIFHUNG, SWP_HIDEWINDOW, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
    SWP_NOZORDER, SWP_SHOWWINDOW, WM_APP, WM_CLOSE, WM_DESTROY, WM_LBUTTONDOWN, WM_MBUTTONDOWN,
    WM_MOUSEACTIVATE, WM_MOUSEHWHEEL, WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_RBUTTONDOWN, WM_SETCURSOR,
    WM_XBUTTONDOWN, WNDCLASSEXW, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST,
    WS_POPUP,
};

use crate::remote::parking::{hider_rect, parking_spot, ParkStep, Screen, PARK_ORDER};

const CLASS_NAME: PCWSTR = w!("LittleBigMouseRemotePointer");

/// `WM_MOUSELEAVE`, which the `windows` crate files under `Win32_UI_Controls`: a whole
/// feature for one number.
const WM_MOUSELEAVE: u32 = 0x02a3;

/// "Make the window match [`WANTED`]". One message for both ways, and the wanted state
/// in an atomic rather than in the message: an unpark is sent, and Windows hands sent
/// messages over before posted ones, so a park posted just earlier can be handled
/// after it. Read at handling time, the state is always the latest.
const WM_RECONCILE: u32 = WM_APP + 0x4c;

/// How long [`Hider::unpark`] waits for the window to be gone before the first motion
/// goes out anyway. The window's thread does nothing else, so it answers in far less;
/// the bound is for a thread that hung, which must not stall the injection. Even then
/// the pointer shows on that first motion: it leaves the one pixel the window covers.
const UNPARK_TIMEOUT: Duration = Duration::from_millis(50);

/// How long the first park waits for the window to be created.
const CREATE_TIMEOUT: Duration = Duration::from_secs(1);

/// Whether the pointer is to be parked. Written by the injection thread, and by
/// whatever sees this machine's own mouse move; obeyed by the window's thread.
static WANTED: AtomicBool = AtomicBool::new(false);

/// The window while it hides the pointer, else 0. One hider per process, as there is
/// one injector: the low-level mouse hook reads this to know whether there is anything
/// to give back, with one load.
static SHOWN: AtomicIsize = AtomicIsize::new(0);

/// Where the pointer was parked, packed: a report of the pointer at that very place is
/// the parking itself, not a person.
static SPOT: AtomicU64 = AtomicU64::new(u64::MAX);

fn pack((x, y): (i32, i32)) -> u64 {
    (u64::from(x as u32) << 32) | u64::from(y as u32)
}

fn hwnd(raw: isize) -> HWND {
    HWND(raw as *mut _)
}

/// The window that hides the pointer, created at the first park and kept.
#[derive(Default)]
pub struct Hider {
    /// `HWND` as an integer: the handle itself is not `Send`, and this lives on the
    /// injection thread.
    window: Option<isize>,
    failed: bool,
}

impl Hider {
    /// Parks the pointer. Never waits, but the first time, for the window's creation.
    pub fn park(&mut self) {
        WANTED.store(true, Ordering::Release);
        if let Some(window) = self.window() {
            unsafe {
                let _ = PostMessageW(hwnd(window), WM_RECONCILE, WPARAM(0), LPARAM(0));
            }
        }
    }

    /// Gives the pointer back, the window gone by the time this returns — unless its
    /// thread does not answer within [`UNPARK_TIMEOUT`].
    pub fn unpark(&mut self) {
        WANTED.store(false, Ordering::Release);
        if let Some(window) = self.window {
            unsafe {
                let _ = SendMessageTimeoutW(
                    hwnd(window),
                    WM_RECONCILE,
                    WPARAM(0),
                    LPARAM(0),
                    SMTO_ABORTIFHUNG,
                    UNPARK_TIMEOUT.as_millis() as u32,
                    None,
                );
            }
        }
    }

    /// Makes the window now, so that the first [`Self::park`] does not wait for it: a
    /// Windows master parks from its low-level hook, which must never wait that long.
    pub fn prepare(&mut self) {
        self.window();
    }

    fn window(&mut self) -> Option<isize> {
        if self.window.is_none() && !self.failed {
            self.window = spawn_window();
            if self.window.is_none() {
                self.failed = true;
                eprintln!(
                    "[LittleBigMouse.Hook] remote input: no window to hide the parked \
                     pointer; it stays visible"
                );
            }
        }
        self.window
    }
}

impl Drop for Hider {
    fn drop(&mut self) {
        WANTED.store(false, Ordering::Release);
        if let Some(window) = self.window.take() {
            unsafe {
                let _ = PostMessageW(hwnd(window), WM_CLOSE, WPARAM(0), LPARAM(0));
            }
        }
    }
}

/// Where the pointer is parked, while it is.
pub fn parked_at() -> Option<(i32, i32)> {
    if SHOWN.load(Ordering::Acquire) == 0 {
        return None;
    }
    let spot = SPOT.load(Ordering::Acquire);
    (spot != u64::MAX).then_some(((spot >> 32) as u32 as i32, spot as u32 as i32))
}

/// This machine's own pointer did something at `at` — an event the low-level mouse
/// hook saw that this injector did not send. Gives the pointer back if it is parked.
/// One atomic load when it is not, which is nearly always: this runs on the hook's
/// hot path.
pub fn local_pointer_input(at: (i32, i32)) {
    let window = SHOWN.load(Ordering::Acquire);
    if window == 0 || pack(at) == SPOT.load(Ordering::Acquire) {
        return;
    }
    WANTED.store(false, Ordering::Release);
    unsafe {
        let _ = PostMessageW(hwnd(window), WM_RECONCILE, WPARAM(0), LPARAM(0));
    }
}

/// Starts the window's thread and waits for its window.
fn spawn_window() -> Option<isize> {
    let (created, wait) = mpsc::sync_channel(1);
    std::thread::Builder::new()
        .name("lbm-remote-pointer".to_string())
        .spawn(move || {
            let window = create_window();
            let _ = created.send(window);
            if window.is_some() {
                pump();
            }
        })
        .ok()?;
    wait.recv_timeout(CREATE_TIMEOUT).ok().flatten()
}

fn create_window() -> Option<isize> {
    unsafe {
        // The process is per-monitor aware already (platform::init); said again for
        // this thread so the window's position and the cursor's are physical pixels
        // whatever ran first.
        let _ = SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        let module = GetModuleHandleW(None).ok()?;
        let instance = HINSTANCE(module.0);
        let class = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(window_proc),
            hInstance: instance,
            // No class cursor: over this window, Windows has no cursor to restore.
            hCursor: HCURSOR::default(),
            lpszClassName: CLASS_NAME,
            ..Default::default()
        };
        RegisterClassExW(&class);
        // Topmost so nothing covers the pixel; a tool window so it is in neither
        // Alt-Tab nor the taskbar; no-activate so neither showing it nor clicking it
        // takes the focus from the window the user was typing in; layered so it can be
        // all but transparent. Not WS_EX_TRANSPARENT: the pointer would then be over
        // the window below, and that window would choose the cursor.
        let window = CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_LAYERED,
            CLASS_NAME,
            PCWSTR::null(),
            WS_POPUP,
            0,
            0,
            1,
            1,
            None,
            None,
            instance,
            None,
        )
        .ok()?;
        // Alpha 1 rather than 0: a layered window that is fully transparent is not hit
        // by the pointer, and would not be asked for its cursor.
        let _ = SetLayeredWindowAttributes(window, COLORREF(0), 1, LWA_ALPHA);
        Some(window.0 as isize)
    }
}

fn pump() {
    let mut msg = MSG::default();
    loop {
        let ret = unsafe { GetMessageW(&mut msg, None, 0, 0) };
        if ret.0 == 0 || ret.0 == -1 {
            break;
        }
        unsafe {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

/// The monitors as [`Screen`]s, or none when Windows reports none.
fn screens() -> Vec<Screen> {
    crate::hook::windows::display::monitors_now()
        .unwrap_or_default()
        .into_iter()
        .map(|r| Screen {
            left: r.left(),
            top: r.top(),
            right: r.right(),
            bottom: r.bottom(),
        })
        .collect()
}

/// Shows or hides the window as [`WANTED`] says. Runs on the window's thread only.
fn reconcile(window: HWND) {
    let wanted = WANTED.load(Ordering::Acquire);
    let shown = SHOWN.load(Ordering::Acquire) != 0;
    unsafe {
        if wanted && !shown {
            let mut cursor = POINT::default();
            if GetCursorPos(&mut cursor).is_err() {
                return;
            }
            let at = (cursor.x, cursor.y);
            let spot = parking_spot(at, &screens()).unwrap_or(at);
            let (x, y, width, height) = hider_rect(spot);
            // In `PARK_ORDER`: the window, then the cursor image blanked, then the
            // pointer onto the window (arriving there is a move over it, which asks it
            // for its cursor). Moving first flashed an arrow in the middle of the
            // screen until this handler returned and `WM_SETCURSOR` was handled.
            let mut previous = HCURSOR::default();
            for step in PARK_ORDER {
                match step {
                    ParkStep::ShowWindow => {
                        if SetWindowPos(
                            window,
                            HWND_TOPMOST,
                            x,
                            y,
                            width,
                            height,
                            SWP_NOACTIVATE | SWP_SHOWWINDOW,
                        )
                        .is_err()
                        {
                            return;
                        }
                        SPOT.store(pack(spot), Ordering::Release);
                        SHOWN.store(window.0 as isize, Ordering::Release);
                    }
                    ParkStep::BlankCursor => {
                        previous = SetCursor(HCURSOR::default());
                    }
                    // SetCursorPos rather than SendInput: exact to the pixel, which a
                    // normalised absolute move is not guaranteed to be, and the window
                    // is one pixel wide. Should it ever reach the low-level hook, it
                    // does so at SPOT, which is not taken for a person.
                    ParkStep::MovePointer => {
                        if SetCursorPos(spot.0, spot.1).is_err() {
                            // The pointer did not reach the window (the input desktop
                            // changed, say): give the image back, take the window
                            // away and forget the parking, so that nothing believes
                            // it is done and a next attempt starts clean.
                            SetCursor(previous);
                            hide(window);
                            return;
                        }
                    }
                }
            }
        } else if !wanted && shown {
            hide(window);
        }
    }
}

/// Takes the window away and forgets where the pointer was parked.
fn hide(window: HWND) {
    SHOWN.store(0, Ordering::Release);
    SPOT.store(u64::MAX, Ordering::Release);
    // Hidden, not moved: an Enter puts the pointer where the master says next, and a
    // person's mouse already took it elsewhere.
    unsafe {
        let _ = SetWindowPos(
            window,
            None,
            0,
            0,
            0,
            0,
            SWP_HIDEWINDOW | SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER,
        );
    }
}

/// This machine's own mouse took the pointer back.
fn give_back(window: HWND) {
    WANTED.store(false, Ordering::Release);
    reconcile(window);
}

unsafe extern "system" fn window_proc(
    window: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let parked = || SHOWN.load(Ordering::Acquire) != 0;
    match msg {
        WM_RECONCILE => {
            reconcile(window);
            LRESULT(0)
        }
        WM_SETCURSOR => {
            unsafe {
                SetCursor(HCURSOR::default());
            }
            LRESULT(1)
        }
        // The pointer reached the window: from now on, leaving it is the sign of a
        // person's mouse. Asked here rather than when parking, because Windows posts
        // the leave at once if the pointer is not over the window yet.
        WM_MOUSEMOVE => {
            if parked() {
                let mut track = TRACKMOUSEEVENT {
                    cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                    dwFlags: TME_LEAVE,
                    hwndTrack: window,
                    dwHoverTime: 0,
                };
                unsafe {
                    let _ = TrackMouseEvent(&mut track);
                }
            }
            LRESULT(0)
        }
        WM_MOUSELEAVE | WM_LBUTTONDOWN | WM_RBUTTONDOWN | WM_MBUTTONDOWN | WM_XBUTTONDOWN
        | WM_MOUSEWHEEL | WM_MOUSEHWHEEL => {
            // Nothing is injected while parked: whatever this is, it is a person here.
            if parked() {
                give_back(window);
            }
            LRESULT(0)
        }
        WM_MOUSEACTIVATE => LRESULT(MA_NOACTIVATE as isize),
        WM_DESTROY => {
            SHOWN.store(0, Ordering::Release);
            unsafe { PostQuitMessage(0) };
            LRESULT(0)
        }
        WM_CLOSE => {
            unsafe {
                let _ = DestroyWindow(window);
            }
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(window, msg, wparam, lparam) },
    }
}
