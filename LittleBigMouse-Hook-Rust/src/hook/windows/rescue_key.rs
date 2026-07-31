//! The panic shortcut listener.
//!
//! A cursor trapped on a screen it cannot leave puts every recovery the UI offers
//! behind a click that cannot be made. This gives the keyboard a way in.
//!
//! Three deliberate choices:
//!
//! * **`RegisterHotKey`, not `WH_KEYBOARD_LL`.** The daemon already hooks the mouse
//!   globally; a keyboard hook would mean seeing every keystroke on the desktop — a
//!   keylogging surface that antivirus and anti-cheat notice, and that a mouse
//!   utility has no business needing. This sees one combination and nothing else,
//!   and Windows arbitrates collisions instead of leaving them silent.
//!
//! * **Its own thread, with its own message loop.** `WM_HOTKEY` is posted to the
//!   queue of the thread that registered. Registering on the hook pump would make
//!   the rescue depend on the pump it is rescuing you from — the one thing a rescue
//!   path must never do.
//!
//! * **Held, not tapped.** Three modifiers are already hard to hit by accident, but
//!   a shortcut that stops the mouse engine should ask to be meant.

use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use windows::Win32::Foundation::HWND;
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, RegisterHotKey, UnregisterHotKey, HOT_KEY_MODIFIERS,
};
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetMessageW, PeekMessageW, TranslateMessage, MSG, PM_REMOVE, WM_APP,
    WM_HOTKEY,
};

use crate::shared::Shared;
use crate::shortcut::{Shortcut, MOD_ALT, MOD_CONTROL, MOD_SHIFT, MOD_WIN};

/// Ask the listener to re-read the shortcut from `Shared` and re-register.
/// Posted when a freshly loaded layout names a different one.
pub const WM_RESCUE_RECONFIGURE: u32 = WM_APP + 2;

/// Our hotkey id. Only meaningful within this thread.
const HOTKEY_ID: i32 = 1;

/// How long the combination must stay down. Long enough not to fire on a fumble,
/// short enough that the two presses of a full escalation stay under three seconds.
const HOLD: Duration = Duration::from_millis(900);

/// How often the hold is re-checked. Fine enough that letting go feels immediate.
const HOLD_POLL: Duration = Duration::from_millis(25);

const VK_SHIFT: u32 = 0x10;
const VK_CONTROL: u32 = 0x11;
const VK_MENU: u32 = 0x12;
const VK_LWIN: u32 = 0x5B;
const VK_RWIN: u32 = 0x5C;

/// Start the listener. `on_fire` runs on this thread, so it must not block on
/// anything the hook pump owns.
pub fn spawn(shared: &'static Shared, on_fire: fn(&'static Shared)) {
    std::thread::spawn(move || {
        shared
            .rescue_tid
            .store(unsafe { GetCurrentThreadId() }, Ordering::SeqCst);

        let mut current = register_wanted(shared, None);

        let mut msg = MSG::default();
        loop {
            let ret = unsafe { GetMessageW(&mut msg, None, 0, 0) };
            match ret.0 {
                -1 | 0 => break, // error, or WM_QUIT
                _ => {}
            }

            match msg.message {
                WM_HOTKEY => {
                    if let Some(shortcut) = current {
                        if held_long_enough(shortcut) {
                            on_fire(shared);
                        }
                        // Auto-repeat queues a WM_HOTKEY per repeat while the
                        // combination is down. Wait it out and drop them, or letting
                        // go would fire the next step immediately.
                        settle(shortcut);
                    }
                }
                WM_RESCUE_RECONFIGURE => current = register_wanted(shared, current),
                _ => unsafe {
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                },
            }
        }
    });
}

/// Re-read the desired shortcut and reconcile the registration to it.
/// Returns what is registered now.
fn register_wanted(shared: &Shared, current: Option<Shortcut>) -> Option<Shortcut> {
    let text = shared
        .rescue_shortcut
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .clone();

    let wanted = Shortcut::parse(&text);
    if wanted == current {
        return current;
    }

    if current.is_some() {
        unsafe {
            let _ = UnregisterHotKey(HWND::default(), HOTKEY_ID);
        }
    }

    let Some(shortcut) = wanted else {
        if !text.trim().is_empty() {
            eprintln!("[LittleBigMouse.Hook] rescue shortcut \"{text}\" is not usable, ignored");
            shared.broadcast(&crate::ipc::protocol::shortcut_unavailable(&text));
        }
        shared.rescue_registered.store(false, Ordering::SeqCst);
        return None;
    };

    // A null HWND registers against this thread, which is where the message loop
    // above is waiting.
    let ok = unsafe {
        RegisterHotKey(
            HWND::default(),
            HOTKEY_ID,
            HOT_KEY_MODIFIERS(shortcut.modifiers),
            shortcut.key,
        )
    }
    .is_ok();

    shared.rescue_registered.store(ok, Ordering::SeqCst);
    if ok {
        eprintln!("[LittleBigMouse.Hook] rescue shortcut registered: {text}");
        Some(shortcut)
    } else {
        // Almost always another application owning the combination. Worth saying
        // out loud: a rescue that silently does not exist is worse than none.
        eprintln!("[LittleBigMouse.Hook] rescue shortcut \"{text}\" is already taken, NOT registered");
        shared.broadcast(&crate::ipc::protocol::shortcut_unavailable(&text));
        None
    }
}

/// Was the whole combination held down for [`HOLD`]?
fn held_long_enough(shortcut: Shortcut) -> bool {
    let deadline = Instant::now() + HOLD;
    while Instant::now() < deadline {
        if !all_down(shortcut) {
            return false;
        }
        std::thread::sleep(HOLD_POLL);
    }
    all_down(shortcut)
}

/// Wait for the combination to be let go, then drop the auto-repeat backlog.
fn settle(shortcut: Shortcut) {
    while all_down(shortcut) {
        std::thread::sleep(HOLD_POLL);
    }
    let mut msg = MSG::default();
    while unsafe { PeekMessageW(&mut msg, None, WM_HOTKEY, WM_HOTKEY, PM_REMOVE) }.as_bool() {}
}

fn all_down(shortcut: Shortcut) -> bool {
    if !key_down(shortcut.key) {
        return false;
    }
    let m = shortcut.modifiers;
    if m & MOD_CONTROL != 0 && !key_down(VK_CONTROL) {
        return false;
    }
    if m & MOD_ALT != 0 && !key_down(VK_MENU) {
        return false;
    }
    if m & MOD_SHIFT != 0 && !key_down(VK_SHIFT) {
        return false;
    }
    if m & MOD_WIN != 0 && !(key_down(VK_LWIN) || key_down(VK_RWIN)) {
        return false;
    }
    true
}

fn key_down(vk: u32) -> bool {
    unsafe { GetAsyncKeyState(vk as i32) as u16 & 0x8000 != 0 }
}
