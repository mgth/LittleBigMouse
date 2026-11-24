//! Telling the agent when Windows drops what this hook injects (UIPI): the decision is
//! [`crate::remote::blocking`]'s, the Windows side of it is here.
//!
//! The foreground is followed by event (`EVENT_SYSTEM_FOREGROUND`), and only while a
//! master drives this machine: [`begin`] reads it once and installs the hook, [`end`]
//! takes it away. Nothing runs between sessions, and nothing is polled. The event hook
//! is out of context, so its callback runs on the thread that installed it, which must
//! pump messages: a thread of its own, made at the first session and kept for the
//! process's life, idle in `GetMessageW` the rest of the time.
//!
//! Between sessions the last word stands: the agent keeps a block said during a session
//! until the next one begins and says otherwise.

use std::sync::{Mutex, OnceLock};

use lbm_ipc::peer::InjectionBlock;
use lbm_ipc::protocol::Event;
use windows::Win32::Foundation::{CloseHandle, HANDLE, HMODULE, HWND, LPARAM, WPARAM};
use windows::Win32::Security::{
    GetSidSubAuthority, GetSidSubAuthorityCount, GetTokenInformation, TokenIntegrityLevel,
    TokenUIAccess, TOKEN_MANDATORY_LABEL, TOKEN_QUERY,
};
use windows::Win32::System::Threading::{
    GetCurrentProcess, GetCurrentThreadId, OpenProcess, OpenProcessToken,
    PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::Accessibility::{SetWinEventHook, UnhookWinEvent, HWINEVENTHOOK};
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetForegroundWindow, GetMessageW, GetWindowThreadProcessId, PeekMessageW,
    PostThreadMessageW, TranslateMessage, EVENT_SYSTEM_FOREGROUND, MSG, PM_NOREMOVE,
    WINEVENT_OUTOFCONTEXT, WINEVENT_SKIPOWNPROCESS, WM_APP,
};

use crate::remote::blocking::{self, Foreground, Integrity, Reporter};
use crate::shared::Shared;

/// Installs the event hook on the watcher's thread.
const WM_WATCH: u32 = WM_APP + 0x50;
/// Takes it away.
const WM_UNWATCH: u32 = WM_APP + 0x51;

static SHARED: OnceLock<&'static Shared> = OnceLock::new();
static REPORTER: Mutex<Reporter> = Mutex::new(Reporter::new());
/// The watcher's thread id; `None` when it could not be made, and then only the
/// session's start is checked.
static WATCHER: OnceLock<Option<u32>> = OnceLock::new();

/// A driven session begins: the foreground is read now, the agent told if the session
/// begins blocked (or no longer is), and the foreground followed until [`end`].
pub fn begin(shared: &'static Shared) {
    let _ = SHARED.set(shared);
    let now = verdict(None);
    say(lock().begin(now));
    if let Some(thread) = *WATCHER.get_or_init(spawn_watcher) {
        // SAFETY: posting to a thread of this process; a failure leaves it unwatched.
        let _ = unsafe { PostThreadMessageW(thread, WM_WATCH, WPARAM(0), LPARAM(0)) };
    }
}

/// The driven session ended: the foreground is no longer followed.
pub fn end() {
    if let Some(Some(thread)) = WATCHER.get() {
        // SAFETY: as in `begin`.
        let _ = unsafe { PostThreadMessageW(*thread, WM_UNWATCH, WPARAM(0), LPARAM(0)) };
    }
}

/// `SendInput` put fewer events into the system than it was handed.
pub fn short() {
    // Already said: nothing to read again, at a thousand batches a second.
    if lock().said().is_some() {
        return;
    }
    let process = match foreground(None) {
        Foreground::Known { process, .. } | Foreground::Unreadable { process } => process,
        Foreground::Nobody => String::new(),
    };
    say(lock().short(process));
}

fn lock() -> std::sync::MutexGuard<'static, Reporter> {
    REPORTER.lock().unwrap_or_else(|p| p.into_inner())
}

fn say(word: Option<Option<InjectionBlock>>) {
    let Some(block) = word else { return };
    match &block {
        Some(block) => eprintln!(
            "[LittleBigMouse.Hook] injection blocked: {} has the focus at a higher \
             integrity level than this hook",
            if block.process.is_empty() {
                "a window"
            } else {
                &block.process
            }
        ),
        None => eprintln!("[LittleBigMouse.Hook] injection no longer blocked"),
    }
    if let Some(shared) = SHARED.get() {
        shared.broadcast(&Event::InjectionBlocked { block });
    }
}

/// What the window `hwnd` (the foreground when `None`) lets this hook inject.
fn verdict(hwnd: Option<HWND>) -> Option<InjectionBlock> {
    static OURS: OnceLock<Option<Integrity>> = OnceLock::new();
    // SAFETY: the pseudo-handle of this process needs no closing.
    let ours = *OURS.get_or_init(|| token_integrity(unsafe { GetCurrentProcess() }));
    blocking::blocked(ours, &foreground(hwnd))
}

