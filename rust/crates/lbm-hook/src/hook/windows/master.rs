//! The Windows multi-PC master (docs/multi-pc.md): what the low-level hooks see goes to
//! [`LlMaster`], which applies the crossing rules; this module is its system side.
//!
//! All of it lives on the pump thread, where the hooks run: the mouse hook
//! ([`super::mouse`]) hands its events here first, the keyboard hook is
//! [`keyboard_proc`], and both go to one master, kept in a thread-local. A hook can be
//! entered again while it waits (showing the cursor waits for the window that hides it,
//! and Windows delivers the hooks' events to a thread that waits for a sent message):
//! an event that finds the master busy is left to the system.
//!
//! Nothing runs while there is no machine to drive. The input endpoint says when its
//! client changes ([`crate::ipc::input::Outlet::on_change`]), and only then is the
//! keyboard hook installed, the window that hides the cursor made, and the moves looked
//! at by the master. While the pointer is away, a timer of the pump thread lets a rescue
//! held without autorepeat fire, and a client that went be noticed without a move.
//!
//! What a low-level hook cannot do:
//!
//! * Ctrl+Alt+Del is not seen by any hook: the secure desktop takes it, and the
//!   releases of Ctrl and Alt with it. The desktop switch brings the pointer home
//!   ([`desktop_switched`]), whose `Leave` releases them over there. Win+L likewise.
//! * A window of a higher integrity level in the foreground gets its input without the
//!   low-level hooks of a process below it seeing it (UIPI). See docs/multi-pc-etat.md.

use std::cell::{Cell, RefCell};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::System::SystemInformation::GetTickCount64;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, GetKeyState, VK_CAPITAL, VK_CONTROL, VK_LBUTTON, VK_MBUTTON, VK_NUMLOCK,
    VK_RBUTTON, VK_SCROLL, VK_XBUTTON1, VK_XBUTTON2,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, ClipCursor, GetClipCursor, GetCursorInfo, GetCursorPos, GetSystemMetrics,
    KillTimer, SetCursorPos, SetTimer, SetWindowsHookExW, UnhookWindowsHookEx, CURSORINFO, HHOOK,
    KBDLLHOOKSTRUCT, LLKHF_EXTENDED, MSLLHOOKSTRUCT, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN,
    SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN, WH_KEYBOARD_LL, WM_KEYDOWN, WM_LBUTTONDOWN, WM_LBUTTONUP,
    WM_MBUTTONDOWN, WM_MBUTTONUP, WM_MOUSEHWHEEL, WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_RBUTTONDOWN,
    WM_RBUTTONUP, WM_SYSKEYDOWN, WM_XBUTTONDOWN, WM_XBUTTONUP, XBUTTON1,
};

use lbm_ipc::input::Locks;

use crate::geometry::{Point, Rect};
use crate::hook::crossing::keys::key_of_virtual_key;
use crate::hook::crossing::Press;
use crate::hook::ll_master::{Desk, LlMaster, Verdict};
use crate::ipc::input::Driver;
use crate::remote::keymap::linux_key;
use crate::remote::windows::hider::{self, Hider};
use crate::shared::{Shared, SHARED};

const BTN_LEFT: u16 = 0x110;
const BTN_RIGHT: u16 = 0x111;
const BTN_MIDDLE: u16 = 0x112;
const BTN_SIDE: u16 = 0x113;
const BTN_EXTRA: u16 = 0x114;

/// How often the master looks at the time while the pointer is away and nothing
/// happens.
const TICK_MS: u32 = 100;

/// The system side of the master.
pub(crate) struct WinDesk {
    hider: Hider,
    /// The last time the keyboard hook was tried for because it was missing.
    keyboard_tried: Option<Instant>,
}

impl Desk for WinDesk {
    fn cursor(&self) -> Point<i32> {
        let mut p = POINT::default();
        // SAFETY: a valid out pointer.
        if unsafe { GetCursorPos(&mut p) }.is_ok() {
            Point::new(p.x, p.y)
        } else {
            Point::new(0, 0)
        }
    }

