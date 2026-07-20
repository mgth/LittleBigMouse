//! Win32 hooking + message pump — port of the C++ `Hooker`.
//!
//! The `Hooker` owns the raw Win32 handles and lives entirely on the pump thread
//! (`HHOOK`/`HWND`/`HWINEVENTHOOK` are `!Send`, so the borrow checker forbids
//! installing the hook off that thread). `run` is the C++ `Hooker::Loop`:
//! reconcile the installed hooks to the desired state, pump messages until a
//! `WM_BREAK_LOOP` (re-reconcile) or `WM_QUIT` (stop).

pub mod display;
pub mod mouse;
pub mod win_events;

use std::sync::atomic::Ordering;

use windows::Win32::Foundation::{HINSTANCE, HMODULE, HWND, LPARAM, POINT, WPARAM};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Accessibility::{SetWinEventHook, UnhookWinEvent, HWINEVENTHOOK};
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetCursorPos, GetMessageW, PostThreadMessageW, SetWindowsHookExW,
    TranslateMessage, UnhookWindowsHookEx, EVENT_OBJECT_FOCUS, EVENT_SYSTEM_DESKTOPSWITCH, HHOOK,
    MSG, WH_MOUSE_LL, WINEVENT_OUTOFCONTEXT, WM_APP, WM_QUIT,
};

use crate::ipc::protocol;
use crate::platform;
use crate::priority::Priority;
use crate::shared::Shared;

/// Custom message that unwinds the pump so `run` re-reconciles the hooks
/// (C++ `WM_BREAK_LOOP = WM_APP + 1`).
const WM_BREAK_LOOP: u32 = WM_APP + 1;

/// Record the pump thread id before the IPC server can accept a command that
/// would post to it. Must be called from the thread that will call [`run`].
pub fn register_main_thread(shared: &Shared) {
    shared
        .pump_tid
        .store(unsafe { GetCurrentThreadId() }, Ordering::SeqCst);
}

/// Run the hook install/uninstall + message pump loop on this thread. Returns
/// when a `Quit` command posts WM_QUIT.
pub fn run(shared: &'static Shared) {
    let mut hooker = Hooker::new();
    hooker.run(shared);
}

fn cursor_pos() -> (i32, i32) {
    let mut p = POINT::default();
    unsafe {
        let _ = GetCursorPos(&mut p);
    }
    (p.x, p.y)
}

/// Recover from Windows silently dropping our low-level mouse hook.
///
/// A `WH_MOUSE_LL` callback that ever exceeds `LowLevelHooksTimeout` (~300 ms) is removed by the OS
/// with no notification: the `HHOOK` stays non-null, `hooked` stays true, but the callback is never
/// called again — crossing dies until a full restart (which is exactly what was observed: no
/// `Stopped`, `moves` frozen while `hooked=true`). This watchdog notices the cursor moving while our
/// hook counts nothing and forces a reinstall, so it heals on its own.
pub fn spawn_watchdog(shared: &'static Shared) {
    std::thread::spawn(move || {
        let mut last_moves = crate::hook::MOUSE_EVENTS.load(Ordering::Relaxed);
        let mut last_pos = cursor_pos();
        loop {
            std::thread::sleep(std::time::Duration::from_millis(1500));

            let moves = crate::hook::MOUSE_EVENTS.load(Ordering::Relaxed);
            let pos = cursor_pos();
            let hooked = shared.hooked.load(Ordering::SeqCst);
            let want = shared.want_hook.load(Ordering::SeqCst);

            // We believe we are hooked and want to be, the mouse physically moved since the last
            // check, yet our hook saw nothing: the OS silently dropped it (LowLevelHooksTimeout).
            // Reinstall on the pump thread so crossing heals instead of staying dead until a restart.
            if hooked && want && pos != last_pos && moves == last_moves {
                eprintln!(
                    "[LittleBigMouse.Hook] watchdog: reinstalling silently-dropped mouse hook"
                );
                request_hook(shared);
            }

            last_moves = moves;
            last_pos = pos;
        }
    });
}

pub struct Hooker {
    mouse_hook: HHOOK,
    focus_hook: HWINEVENTHOOK,
    desktop_hook: HWINEVENTHOOK,
    hwnd: HWND,
}

impl Default for Hooker {
    fn default() -> Self {
        Self::new()
    }
}

impl Hooker {
    pub fn new() -> Self {
        Hooker {
            mouse_hook: HHOOK::default(),
            focus_hook: HWINEVENTHOOK::default(),
            desktop_hook: HWINEVENTHOOK::default(),
            hwnd: HWND::default(),
        }
    }

    /// C++ `Hooker::Loop`.
    pub fn run(&mut self, shared: &'static Shared) {
        register_main_thread(shared);

        let mut stopping = false;
        while !stopping {
            self.do_hook(shared);
            stopping = !pump_messages();
            self.do_unhook(shared);
        }
    }

    /// C++ `Hooker::DoHook`: install the mouse hook only when hooking is desired;
    /// always install the focus/desktop/display hooks.
    fn do_hook(&mut self, shared: &Shared) {
        if shared.want_hook.load(Ordering::SeqCst) {
            platform::set_process_priority(Priority::from_u8(
                shared.priority.load(Ordering::SeqCst),
            ));
            self.hook_mouse(shared);
        } else {
            platform::set_process_priority(Priority::from_u8(
                shared.priority_unhooked.load(Ordering::SeqCst),
            ));
        }

        self.hook_focus();
        self.hook_desktop();
        self.hwnd = display::create_window();
    }

