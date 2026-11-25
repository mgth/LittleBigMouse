//! Overlays shown by a process that has no window of its own: the agent of a remote
//! machine in the multi-PC mode, drawing the rulers its master asks for
//! (docs/multi-pc.md, "Rulers on a remote's screens").
//!
//! The surfaces live on a thread of their own, which exists only while something is
//! shown: [`Standalone::show`] starts it, dropping the handle ends it, and with it every
//! surface. Nothing is left installed between two showings.
//!
//! The bands are either [`Band::passive`] ([`Standalone::show`]): the pointer and the
//! keyboard go through them to whatever is beneath; or they take them
//! ([`Standalone::interactive`]) and hand every gesture to a callback, for an editor
//! elsewhere — the master's, in the multi-PC mode — to act on.
//!
//! What the thread waits on:
//!
//! - **Windows**: its message queue, and nothing else. A new scene or the end is posted
//!   to it as a thread message; a display change arrives as `WM_DISPLAYCHANGE` on the
//!   surfaces themselves, a gesture as the surfaces' own mouse and key messages. No timer.
//! - **Linux**: the surfaces belong to the same helper process an editor uses
//!   (`--screen-overlay-host`, which the binary must dispatch to
//!   [`crate::linux::run_host`], as `lbm-app` and `lbm-agent` do). That helper ends itself
//!   when it is not told anything for two seconds, so that a wedged owner leaves no ruler
//!   behind; the thread therefore renews it every [`PULSE`] while — and only while —
//!   something is shown. A gesture or an error from the helper wakes it at once.

use crate::input::Input;
use crate::scene::Band;

/// How often the Linux helper's lease is renewed while rulers are shown. Its lease is two
/// seconds: four renewals fit in it.
pub const PULSE: std::time::Duration = std::time::Duration::from_millis(500);

/// Called once, from the overlay thread, when the surfaces could not be shown or stopped
/// being valid (a display changed under them, the helper died). They are gone by then.
pub type OnError = Box<dyn FnOnce(String) + Send>;

/// Called from the overlay thread with every gesture on an interactive band, in order.
pub type OnInput = Box<dyn FnMut(Input) + Send>;

/// What the overlay thread is told.
#[cfg_attr(not(any(windows, target_os = "linux")), allow(dead_code))]
enum Message {
    Show(Vec<Band>),
    /// The Linux helper said something: an input to pass on, or an error.
    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    Woken,
    Stop,
}

/// The surfaces of a process without a window, while they are shown.
pub struct Standalone {
    #[cfg(any(windows, target_os = "linux"))]
    sender: std::sync::mpsc::Sender<Message>,
    #[cfg(windows)]
    thread_id: u32,
    passive: bool,
}

/// The same bands, made passive or not.
fn marked(mut bands: Vec<Band>, passive: bool) -> Vec<Band> {
    for band in &mut bands {
        band.passive = passive;
    }
    bands
}

impl Standalone {
    /// Starts showing `bands`, passive, on a thread of their own. An error found once
    /// shown comes back through `on_error`, and the thread ends with it.
    pub fn show(bands: Vec<Band>, on_error: OnError) -> Result<Standalone, String> {
        Self::start(bands, on_error, None)
    }

    /// Starts showing `bands`, taking the pointer and the keyboard: what is done on them
    /// goes to `on_input`, which nothing here acts on.
    pub fn interactive(
        bands: Vec<Band>,
        on_error: OnError,
        on_input: OnInput,
    ) -> Result<Standalone, String> {
        Self::start(bands, on_error, Some(on_input))
    }

    fn start(
        bands: Vec<Band>,
        on_error: OnError,
        on_input: Option<OnInput>,
    ) -> Result<Standalone, String> {
        if bands.is_empty() {
            return Err("Nothing to show.".into());
        }
        let passive = on_input.is_none();
        platform::spawn(marked(bands, passive), on_error, on_input, passive)
    }

    /// Whether the bands let the pointer through.
    pub fn passive(&self) -> bool {
        self.passive
    }

    /// Shows `bands` instead of what was shown, as passive as they were. Ignored once the
    /// thread has ended on an error (which `on_error` has reported).
    pub fn update(&self, bands: Vec<Band>) {
        if bands.is_empty() {
            return;
        }
        #[cfg(any(windows, target_os = "linux"))]
        {
            let _ = self.sender.send(Message::Show(marked(bands, self.passive)));
            self.wake();
        }
    }

    #[cfg(any(windows, target_os = "linux"))]
    fn wake(&self) {
        #[cfg(windows)]
        platform::wake(self.thread_id);
    }
}

impl Drop for Standalone {
    /// Ends the thread, which takes every surface down with it. Not waited for: a
    /// compositor that does not answer must not hold up the caller. On Linux the helper
    /// is killed in any case.
    fn drop(&mut self) {
        #[cfg(any(windows, target_os = "linux"))]
        {
            let _ = self.sender.send(Message::Stop);
            self.wake();
        }
    }
}

#[cfg(target_os = "linux")]
mod platform {
    use super::*;
    use std::sync::mpsc::{self, RecvTimeoutError};
    use std::sync::{Arc, Mutex};
    use std::time::Instant;