    fn set_cursor(&mut self, at: Point<i32>) {
        PLACED.store(pack(at.x(), at.y()), Ordering::Relaxed);
        // SAFETY: no pointer.
        unsafe {
            let _ = SetCursorPos(at.x(), at.y());
        }
    }

    fn clip(&self) -> Rect<i32> {
        let mut r = RECT::default();
        // SAFETY: a valid out pointer.
        if unsafe { GetClipCursor(&mut r) }.is_ok() {
            Rect::new(r.left, r.top, r.right - r.left, r.bottom - r.top)
        } else {
            self.desktop()
        }
    }

    fn set_clip(&mut self, clip: Option<Rect<i32>>) {
        let rect = clip.map(|r| RECT {
            left: r.left(),
            top: r.top(),
            right: r.right(),
            bottom: r.bottom(),
        });
        // SAFETY: a valid rectangle or none.
        unsafe {
            let _ = ClipCursor(rect.as_ref().map(|r| r as *const RECT));
        }
    }

    fn clip_is_subrect_of_virtual_screen(&self) -> bool {
        let clip = self.clip();
        let desktop = self.desktop();
        clip.left() > desktop.left()
            || clip.top() > desktop.top()
            || clip.right() < desktop.right()
            || clip.bottom() < desktop.bottom()
    }

    fn cursor_hidden(&self) -> bool {
        let mut ci = CURSORINFO {
            cbSize: std::mem::size_of::<CURSORINFO>() as u32,
            ..Default::default()
        };
        // SAFETY: a sized out structure. Fail-open, as `Win32Cursor`.
        unsafe { GetCursorInfo(&mut ci) }.is_ok() && ci.flags.0 == 0
    }

    fn ctrl_down(&self) -> bool {
        down(VK_CONTROL.0)
    }

    fn buttons_down(&self) -> bool {
        [VK_LBUTTON, VK_RBUTTON, VK_MBUTTON, VK_XBUTTON1, VK_XBUTTON2]
            .iter()
            .any(|vk| down(vk.0))
    }

    fn tick_count(&self) -> u64 {
        // SAFETY: no pointer.
        unsafe { GetTickCount64() }
    }

    fn desktop(&self) -> Rect<i32> {
        // SAFETY: no pointer.
        unsafe {
            Rect::new(
                GetSystemMetrics(SM_XVIRTUALSCREEN),
                GetSystemMetrics(SM_YVIRTUALSCREEN),
                GetSystemMetrics(SM_CXVIRTUALSCREEN),
                GetSystemMetrics(SM_CYVIRTUALSCREEN),
            )
        }
    }

    fn locks(&self) -> Locks {
        // The toggle bit. Read on the pump thread, which has no keyboard focus of its
        // own: its view is the system's as of the last input it saw go by.
        // SAFETY: no pointer; any virtual key may be asked.
        let toggled = |vk: u16| Some(unsafe { GetKeyState(i32::from(vk)) } & 1 != 0);
        let locks = Locks {
            caps: toggled(VK_CAPITAL.0),
            num: toggled(VK_NUMLOCK.0),
            scroll: toggled(VK_SCROLL.0),
        };
        // Read once per crossing out: said, since nothing else shows what went over.
        eprintln!("[LittleBigMouse.Hook] multi-PC: the lock keys here: {locks:?}");
        locks
    }

    fn hide_cursor(&mut self) {
        self.hider.park();
    }

    fn hiding_spot(&self) -> Option<Point<i32>> {
        hider::parked_at().map(|(x, y)| Point::new(x, y))
    }

    fn show_cursor(&mut self) {
        self.hider.unpark();
    }

    fn key_down(&self, vk: u16) -> bool {
        down(vk)
    }