    /// C++ `Hooker::DoUnhook`.
    fn do_unhook(&mut self, shared: &Shared) {
        self.unhook_mouse(shared);
        self.unhook_focus();
        self.unhook_desktop();
        self.unhook_display();
        platform::set_process_priority(Priority::from_u8(
            shared.priority_unhooked.load(Ordering::SeqCst),
        ));
    }

    /// C++ `Hooker::HookMouse`.
    fn hook_mouse(&mut self, shared: &Shared) {
        match unsafe {
            SetWindowsHookExW(
                WH_MOUSE_LL,
                Some(mouse::mouse_proc),
                HINSTANCE::default(),
                0,
            )
        } {
            Ok(h) => {
                self.mouse_hook = h;
                shared.hooked.store(true, Ordering::SeqCst);
                shared.broadcast(protocol::RUNNING);
            }
            Err(_) => {
                self.mouse_hook = HHOOK::default();
                shared.hooked.store(false, Ordering::SeqCst);
                shared.broadcast(protocol::STOPPED);
            }
        }
    }

    /// C++ `Hooker::UnhookMouse`.
    fn unhook_mouse(&mut self, shared: &Shared) {
        if self.mouse_hook == HHOOK::default() {
            return;
        }
        unsafe {
            let _ = UnhookWindowsHookEx(self.mouse_hook);
        }
        self.mouse_hook = HHOOK::default();

        // C++ feeds a final `running = false` move so the engine restores any clip
        // it set. Safe to block-lock here: the callback only runs while pumping,
        // and we are between pump cycles. Recover from a poisoned lock (prior panic under it).
        {
            let mut engine = shared.engine.lock().unwrap_or_else(|p| p.into_inner());
            let mut env = crate::platform::cursor::Win32Cursor;
            let mut e = crate::engine::event::MouseEventArg::new(crate::geometry::Point::default());
            e.running = false;
            engine.on_mouse_move(&mut env, &mut e);
        }
        // Safety net: never leave the cursor confined once we stop managing it.
        crate::platform::cursor::release_clip();

        shared.hooked.store(false, Ordering::SeqCst);
        shared.broadcast(protocol::STOPPED);
    }

    fn hook_focus(&mut self) {
        self.focus_hook = unsafe {
            SetWinEventHook(
                EVENT_OBJECT_FOCUS,
                EVENT_OBJECT_FOCUS,
                HMODULE::default(),
                Some(win_events::focus_proc),
                0,
                0,
                WINEVENT_OUTOFCONTEXT,
            )
        };
    }

    fn unhook_focus(&mut self) {
        if self.focus_hook != HWINEVENTHOOK::default() {
            unsafe {
                let _ = UnhookWinEvent(self.focus_hook);
            }
            self.focus_hook = HWINEVENTHOOK::default();
        }
    }

    fn hook_desktop(&mut self) {
        self.desktop_hook = unsafe {
            SetWinEventHook(
                EVENT_SYSTEM_DESKTOPSWITCH,
                EVENT_SYSTEM_DESKTOPSWITCH,
                HMODULE::default(),
                Some(win_events::desktop_proc),
                0,
                0,
                WINEVENT_OUTOFCONTEXT,
            )
        };
    }

    fn unhook_desktop(&mut self) {
        if self.desktop_hook != HWINEVENTHOOK::default() {
            unsafe {
                let _ = UnhookWinEvent(self.desktop_hook);
            }
            self.desktop_hook = HWINEVENTHOOK::default();
        }
    }

    fn unhook_display(&mut self) {
        display::destroy_window(self.hwnd);
        self.hwnd = HWND::default();
    }
}

/// C++ `Hooker::PumpMessages`. Returns `true` to re-reconcile (WM_BREAK_LOOP),
/// `false` to stop (WM_QUIT or error).
fn pump_messages() -> bool {
    let mut msg = MSG::default();
    loop {
        let ret = unsafe { GetMessageW(&mut msg, None, 0, 0) };
        match ret.0 {
            -1 => return false, // error
            0 => return false,  // WM_QUIT retrieved
            _ => {
                if msg.message == WM_BREAK_LOOP {
                    return true;
                }
                unsafe {
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            }
        }
    }
}

/// Ask the pump to install the mouse hook (C++ `Hooker::Hook`).
pub fn request_hook(shared: &Shared) {
    shared.want_hook.store(true, Ordering::SeqCst);
    break_loop(shared);
}

/// Ask the pump to remove the mouse hook (C++ `Hooker::Unhook`).
pub fn request_unhook(shared: &Shared) {
    shared.want_hook.store(false, Ordering::SeqCst);
    break_loop(shared);
}

/// Ask the pump to exit (C++ `Hooker::Quit`).
pub fn request_quit(shared: &Shared) {
    post(shared, WM_QUIT);
}

fn break_loop(shared: &Shared) {
    post(shared, WM_BREAK_LOOP);
}

fn post(shared: &Shared, message: u32) {
    let tid = shared.pump_tid.load(Ordering::SeqCst);
    if tid != 0 {
        unsafe {
            let _ = PostThreadMessageW(tid, message, WPARAM(0), LPARAM(0));
        }
    }
}
