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
//! Each batch injected also compares the foreground window with the one last judged
//! ([`input`]): a net for an event that does not come.
//!
//! Between sessions the last word stands: the agent keeps a block said during a session
//! until the next one begins and says otherwise.

use std::sync::{Mutex, OnceLock};

use lbm_ipc::peer::{BlockCause, InjectionBlock};
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

use crate::remote::blocking::{self, Foreground, Integrity, Watch};
use crate::shared::Shared;

/// Installs the event hook on the watcher's thread.
const WM_WATCH: u32 = WM_APP + 0x50;
/// Takes it away.
const WM_UNWATCH: u32 = WM_APP + 0x51;

static SHARED: OnceLock<&'static Shared> = OnceLock::new();
/// The foreground last judged and what was said. Every observation — the session's
/// start, an event, a batch — reads the foreground, judges it and says the result while
/// holding this lock, so that two of them cannot say what they read out of order.
static WATCH: Mutex<Watch> = Mutex::new(Watch::new());
/// The watcher's thread id; `None` when it could not be made, and then only the
/// session's start and the batches are checked.
static WATCHER: OnceLock<Option<u32>> = OnceLock::new();

/// A driven session begins: the foreground is read now, the agent told if the session
/// begins blocked (or no longer is), and the foreground followed until [`end`].
pub fn begin(shared: &'static Shared) {
    let _ = SHARED.set(shared);
    {
        let mut watch = lock();
        let hwnd = foreground_window();
        say(watch.begin(hwnd.0 as isize, || verdict(hwnd)));
    }
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

/// A batch is about to be injected: if the foreground is not the window last judged, it
/// is judged now. A net under the event hook, which on the vm124 once let a change to
/// `osk.exe` go by unreported (the hook's own focus hook missed it too): the master keeps
/// sending while its pointer is frozen over there, so the next batch catches it. One
/// `GetForegroundWindow` per batch otherwise — no timer, nothing between sessions.
pub fn input() {
    let mut watch = lock();
    let hwnd = foreground_window();
    say(watch.batch(hwnd.0 as isize, || verdict(hwnd)));
}

/// The event hook says the foreground changed. Whatever window the event names, the
/// foreground is read again now: the event was queued, and may be older than what a
/// batch already saw (see [`Watch`]).
fn changed() {
    let mut watch = lock();
    let hwnd = foreground_window();
    say(watch.event(hwnd.0 as isize, || verdict(hwnd)));
}

/// `SendInput` put fewer events into the system than it was handed: a block whose cause
/// this hook did not see, said as such.
pub fn short() {
    let mut watch = lock();
    say(watch.short());
}

fn foreground_window() -> HWND {
    // SAFETY: no pointer.
    unsafe { GetForegroundWindow() }
}

fn lock() -> std::sync::MutexGuard<'static, Watch> {
    WATCH.lock().unwrap_or_else(|p| p.into_inner())
}

/// Says `word`, if there is one. Called with the lock held, so that what is said goes
/// out in the order it was observed.
fn say(word: Option<Option<InjectionBlock>>) {
    let Some(block) = word else { return };
    match &block {
        Some(block) if block.cause == BlockCause::Integrity => eprintln!(
            "[LittleBigMouse.Hook] injection blocked: {} has the focus at a higher \
             integrity level than this hook",
            if block.process.is_empty() {
                "a window"
            } else {
                &block.process
            }
        ),
        Some(_) => eprintln!(
            "[LittleBigMouse.Hook] injection blocked: SendInput refused events, cause unknown"
        ),
        None => eprintln!("[LittleBigMouse.Hook] injection no longer blocked"),
    }
    if let Some(shared) = SHARED.get() {
        shared.broadcast(&Event::InjectionBlocked { block });
    }
}

/// The window in the foreground, as a number: to tell whether it changed.
pub(crate) fn foreground_id() -> isize {
    foreground_window().0 as isize
}

/// Whether the window `id` ([`foreground_id`]) runs above this hook. A Windows master's
/// low-level hooks then see nothing of the input it gets (UIPI), as `SendInput` here
/// cannot reach it (`hook::windows::master`).
pub(crate) fn above_us(id: isize) -> Option<InjectionBlock> {
    verdict(HWND(id as *mut _))
}

/// What the window `hwnd` lets this hook inject.
fn verdict(hwnd: HWND) -> Option<InjectionBlock> {
    static OURS: OnceLock<Option<Integrity>> = OnceLock::new();
    // SAFETY: the pseudo-handle of this process needs no closing.
    let ours = *OURS.get_or_init(|| token_integrity(unsafe { GetCurrentProcess() }));
    blocking::blocked(ours, &foreground(hwnd))
}

fn foreground(hwnd: HWND) -> Foreground {
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
                changed();
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
    // The window the event names is not read: it may be stale (see `changed`).
    _hwnd: HWND,
    _id_object: i32,
    _id_child: i32,
    _thread: u32,
    _time: u32,
) {
    crate::hook::guard(changed);
}