    /// The keyboard hook is there; when it is not, it is tried again, at most once a
    /// second, from here: the routing thread is the one that owns it.
    fn can_drive(&mut self) -> bool {
        if keyboard_hooked() {
            return true;
        }
        let now = Instant::now();
        if self
            .keyboard_tried
            .is_some_and(|at| now.duration_since(at) < KEYBOARD_RETRY)
        {
            return false;
        }
        self.keyboard_tried = Some(now);
        set_keyboard_hook(true);
        keyboard_hooked()
    }
}

/// How long the master waits between two tries at the keyboard hook.
const KEYBOARD_RETRY: std::time::Duration = std::time::Duration::from_secs(1);

fn keyboard_hooked() -> bool {
    !KEYBOARD.with(Cell::get).0.is_null()
}

fn down(vk: u16) -> bool {
    // SAFETY: no pointer.
    (unsafe { GetAsyncKeyState(i32::from(vk)) } as u16 & 0x8000) != 0
}

type Master = LlMaster<WinDesk, Driver>;

/// Where the master last put the cursor itself, packed; `u64::MAX` before it ever did.
static PLACED: AtomicU64 = AtomicU64::new(u64::MAX);

fn pack(x: i32, y: i32) -> u64 {
    (u64::from(x as u32) << 32) | u64::from(y as u32)
}

/// Whether the cursor at `(x, y)` is where the master put it — on what hides it, or
/// back home — rather than where a mouse took it. The hook's watchdog
/// (`super::spawn_watchdog`) reinstalls a mouse hook Windows dropped when the cursor
/// moved with no move counted: a move of the master's own is no such sign, a mouse
/// taking the cursor off the parking spot while the pointer is away is one.
pub(crate) fn placed_by_master(x: i32, y: i32) -> bool {
    PLACED.load(Ordering::Relaxed) == pack(x, y) || hider::parked_at() == Some((x, y))
}

thread_local! {
    static MASTER: RefCell<Option<Master>> = const { RefCell::new(None) };
    static KEYBOARD: Cell<HHOOK> = const { Cell::new(HHOOK(std::ptr::null_mut())) };
    static TIMER: Cell<usize> = const { Cell::new(0) };
    /// The foreground window last judged while the pointer is away.
    static JUDGED: Cell<isize> = const { Cell::new(0) };
}

/// Runs `f` on the master, made at the first call. `None` when it is busy: the event
/// is left to the system.
fn with<R>(f: impl FnOnce(&mut Master) -> R) -> Option<R> {
    let r = MASTER.with(|cell| {
        let mut slot = cell.try_borrow_mut().ok()?;
        let master = slot.get_or_insert_with(|| {
            LlMaster::new(
                WinDesk {
                    hider: Hider::default(),
                    keyboard_tried: None,
                },
                Driver::default(),
            )
        });
        Some(f(master))
    });
    sync_timer();
    r
}

/// Asks the pump to look at the client again when it changes. Call on the pump thread,
/// before the endpoint starts accepting.
pub(crate) fn listen(shared: &'static Shared) {
    shared.outlet.on_change(|| {
        if let Some(shared) = SHARED.get() {
            super::post(shared, super::WM_MASTER_CLIENT);
        }
    });
}

/// The input endpoint's client changed: the keyboard hook and the window that hides
/// the cursor are there while a machine may be driven, and nowhere else.
pub(crate) fn client_changed(shared: &'static Shared) {
    let wanted = with(|master| {
        let wanted = shared.hooked.load(Ordering::SeqCst) && master.wants_keyboard(shared);
        if wanted {
            master.desk.hider.prepare();
        }
        // A client that went, while the pointer is away: home now, not at the next move.
        master.on_tick(shared);
        wanted
    })
    .unwrap_or(false);
    set_keyboard_hook(wanted);
}

