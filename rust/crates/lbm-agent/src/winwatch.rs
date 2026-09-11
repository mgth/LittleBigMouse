//! Display changes under Windows, seen by the agent itself (the plan, phase 2: "fenêtre
//! cachée top-level reprise de `hook/windows/display.rs`, plus les notifications de
//! session WTS").
//!
//! The hook reports display changes too, but only while it runs: a change while no hook
//! is connected would leave the agent's layout stale, and the next hook would be handed
//! it. So the agent keeps its own window, hidden and **top-level** (`WM_DISPLAYCHANGE` and
//! the `SPI_SETWORKAREA` broadcast never reach a message-only window), on a thread of its
//! own. It also follows the session (WTS): back from the lock screen or the secure desktop,
//! or reconnected to the console, the displays may have changed or the hook may have been
//! unhooked meanwhile — the display flow rebuilds or re-hooks, as for any change.
//!
//! Every one of them is an [`Input::DisplayChanged`]: the reconciler's debounce folds it
//! with the hook's own report of the same change.

use std::cell::RefCell;

use tokio::sync::mpsc::UnboundedSender;
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::RemoteDesktop::{
    WTSRegisterSessionNotification, WTSUnRegisterSessionNotification, NOTIFY_FOR_THIS_SESSION,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW, PostQuitMessage,
    RegisterClassExW, TranslateMessage, CW_USEDEFAULT, MSG, SPI_SETWORKAREA, WINDOW_EX_STYLE,
    WM_DISPLAYCHANGE, WM_SETTINGCHANGE, WM_WTSSESSION_CHANGE, WNDCLASSEXW, WS_OVERLAPPEDWINDOW,
};

use crate::reconcile::Input;

const CLASS_NAME: PCWSTR = w!("LittleBigMouseAgentDisplayWatch");

/// `WM_WTSSESSION_CHANGE` reasons after which the displays are worth a look.
const WTS_CONSOLE_CONNECT: usize = 0x1;
const WTS_REMOTE_CONNECT: usize = 0x3;
const WTS_SESSION_UNLOCK: usize = 0x8;

thread_local! {
    /// Where the window's messages go: the thread's own, read by its window procedure.
    static INPUTS: RefCell<Option<UnboundedSender<Input>>> = const { RefCell::new(None) };
}

/// Which messages mean the displays may have changed.
pub fn is_display_message(msg: u32, wparam: usize) -> bool {
    match msg {
        WM_DISPLAYCHANGE => true,
        WM_SETTINGCHANGE => wparam as u32 == SPI_SETWORKAREA.0,
        WM_WTSSESSION_CHANGE => matches!(
            wparam,
            WTS_CONSOLE_CONNECT | WTS_REMOTE_CONNECT | WTS_SESSION_UNLOCK
        ),
        _ => false,
    }
}

/// Starts the watch on a thread of its own; it ends at the first message after `inputs`
/// is closed.
pub fn spawn(inputs: UnboundedSender<Input>) -> Option<std::thread::JoinHandle<()>> {
    std::thread::Builder::new()
        .name("display-watch".to_owned())
        .spawn(move || run(inputs))
        .inspect_err(|error| eprintln!("[lbm-agent] no display watch: {error}"))
        .ok()
}

fn run(inputs: UnboundedSender<Input>) {
    INPUTS.with(|slot| *slot.borrow_mut() = Some(inputs));
    // SAFETY: plain Win32 window setup on this thread, torn down below on this thread.
    unsafe {
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
        let Ok(window) = CreateWindowExW(
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
            instance,
            None,
        ) else {
            eprintln!("[lbm-agent] no display watch: the window could not be created");
            return;
        };
        let sessions = WTSRegisterSessionNotification(window, NOTIFY_FOR_THIS_SESSION).is_ok();

        let mut message = MSG::default();
        while GetMessageW(&mut message, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }

        if sessions {
            let _ = WTSUnRegisterSessionNotification(window);
        }
        let _ = DestroyWindow(window);
    }
}

unsafe extern "system" fn window_procedure(
    window: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if is_display_message(msg, wparam.0) {
        let delivered = INPUTS.with(|slot| {
            slot.borrow()
                .as_ref()
                .is_some_and(|inputs| inputs.send(Input::DisplayChanged).is_ok())
        });
        if !delivered {
            // The agent is gone: so is the watch.
            // SAFETY: ends this thread's message loop.
            unsafe { PostQuitMessage(0) };
        }
        return LRESULT(0);
    }
    // SAFETY: the default procedure, with what the window was sent.
    unsafe { DefWindowProcW(window, msg, wparam, lparam) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::UI::WindowsAndMessaging::{SPI_SETDESKWALLPAPER, WM_PAINT};

    #[test]
    fn the_messages_that_mean_a_display_change() {
        assert!(is_display_message(WM_DISPLAYCHANGE, 0));
        assert!(is_display_message(
            WM_SETTINGCHANGE,
            SPI_SETWORKAREA.0 as usize
        ));
        assert!(!is_display_message(
            WM_SETTINGCHANGE,
            SPI_SETDESKWALLPAPER.0 as usize
        ));
        assert!(is_display_message(WM_WTSSESSION_CHANGE, WTS_SESSION_UNLOCK));
        assert!(is_display_message(
            WM_WTSSESSION_CHANGE,
            WTS_CONSOLE_CONNECT
        ));
        assert!(!is_display_message(WM_WTSSESSION_CHANGE, 0x7)); // WTS_SESSION_LOCK
        assert!(!is_display_message(WM_PAINT, 0));
    }

    /// The real window, on the runner's desktop: a display change posted to it comes out
    /// as an input, and once the agent is gone the next message ends the thread.
    #[test]
    fn the_window_turns_a_display_change_into_an_input_and_ends_with_the_agent() {
        use windows::Win32::UI::WindowsAndMessaging::{FindWindowW, PostMessageW};

        let (inputs, mut received) = tokio::sync::mpsc::unbounded_channel();
        let thread = spawn(inputs).unwrap();
        let window = (0..500)
            .find_map(|_| {
                // SAFETY: a lookup by class name.
                let found = unsafe { FindWindowW(CLASS_NAME, None) }.ok();
                if found.is_none() {
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
                found
            })
            .expect("the watch window");
        let post = || {
            // SAFETY: a message to a window of this process.
            unsafe { PostMessageW(window, WM_DISPLAYCHANGE, WPARAM(0), LPARAM(0)) }.unwrap()
        };

        post();
        let input = (0..500).find_map(|_| {
            let input = received.try_recv().ok();
            if input.is_none() {
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            input
        });
        assert_eq!(input, Some(Input::DisplayChanged));

        drop(received);
        post();
        thread.join().unwrap();
    }
}
