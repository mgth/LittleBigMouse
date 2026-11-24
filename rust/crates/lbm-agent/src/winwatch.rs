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
//!
//! The same window hears the wallpaper being set (`WM_SETTINGCHANGE` naming
//! `SPI_SETDESKWALLPAPER`), which matters to nobody but a master of this machine: it is
//! passed on only while a watch is held ([`watch_wallpaper`], `wallpaper_watch`), and a
//! registry notification stands beside it for as long, for what changes the wallpaper
//! without a broadcast.

use std::cell::RefCell;
use std::sync::Mutex;

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

use windows::Win32::Foundation::{CloseHandle, HANDLE, WAIT_OBJECT_0};
use windows::Win32::System::Registry::{
    RegCloseKey, RegNotifyChangeKeyValue, RegOpenKeyExW, HKEY, HKEY_CURRENT_USER, KEY_NOTIFY,
    REG_NOTIFY_CHANGE_LAST_SET,
};
use windows::Win32::System::Threading::{CreateEventW, SetEvent, WaitForMultipleObjects, INFINITE};

use crate::reconcile::Input;
use crate::wallpaper_watch::{is_wallpaper_message, Changes, Watching};

const CLASS_NAME: PCWSTR = w!("LittleBigMouseAgentDisplayWatch");

/// `WM_WTSSESSION_CHANGE` reasons after which the displays are worth a look.
const WTS_CONSOLE_CONNECT: usize = 0x1;
const WTS_REMOTE_CONNECT: usize = 0x3;
const WTS_SESSION_UNLOCK: usize = 0x8;

thread_local! {
    /// Where the window's messages go: the thread's own, read by its window procedure.
    static INPUTS: RefCell<Option<UnboundedSender<Input>>> = const { RefCell::new(None) };
}

/// Who is told the wallpaper was set, while somebody is: the one watch of the agent, and
/// the number it was given — a watch let go of late must not take down its successor.
/// Process-wide, not the window thread's: the watch is taken and dropped from the
/// agent's loop.
static WALLPAPER: Mutex<Option<(u64, Changes)>> = Mutex::new(None);
static WATCHES: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Where the shell keeps the wallpaper, under `HKEY_CURRENT_USER`: `WallPaper` and the
/// `TranscodedImageCache` values are rewritten when the picture changes, slide shows
/// included, with or without a broadcast.
const WALLPAPER_KEY: PCWSTR = w!("Control Panel\\Desktop");

/// Says `changes` when the wallpaper is set, until the watch is dropped: the window's
/// broadcast, and a notification on the registry key — a thread asleep on two handles,
/// woken by the system or by the drop, never by a clock.
pub fn watch_wallpaper(changes: Changes) -> Watching {
    let id = WATCHES.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    *WALLPAPER.lock().unwrap() = Some((id, changes.clone()));
    Watching::new(WallpaperWatch {
        id,
        registry: RegistryWatch::spawn(changes),
    })
}

/// The broadcast came. Nobody to tell unless a master is linked: the message then just
/// goes its way.
fn wallpaper_was_set() {
    if let Some((_, changes)) = WALLPAPER.lock().unwrap().as_ref() {
        changes.changed();
    }
}

struct WallpaperWatch {
    id: u64,
    #[allow(dead_code)]
    registry: Option<RegistryWatch>,
}

impl Drop for WallpaperWatch {
    fn drop(&mut self) {
        let mut told = WALLPAPER.lock().unwrap();
        if told.as_ref().is_some_and(|(id, _)| *id == self.id) {
            *told = None;
        }
    }
}

/// A handle that may cross to the thread waiting on it: an event is the kernel's, and
/// signalling or waiting on one is sound from any thread.
#[derive(Clone, Copy)]
struct Event(HANDLE);
// SAFETY: see above; the handle is closed once, by `RegistryWatch::drop`, after the join.
unsafe impl Send for Event {}
unsafe impl Sync for Event {}