fn set_keyboard_hook(wanted: bool) {
    let hook = KEYBOARD.with(Cell::get);
    let installed = !hook.0.is_null();
    if wanted && !installed {
        // SAFETY: a hook procedure of this process, on this thread, which pumps.
        match unsafe {
            SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_proc), HINSTANCE::default(), 0)
        } {
            Ok(hook) => {
                eprintln!("[LittleBigMouse.Hook] multi-PC: the keyboard hook is installed");
                KEYBOARD.with(|k| k.set(hook));
            }
            Err(e) => eprintln!(
                "[LittleBigMouse.Hook] multi-PC: no keyboard hook ({e}): the other machines' borders stay walls"
            ),
        }
    } else if !wanted && installed {
        // SAFETY: the hook this thread installed.
        unsafe {
            let _ = UnhookWindowsHookEx(hook);
        }
        KEYBOARD.with(|k| k.set(HHOOK(std::ptr::null_mut())));
        eprintln!("[LittleBigMouse.Hook] multi-PC: the keyboard hook is removed");
    }
}

/// The pointer away has a timer; the pointer here has none.
fn sync_timer() {
    let away = MASTER.with(|cell| {
        cell.try_borrow()
            .ok()
            .and_then(|m| m.as_ref().map(LlMaster::is_remote))
    });
    let Some(away) = away else { return };
    let timer = TIMER.with(Cell::get);
    if away && timer == 0 {
        // The foreground the pointer left from is judged at the first tick.
        JUDGED.with(|j| j.set(0));
        // SAFETY: a thread timer of the pump thread, delivered by its message loop.
        let id = unsafe { SetTimer(HWND::default(), 0, TICK_MS, Some(timer_proc)) };
        TIMER.with(|t| t.set(id));
    } else if !away && timer != 0 {
        // SAFETY: the timer this thread set.
        unsafe {
            let _ = KillTimer(HWND::default(), timer);
        }
        TIMER.with(|t| t.set(0));
    }
}

unsafe extern "system" fn timer_proc(_hwnd: HWND, _msg: u32, _id: usize, _time: u32) {
    crate::hook::guard(|| {
        if let Some(shared) = SHARED.get() {
            let blocked = elevated_foreground();
            with(|master| {
                if blocked {
                    master.shut_down();
                } else {
                    master.on_tick(shared);
                }
            });
        }
    });
}

/// Whether a window above this hook has just taken the focus. Its input never reaches
/// the low-level hooks: the mouse would move this machine's cursor out from under what
/// hides it and the keys would type there, while the other machine waits, driven. The
/// pointer comes home instead (tried on the vm124: with an elevated window in the
/// foreground, a hook that is not elevated sees neither a move nor a key). Judged only
/// when the foreground changes: one `GetForegroundWindow` per tick otherwise.
fn elevated_foreground() -> bool {
    use crate::remote::windows::foreground;
    let id = foreground::foreground_id();
    if JUDGED.with(Cell::get) == id {
        return false;
    }
    JUDGED.with(|j| j.set(id));
    match foreground::above_us(id) {
        Some(block) => {
            eprintln!(
                "[LittleBigMouse.Hook] multi-PC: {} has the focus above this hook, which no \
                 longer sees the mouse and keyboard: the pointer comes home",
                if block.process.is_empty() {
                    "a window"
                } else {
                    &block.process
                }
            );
            true
        }
        None => false,
    }
}

/// The desktop switched — the secure desktop, the lock screen, or back: the pointer
/// comes home.
pub(crate) fn desktop_switched() {
    with(|master| master.shut_down());
}

/// The mouse hook goes away: the pointer comes home while it can still say so, and the
/// keyboard is this machine's again.
pub(crate) fn unhooked() {
    with(|master| master.shut_down());
    set_keyboard_hook(false);
}

/// One event of the mouse hook, while the pointer is away: `Some(swallow)` when the
/// master took it. Moves are not deduplicated here: the cursor is frozen, so two equal
/// reports are two equal displacements.
pub(super) fn away_event(
    shared: &'static Shared,
    message: u32,
    ms: &MSLLHOOKSTRUCT,
) -> Option<bool> {
    with(|master| {
        master.active(shared);
        if !master.is_remote() {
            return None;
        }
        let verdict = match message {
            WM_MOUSEMOVE => {
                // Seen, for the watchdog: the hook is alive.
                crate::hook::hot_path::count_event();
                master.on_move(shared, Point::new(ms.pt.x, ms.pt.y))
            }
            _ => {
                let (input, wheel) = pointer_input(message, ms.mouseData)?;
                match input {
                    Some((code, press)) => master.on_button(shared, code, press),
                    None => master.on_wheel(shared, wheel.0, wheel.1),
                }
            }
        };
        verdict.map(|v| v == Verdict::Swallow)
    })
    .flatten()
}

