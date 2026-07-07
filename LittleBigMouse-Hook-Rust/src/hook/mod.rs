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

use windows::Win32::Foundation::{HINSTANCE, HMODULE, HWND, LPARAM, WPARAM};
use windows::Win32::System::Threading::{
    GetCurrentProcess, GetCurrentThreadId, SetPriorityClass, ABOVE_NORMAL_PRIORITY_CLASS,
    BELOW_NORMAL_PRIORITY_CLASS, HIGH_PRIORITY_CLASS, IDLE_PRIORITY_CLASS, NORMAL_PRIORITY_CLASS,
    REALTIME_PRIORITY_CLASS,
};
use windows::Win32::UI::Accessibility::{SetWinEventHook, UnhookWinEvent, HWINEVENTHOOK};
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetMessageW, PostThreadMessageW, SetWindowsHookExW, TranslateMessage,
    UnhookWindowsHookEx, EVENT_OBJECT_FOCUS, EVENT_SYSTEM_DESKTOPSWITCH, HHOOK, MSG, WH_MOUSE_LL,
    WINEVENT_OUTOFCONTEXT, WM_APP, WM_QUIT,
};

use crate::ipc::protocol;
use crate::priority::Priority;
use crate::shared::Shared;

/// Custom message that unwinds the pump so `run` re-reconciles the hooks
/// (C++ `WM_BREAK_LOOP = WM_APP + 1`).
const WM_BREAK_LOOP: u32 = WM_APP + 1;

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
        shared
            .pump_tid
            .store(unsafe { GetCurrentThreadId() }, Ordering::SeqCst);

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
            set_priority(Priority::from_u8(shared.priority.load(Ordering::SeqCst)));
            self.hook_mouse(shared);
        } else {
            set_priority(Priority::from_u8(shared.priority_unhooked.load(Ordering::SeqCst)));
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
        set_priority(Priority::from_u8(shared.priority_unhooked.load(Ordering::SeqCst)));
    }

    /// C++ `Hooker::HookMouse`.
    fn hook_mouse(&mut self, shared: &Shared) {
        match unsafe {
            SetWindowsHookExW(WH_MOUSE_LL, Some(mouse::mouse_proc), HINSTANCE::default(), 0)
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
        // and we are between pump cycles.
        if let Ok(mut engine) = shared.engine.lock() {
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

fn set_priority(priority: Priority) {
    let class = match priority {
        Priority::Idle => IDLE_PRIORITY_CLASS,
        Priority::Below => BELOW_NORMAL_PRIORITY_CLASS,
        Priority::Normal => NORMAL_PRIORITY_CLASS,
        Priority::Above => ABOVE_NORMAL_PRIORITY_CLASS,
        Priority::High => HIGH_PRIORITY_CLASS,
        Priority::Realtime => REALTIME_PRIORITY_CLASS,
    };
    unsafe {
        let _ = SetPriorityClass(GetCurrentProcess(), class);
    }
}

// --- Event actions, shared by the callbacks (C++ LittleBigMouseDaemon) --------

/// A display was added/removed/reconfigured: stop hooking and tell the UI to
/// recompute and reload the layout (C++ `LittleBigMouseDaemon::DisplayChanged`).
pub(crate) fn on_display_changed(shared: &Shared) {
    if shared.hooked.load(Ordering::SeqCst) {
        request_unhook(shared);
    }
    shared.broadcast(protocol::DISPLAY_CHANGED);
}

/// The work area changed (C++ `SettingChanged`).
pub(crate) fn on_setting_changed(shared: &Shared) {
    if shared.hooked.load(Ordering::SeqCst) {
        request_unhook(shared);
    }
    shared.broadcast(protocol::SETTING_CHANGED);
}

/// The system switched to/from the secure (UAC) desktop (C++ `DesktopChanged`).
pub(crate) fn on_desktop_changed(shared: &Shared) {
    shared.broadcast(protocol::DESKTOP_CHANGED);
}

/// The foreground window changed (C++ `FocusChanged`). Phase 4 adds the
/// exclusion-based pause/resume; for now this only notifies the UI.
pub(crate) fn on_focus_changed(shared: &Shared, path: String) {
    shared.broadcast(&protocol::focus_changed(&path));
}

/// Run a callback body catching any panic, so it can never unwind across the
/// `extern "system"` FFI boundary (which would be UB).
pub(crate) fn guard<F: FnOnce()>(body: F) {
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(body));
}