/// The thread following [`WALLPAPER_KEY`], and what ends it.
struct RegistryWatch {
    stop: Event,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl RegistryWatch {
    fn spawn(changes: Changes) -> Option<Self> {
        // SAFETY: a fresh, unnamed, auto-reset event, owned by the watch.
        let stop = Event(unsafe { CreateEventW(None, false, false, None) }.ok()?);
        let thread = std::thread::Builder::new()
            .name("wallpaper-watch".to_owned())
            .spawn(move || follow_registry(stop, changes))
            .inspect_err(|error| eprintln!("[lbm-agent] no wallpaper registry watch: {error}"))
            .ok();
        if thread.is_none() {
            // SAFETY: nobody else has the handle.
            let _ = unsafe { CloseHandle(stop.0) };
            return None;
        }
        Some(RegistryWatch { stop, thread })
    }
}

impl Drop for RegistryWatch {
    fn drop(&mut self) {
        // SAFETY: the event is open until the line after the join.
        unsafe {
            let _ = SetEvent(self.stop.0);
            if let Some(thread) = self.thread.take() {
                let _ = thread.join();
            }
            let _ = CloseHandle(self.stop.0);
        }
    }
}

fn follow_registry(stop: Event, changes: Changes) {
    // SAFETY: plain registry and event calls on handles this thread opens and closes;
    // `stop` outlives the thread (its owner joins before closing it).
    unsafe {
        let mut key = HKEY::default();
        if RegOpenKeyExW(HKEY_CURRENT_USER, WALLPAPER_KEY, 0, KEY_NOTIFY, &mut key).is_err() {
            eprintln!("[lbm-agent] no wallpaper registry watch: the key cannot be opened");
            return;
        }
        if let Ok(changed) = CreateEventW(None, false, false, None) {
            // One notification per registration: armed again after each, and the first
            // registration asks for a reading of its own (`wallpaper_watch::follow`).
            crate::wallpaper_watch::follow(
                || {
                    RegNotifyChangeKeyValue(key, false, REG_NOTIFY_CHANGE_LAST_SET, changed, true)
                        .is_ok()
                },
                // The stop, or a wait that failed: either way there is nothing to follow.
                || {
                    WaitForMultipleObjects(&[stop.0, changed], false, INFINITE).0
                        == WAIT_OBJECT_0.0 + 1
                },
                &changes,
            );
            let _ = CloseHandle(changed);
        }
        let _ = RegCloseKey(key);
    }
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
    if is_wallpaper_message(msg, wparam.0) {
        wallpaper_was_set();
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

    /// The crate's numbers are the system's: the filter is written without the `windows`
    /// crate so that it is tested everywhere, and this is what keeps it honest.
    #[test]
    fn the_wallpaper_filters_numbers_are_the_systems() {
        assert_eq!(crate::wallpaper_watch::WM_SETTINGCHANGE, WM_SETTINGCHANGE);
        assert_eq!(
            crate::wallpaper_watch::SPI_SETDESKWALLPAPER,
            SPI_SETDESKWALLPAPER.0 as usize
        );
        assert!(!is_display_message(
            WM_SETTINGCHANGE,
            SPI_SETDESKWALLPAPER.0 as usize
        ));
    }

    /// The wallpaper being set is passed on while a watch is held, and to nobody once
    /// it is let go of — what the window procedure does with the broadcast, without a
    /// window: nothing is set on the runner's desktop.
    #[test]
    fn the_wallpaper_being_set_is_passed_on_only_while_a_watch_is_held() {
        let (changes, mut said) = crate::wallpaper_watch::channel();
        wallpaper_was_set();
        assert!(said.try_recv().is_err(), "no watch yet");

        let watching = watch_wallpaper(changes);
        wallpaper_was_set();
        assert!(said.try_recv().is_ok(), "the broadcast was not passed on");

        drop(watching);
        while said.try_recv().is_ok() {}
        wallpaper_was_set();
        assert!(said.try_recv().is_err(), "passed on with no watch held");
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