/// A deduplicated move while the pointer is here: `Some(swallow)` when the master
/// routed it, `None` when there is no machine to drive and the hook routes it as it
/// always did.
pub(super) fn local_move(shared: &'static Shared, report: (i32, i32)) -> Option<bool> {
    with(|master| master.on_move(shared, Point::new(report.0, report.1)))
        .flatten()
        .map(|v| v == Verdict::Swallow)
}

/// A button (Linux code and press) or a wheel turn (horizontal, vertical, in 120ths).
#[allow(clippy::type_complexity)]
fn pointer_input(message: u32, data: u32) -> Option<(Option<(u16, Press)>, (i32, i32))> {
    let high = (data >> 16) as u16;
    let (code, press) = match message {
        WM_LBUTTONDOWN => (BTN_LEFT, Press::Down),
        WM_LBUTTONUP => (BTN_LEFT, Press::Up),
        WM_RBUTTONDOWN => (BTN_RIGHT, Press::Down),
        WM_RBUTTONUP => (BTN_RIGHT, Press::Up),
        WM_MBUTTONDOWN => (BTN_MIDDLE, Press::Down),
        WM_MBUTTONUP => (BTN_MIDDLE, Press::Up),
        WM_XBUTTONDOWN | WM_XBUTTONUP => {
            let code = if high == XBUTTON1 {
                BTN_SIDE
            } else {
                BTN_EXTRA
            };
            let press = if message == WM_XBUTTONDOWN {
                Press::Down
            } else {
                Press::Up
            };
            (code, press)
        }
        // The high word is the signed travel in 120ths: away from the user, to the right.
        WM_MOUSEWHEEL => return Some((None, (0, i32::from(high as i16)))),
        WM_MOUSEHWHEEL => return Some((None, (i32::from(high as i16), 0))),
        _ => return None,
    };
    Some((Some((code, press)), (0, 0)))
}

/// The low-level keyboard hook, installed while a machine may be driven.
///
/// # Safety
/// Invoked by Windows as a `WH_KEYBOARD_LL` hook procedure; never call it directly.
unsafe extern "system" fn keyboard_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let swallow = std::panic::catch_unwind(|| key(code, wparam, lparam)).unwrap_or(false);
    if swallow {
        LRESULT(1)
    } else {
        // SAFETY: the arguments Windows gave.
        unsafe { CallNextHookEx(HHOOK::default(), code, wparam, lparam) }
    }
}

fn key(code: i32, wparam: WPARAM, lparam: LPARAM) -> bool {
    if code < 0 || lparam.0 == 0 {
        return false;
    }
    // SAFETY: for `code >= 0`, `lparam` points to the event's `KBDLLHOOKSTRUCT`.
    let kb = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };
    // What this machine injects as another master's remote is that master's input.
    if kb.dwExtraInfo == crate::remote::windows::INJECTED {
        return false;
    }
    let Some(shared) = SHARED.get() else {
        return false;
    };
    let message = wparam.0 as u32;
    let down = message == WM_KEYDOWN || message == WM_SYSKEYDOWN;
    let extended = kb.flags.0 & LLKHF_EXTENDED.0 != 0;
    let vk = kb.vkCode as u16;
    // A key injected by virtual key alone has no scancode: read by its virtual key.
    let linux = if kb.scanCode == 0 {
        key_of_virtual_key(kb.vkCode)
    } else {
        linux_key(kb.scanCode as u16, extended)
    };
    with(|master| master.on_key(shared, linux, vk, down, Instant::now()) == Verdict::Swallow)
        .unwrap_or(false)
}