fn foreground(hwnd: Option<HWND>) -> Foreground {
    // SAFETY: no pointer.
    let hwnd = hwnd.unwrap_or_else(|| unsafe { GetForegroundWindow() });
    if hwnd.is_invalid() {
        return Foreground::Nobody;
    }
    let mut pid = 0u32;
    // SAFETY: `pid` outlives the call.
    unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
    if pid == 0 {
        return Foreground::Nobody;
    }
    let process = crate::platform::windows::process::exe_path_from_window(hwnd)
        .map(|path| blocking::file_name(&path))
        .unwrap_or_default();
    // LIMITED is granted on elevated processes, and is what OpenProcessToken needs.
    // SAFETY: the handle is closed below.
    let Ok(handle) = (unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }) else {
        return Foreground::Unreadable { process };
    };
    let integrity = token_integrity(handle);
    // SAFETY: opened above.
    unsafe {
        let _ = CloseHandle(handle);
    }
    match integrity {
        Some(integrity) => Foreground::Known { process, integrity },
        None => Foreground::Unreadable { process },
    }
}

/// The integrity level and UIAccess flag of `process`'s token.
fn token_integrity(process: HANDLE) -> Option<Integrity> {
    let mut token = HANDLE::default();
    // SAFETY: `token` is closed below.
    unsafe { OpenProcessToken(process, TOKEN_QUERY, &mut token) }.ok()?;
    let integrity = read_integrity(token);
    // SAFETY: opened above.
    unsafe {
        let _ = CloseHandle(token);
    }
    integrity
}

fn read_integrity(token: HANDLE) -> Option<Integrity> {
    // A TOKEN_MANDATORY_LABEL and the SID it points to, which follows it in the buffer:
    // a few dozen bytes. u64s for the alignment of the pointer it starts with.
    let mut buffer = [0u64; 16];
    let mut length = 0u32;
    // SAFETY: the buffer is as long as said; Windows writes at most that.
    unsafe {
        GetTokenInformation(
            token,
            TokenIntegrityLevel,
            Some(buffer.as_mut_ptr().cast()),
            std::mem::size_of_val(&buffer) as u32,
            &mut length,
        )
    }
    .ok()?;
    // SAFETY: Windows filled a TOKEN_MANDATORY_LABEL whose SID lives in `buffer`.
    let level = unsafe {
        let label = &*(buffer.as_ptr().cast::<TOKEN_MANDATORY_LABEL>());
        let sid = label.Label.Sid;
        let count = *GetSidSubAuthorityCount(sid);
        if count == 0 {
            return None;
        }
        *GetSidSubAuthority(sid, u32::from(count) - 1)
    };
    let mut ui_access = 0u32;
    // SAFETY: a u32 is what TokenUIAccess writes.
    let ui_access = unsafe {
        GetTokenInformation(
            token,
            TokenUIAccess,
            Some((&mut ui_access as *mut u32).cast()),
            4,
            &mut length,
        )
    }
    .is_ok()
        && ui_access != 0;
    Some(Integrity { level, ui_access })
}

/// The thread the event hook lives on.
fn spawn_watcher() -> Option<u32> {
    let (tx, rx) = std::sync::mpsc::channel();
    let spawned = std::thread::Builder::new()
        .name("lbm-foreground".into())
        .spawn(move || {
            // The thread's message queue is made by its first call that reads one: made
            // before the id goes out, so that nothing posted to it is lost.
            let mut msg = MSG::default();
            // SAFETY: `msg` outlives the call; nothing is removed.
            let _ = unsafe { PeekMessageW(&mut msg, None, 0, 0, PM_NOREMOVE) };
            // SAFETY: no pointer.
            let _ = tx.send(unsafe { GetCurrentThreadId() });
            pump();
        });
    if let Err(error) = spawned {
        eprintln!("[LittleBigMouse.Hook] no foreground watcher: {error}");
        return None;
    }
    rx.recv().ok()
}

fn pump() {
    let mut hook = HWINEVENTHOOK::default();
    let mut msg = MSG::default();
    // SAFETY: a plain message loop on this thread's own queue.
    while unsafe { GetMessageW(&mut msg, None, 0, 0) }.0 > 0 {
        match msg.message {
            WM_WATCH if hook.is_invalid() => {
                // SAFETY: the callback is a plain function; out of context, so it runs
                // here, in this loop.
                hook = unsafe {
                    SetWinEventHook(
                        EVENT_SYSTEM_FOREGROUND,
                        EVENT_SYSTEM_FOREGROUND,
                        HMODULE::default(),
                        Some(foreground_proc),
                        0,
                        0,
                        WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
                    )
                };
                // The foreground may have changed between `begin`'s reading and now.
                say(lock().changed(verdict(None)));
            }
            WM_UNWATCH if !hook.is_invalid() => {
                // SAFETY: installed above.
                let _ = unsafe { UnhookWinEvent(hook) };
                hook = HWINEVENTHOOK::default();
            }
            WM_WATCH | WM_UNWATCH => {}
            _ => {
                // SAFETY: the message just read.
                unsafe {
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            }
        }
    }
}

unsafe extern "system" fn foreground_proc(
    _hook: HWINEVENTHOOK,
    _event: u32,
    hwnd: HWND,
    _id_object: i32,
    _id_child: i32,
    _thread: u32,
    _time: u32,
) {
    crate::hook::guard(|| say(lock().changed(verdict(Some(hwnd)))));
}