    pub(super) fn spawn(
        bands: Vec<Band>,
        on_error: OnError,
        mut on_input: Option<OnInput>,
        passive: bool,
    ) -> Result<Standalone, String> {
        let (sender, receiver) = mpsc::channel::<Message>();
        // The helper's reader thread wakes this one; a sender behind a lock, as the
        // waker must be shareable.
        let woken = Mutex::new(sender.clone());
        let waker: crate::linux::Waker = Arc::new(move || {
            let _ = woken.lock().map(|s| s.send(Message::Woken));
        });
        std::thread::Builder::new()
            .name("lbm-overlay".into())
            .spawn(move || {
                let mut overlay = crate::Manager::new(crate::linux::Native::with_waker(waker));
                let mut shown = bands;
                // A sync renews the lease: once per pulse, or for a new scene — not for
                // every gesture, which would write to the helper at the mouse's rate.
                let mut synced: Option<Instant> = None;
                loop {
                    if synced.is_none_or(|at| at.elapsed() >= PULSE) {
                        if let Err(error) = overlay.sync(&shown) {
                            overlay.clear();
                            on_error(error);
                            return;
                        }
                        synced = Some(Instant::now());
                    }
                    for input in overlay.take_input() {
                        if let Some(on_input) = &mut on_input {
                            on_input(input);
                        }
                    }
                    let wait = synced.map_or(PULSE, |at| PULSE.saturating_sub(at.elapsed()));
                    match receiver.recv_timeout(wait) {
                        Ok(Message::Show(bands)) => {
                            shown = bands;
                            synced = None;
                        }
                        Ok(Message::Woken) | Err(RecvTimeoutError::Timeout) => {}
                        Ok(Message::Stop) | Err(RecvTimeoutError::Disconnected) => {
                            overlay.clear();
                            return;
                        }
                    }
                }
            })
            .map_err(|e| format!("Cannot start screen overlays: {e}"))?;
        Ok(Standalone { sender, passive })
    }
}

#[cfg(windows)]
mod platform {
    use super::*;
    use std::sync::mpsc;
    use windows::Win32::{
        Foundation::{LPARAM, WPARAM},
        System::Threading::GetCurrentThreadId,
        UI::WindowsAndMessaging::{
            DispatchMessageW, GetMessageW, PeekMessageW, PostThreadMessageW, TranslateMessage, MSG,
            PM_NOREMOVE, WM_APP, WM_USER,
        },
    };

    /// The thread message that says the channel has something.
    const WAKE: u32 = WM_APP + 0x4c;

    pub(super) fn wake(thread_id: u32) {
        // SAFETY: posting to a thread id; a thread already gone just fails the post.
        unsafe {
            let _ = PostThreadMessageW(thread_id, WAKE, WPARAM(0), LPARAM(0));
        }
    }

    pub(super) fn spawn(
        bands: Vec<Band>,
        on_error: OnError,
        mut on_input: Option<OnInput>,
        passive: bool,
    ) -> Result<Standalone, String> {
        let (sender, receiver) = mpsc::channel::<Message>();
        let (started, thread_id) = mpsc::channel::<u32>();
        std::thread::Builder::new()
            .name("lbm-overlay".into())
            .spawn(move || {
                let mut msg = MSG::default();
                // SAFETY: plain Win32 calls on this thread's own queue. The peek creates the
                // queue before the id is handed out, so no wake-up posted after can be lost.
                unsafe {
                    let _ = PeekMessageW(&mut msg, None, WM_USER, WM_USER, PM_NOREMOVE);
                    let _ = started.send(GetCurrentThreadId());
                }
                let mut overlay = match crate::Overlay::standalone() {
                    Ok(overlay) => overlay,
                    Err(error) => return on_error(error),
                };
                let mut shown = bands;
                loop {
                    if let Err(error) = overlay.sync(&shown) {
                        overlay.clear();
                        return on_error(error);
                    }
                    // SAFETY: as above; DispatchMessageW runs the overlay's own window
                    // procedure on this thread, which owns those windows.
                    let got = unsafe { GetMessageW(&mut msg, None, 0, 0) };
                    if got.0 <= 0 {
                        return;
                    }
                    if msg.hwnd.is_invalid() && msg.message == WAKE {
                        for next in receiver.try_iter() {
                            match next {
                                Message::Show(bands) => shown = bands,
                                Message::Woken => {}
                                Message::Stop => return,
                            }
                        }
                    } else {
                        unsafe {
                            let _ = TranslateMessage(&msg);
                            DispatchMessageW(&msg);
                        }
                        // What the window procedure queued of the pointer and the keys.
                        for input in overlay.take_input() {
                            if let Some(on_input) = &mut on_input {
                                on_input(input);
                            }
                        }
                    }
                }
            })
            .map_err(|e| format!("Cannot start screen overlays: {e}"))?;
        let thread_id = thread_id
            .recv()
            .map_err(|_| "The screen overlay thread did not start.".to_owned())?;
        Ok(Standalone {
            sender,
            thread_id,
            passive,
        })
    }
}

#[cfg(not(any(windows, target_os = "linux")))]
mod platform {
    use super::*;
    pub(super) fn spawn(
        _: Vec<Band>,
        _: OnError,
        _: Option<OnInput>,
        _: bool,
    ) -> Result<Standalone, String> {
        Err("Screen overlays are currently available on Windows and Linux only.".into())
    }
}
