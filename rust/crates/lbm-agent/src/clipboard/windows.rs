//! The clipboard under Windows: a message-only window on a thread of its own, told of
//! every change by a clipboard format listener (`WM_CLIPBOARDUPDATE`), and owner of the
//! clipboard with **delayed rendering** while it holds another machine's offer — each
//! format is set without data, and Windows asks for it (`WM_RENDERFORMAT`) the first
//! time an application reads it. Nothing is ever polled.
//!
//! `WM_RENDERFORMAT` must be answered before the window procedure returns, while the
//! application that reads waits in `GetClipboardData`: this thread waits there for the
//! content to cross, up to [`RENDER_PATIENCE`] (Windows gives up at 30 s), and does nothing
//! else meanwhile.
//!
//! An offer held here is kept out of the clipboard history and the cloud clipboard
//! (`CanIncludeInClipboardHistory`, `CanUploadToCloudClipboard` at 0): the history would
//! read every format as soon as it is offered, which would make every copy on the other
//! machine cross whether it is pasted or not, and another machine's clipboard is not this
//! machine's to upload. What a password manager marks as not to be monitored
//! (`ExcludeClipboardContentFromMonitorProcessing`) is never announced.

use std::cell::RefCell;
use std::sync::mpsc as std_mpsc;
use std::time::Duration;

use lbm_net::wire::ClipFormat;
use tokio::sync::{mpsc, oneshot};
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{HANDLE, HGLOBAL, HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::DataExchange::{
    AddClipboardFormatListener, CloseClipboard, EmptyClipboard, GetClipboardData,
    GetClipboardOwner, GetClipboardSequenceNumber, IsClipboardFormatAvailable, OpenClipboard,
    RegisterClipboardFormatW, RemoveClipboardFormatListener, SetClipboardData,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Memory::{
    GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock, GMEM_MOVEABLE,
};
use windows::Win32::System::Ole::{CF_DIB, CF_DIBV5, CF_UNICODETEXT};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW, PostMessageW,
    PostQuitMessage, RegisterClassExW, HWND_MESSAGE, MSG, WINDOW_EX_STYLE, WINDOW_STYLE, WM_APP,
    WM_CLIPBOARDUPDATE, WM_DESTROY, WM_DESTROYCLIPBOARD, WM_RENDERFORMAT, WNDCLASSEXW,
};

use super::{convert, too_large, Clipboard, Command, Data, Event};

const LOG: &str = "[lbm-agent] clipboard (Windows)";
const CLASS_NAME: PCWSTR = w!("LittleBigMouseAgentClipboard");
/// A command waits in the channel.
const WM_COMMAND_WAITING: u32 = WM_APP + 1;
/// How long an application reading another machine's clipboard is kept waiting. Windows
/// itself gives up on `WM_RENDERFORMAT` after 30 s and the reader gets nothing (Raymond
/// Chen, The Old New Thing, 2022-06-09): waiting longer would only keep this thread from
/// the next request.
const RENDER_PATIENCE: Duration = Duration::from_secs(25);
/// `OpenClipboard` fails while another application has it open, which lasts
/// milliseconds: tried this many times, this far apart.
const OPEN_TRIES: u32 = 10;
const OPEN_PAUSE: Duration = Duration::from_millis(15);

/// The registered formats this reads or writes.
#[derive(Clone, Copy)]
struct Formats {
    png: u32,
    exclude: u32,
    history: u32,
    cloud: u32,
}

impl Formats {
    fn register() -> Formats {
        // SAFETY: registering a name returns its number, the same in every process.
        unsafe {
            Formats {
                png: RegisterClipboardFormatW(w!("PNG")),
                exclude: RegisterClipboardFormatW(w!(
                    "ExcludeClipboardContentFromMonitorProcessing"
                )),
                history: RegisterClipboardFormatW(w!("CanIncludeInClipboardHistory")),
                cloud: RegisterClipboardFormatW(w!("CanUploadToCloudClipboard")),
            }
        }
    }
}

struct Win {
    events: mpsc::UnboundedSender<Event>,
    commands: std_mpsc::Receiver<Command>,
    formats: Formats,
    /// The offer this window holds the clipboard for: its number and formats.
    offer: Option<(u64, Vec<ClipFormat>)>,
}

thread_local! {
    /// The window's state, read by its procedure. Never borrowed across a Win32 call
    /// that may send this window a message: those re-enter the procedure.
    static WIN: RefCell<Option<Win>> = const { RefCell::new(None) };
}

/// The window, across threads: only ever posted to.
#[derive(Clone, Copy)]
struct Window(isize);

impl Window {
    fn hwnd(self) -> HWND {
        HWND(self.0 as *mut _)
    }
}

pub(super) fn open(events: mpsc::UnboundedSender<Event>) -> Result<Box<dyn Clipboard>, String> {
    let (commands, commands_rx) = std_mpsc::channel();
    let (made, window) = std_mpsc::channel();
    std::thread::Builder::new()
        .name("clipboard".to_owned())
        .spawn(move || run(events, commands_rx, made))
        .map_err(|error| format!("no clipboard thread: {error}"))?;
    let window = window
        .recv()
        .map_err(|_| "the clipboard window could not be made".to_owned())??;
    Ok(Box::new(Open {
        commands: Some(commands),
        window,
    }))
}

struct Open {
    commands: Option<std_mpsc::Sender<Command>>,
    window: Window,
}

impl Clipboard for Open {
    fn command(&self, command: Command) {
        if let Some(commands) = &self.commands {
            if commands.send(command).is_ok() {
                post(self.window);
            }
        }
    }
}

impl Drop for Open {
    fn drop(&mut self) {
        // The thread sees the channel closed and ends. Not joined: it may be waiting for
        // a content that will now never come, until its patience runs out.
        self.commands = None;
        post(self.window);
    }
}

fn post(window: Window) {
    // SAFETY: posting to a window this process made; a window gone fails harmlessly.
    let _ = unsafe { PostMessageW(window.hwnd(), WM_COMMAND_WAITING, WPARAM(0), LPARAM(0)) };
}

fn run(
    events: mpsc::UnboundedSender<Event>,
    commands: std_mpsc::Receiver<Command>,
    made: std_mpsc::Sender<Result<Window, String>>,
) {
    // SAFETY: plain Win32 window setup on this thread, torn down below on this thread.
    let window = unsafe {
        let module = GetModuleHandleW(None).unwrap_or_default();
        let instance = HINSTANCE(module.0);
        let class = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(window_procedure),
            hInstance: instance,
            lpszClassName: CLASS_NAME,
            ..Default::default()
        };
        // A second registration fails, the class being there already: harmless.
        RegisterClassExW(&class);
        CreateWindowExW(
            WINDOW_EX_STYLE(0),
            CLASS_NAME,
            CLASS_NAME,
            WINDOW_STYLE(0),
            0,
            0,
            0,
            0,
            HWND_MESSAGE,
            None,
            instance,
            None,
        )
    };
    let window = match window {
        Ok(window) => window,
        Err(error) => {
            let _ = made.send(Err(format!(
                "the clipboard window could not be made: {error}"
            )));
            return;
        }
    };
    WIN.with(|slot| {
        *slot.borrow_mut() = Some(Win {
            events,
            commands,
            formats: Formats::register(),
            offer: None,
        })
    });
    // SAFETY: the window is this thread's.
    if let Err(error) = unsafe { AddClipboardFormatListener(window) } {
        let _ = made.send(Err(format!("the clipboard cannot be listened to: {error}")));
        // SAFETY: as above.
        let _ = unsafe { DestroyWindow(window) };
        return;
    }
    let _ = made.send(Ok(Window(window.0 as isize)));
    // What was copied before: the sharing knows it from the start, as under Wayland.
    changed(window);

    let mut message = MSG::default();
    // SAFETY: this thread's message loop.
    unsafe {
        while GetMessageW(&mut message, None, 0, 0).as_bool() {
            DispatchMessageW(&message);
        }
    }
    WIN.with(|slot| *slot.borrow_mut() = None);
}

unsafe extern "system" fn window_procedure(
    window: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_CLIPBOARDUPDATE => changed(window),
        WM_RENDERFORMAT => render(wparam.0 as u32),
        WM_DESTROYCLIPBOARD => {
            // The clipboard was emptied under this window's offer: it holds none now.
            WIN.with(|slot| {
                if let Some(win) = slot.borrow_mut().as_mut() {
                    win.offer = None;
                }
            });
        }
        WM_COMMAND_WAITING => commands(window),
        WM_DESTROY => {
            // SAFETY: the listener of this window, on its thread.
            let _ = unsafe { RemoveClipboardFormatListener(window) };
            // SAFETY: ends this thread's message loop.
            unsafe { PostQuitMessage(0) };
        }
        // SAFETY: the default procedure, with what the window was sent.
        _ => return unsafe { DefWindowProcW(window, msg, wparam, lparam) },
    }
    LRESULT(0)
}

fn state<T>(f: impl FnOnce(&mut Win) -> T) -> Option<T> {
    WIN.with(|slot| slot.borrow_mut().as_mut().map(f))
}

/// The clipboard changed: what it holds now, unless this window put it there.
fn changed(window: HWND) {
    // SAFETY: reading who owns the clipboard.
    let owner = unsafe { GetClipboardOwner() }.ok();
    if owner == Some(window) {
        return;
    }
    let Some((events, formats)) = state(|win| {
        win.offer = None;
        (win.events.clone(), win.formats)
    }) else {
        return;
    };
    // SAFETY: plain reads of the clipboard's state.
    let copy = u64::from(unsafe { GetClipboardSequenceNumber() });
    let available = |format: u32| unsafe { IsClipboardFormatAvailable(format) }.is_ok();
    let mut shared = Vec::new();
    if !available(formats.exclude) {
        if available(u32::from(CF_UNICODETEXT.0)) {
            shared.push(ClipFormat::Text);
        }
        if available(formats.png)
            || available(u32::from(CF_DIB.0))
            || available(u32::from(CF_DIBV5.0))
        {
            shared.push(ClipFormat::Png);
        }
    }
    let _ = events.send(Event::Changed {
        copy,
        formats: shared,
    });
}

/// Every command waiting, in order; the window goes once the channel is closed.
fn commands(window: HWND) {
    loop {
        let Some(next) = state(|win| win.commands.try_recv()) else {
            return;
        };
        match next {
            Ok(Command::Offer { serial, formats }) => offer(window, serial, formats),
            Ok(Command::Withdraw { serial }) => {
                let ours = state(|win| win.offer.as_ref().is_some_and(|(s, _)| *s == serial));
                if ours == Some(true) {
                    // SAFETY: reading who owns the clipboard.
                    if unsafe { GetClipboardOwner() }.ok() == Some(window) && open_clipboard(window)
                    {
                        // SAFETY: the clipboard is open by this window.
                        unsafe {
                            let _ = EmptyClipboard();
                            let _ = CloseClipboard();
                        }
                    }
                    state(|win| win.offer = None);
                }
            }
            Ok(Command::Read {
                copy,
                format,
                limit,
                reply,
            }) => {
                let _ = reply.send(read(window, copy, format, limit));
            }
            Err(std_mpsc::TryRecvError::Empty) => return,
            Err(std_mpsc::TryRecvError::Disconnected) => {
                let ours = state(|win| win.offer.take().is_some()) == Some(true);
                // SAFETY: as above.
                if ours
                    && unsafe { GetClipboardOwner() }.ok() == Some(window)
                    && open_clipboard(window)
                {
                    // An offer nobody will serve any more: a paste finds nothing rather
                    // than waits.
                    // SAFETY: the clipboard is open by this window.
                    unsafe {
                        let _ = EmptyClipboard();
                        let _ = CloseClipboard();
                    }
                }
                // SAFETY: this thread's window.
                let _ = unsafe { DestroyWindow(window) };
                return;
            }
        }
    }
}

fn open_clipboard(window: HWND) -> bool {
    for attempt in 0..OPEN_TRIES {
        // SAFETY: opening for this window; closed by the caller.
        if unsafe { OpenClipboard(window) }.is_ok() {
            return true;
        }
        if attempt + 1 < OPEN_TRIES {
            std::thread::sleep(OPEN_PAUSE);
        }
    }
    eprintln!("{LOG}: another application keeps the clipboard open");
    false
}

/// The Windows formats an offer in `format` is set in, without data.
fn delayed(format: ClipFormat, formats: Formats) -> Vec<u32> {
    match format {
        ClipFormat::Text => vec![u32::from(CF_UNICODETEXT.0)],
        // PNG for the applications that keep transparency; a DIB for every other one,
        // from which Windows makes CF_DIBV5 and CF_BITMAP when asked.
        ClipFormat::Png => vec![formats.png, u32::from(CF_DIB.0)],
        ClipFormat::Unknown => Vec::new(),
    }
}

fn offer(window: HWND, serial: u64, offered: Vec<ClipFormat>) {
    let Some(formats) = state(|win| win.formats) else {
        return;
    };
    if !open_clipboard(window) {
        return;
    }
    // SAFETY: the clipboard is open by this window, which owns it once emptied.
    unsafe {
        if EmptyClipboard().is_err() {
            let _ = CloseClipboard();
            return;
        }
        for format in &offered {
            for windows_format in delayed(*format, formats) {
                // Without data: rendered on demand. The call returns no handle then.
                let _ = SetClipboardData(windows_format, HANDLE(std::ptr::null_mut()));
            }
        }
        for flag in [formats.history, formats.cloud] {
            if let Some(memory) = global(&0u32.to_le_bytes()) {
                let _ = SetClipboardData(flag, HANDLE(memory.0));
            }
        }
        let _ = CloseClipboard();
    }
    state(|win| win.offer = Some((serial, offered)));
}

/// `bytes` in memory the clipboard can take over. `None` when there is no memory.
fn global(bytes: &[u8]) -> Option<HGLOBAL> {
    // SAFETY: a block of that size, written whole while locked.
    unsafe {
        let memory = GlobalAlloc(GMEM_MOVEABLE, bytes.len().max(1)).ok()?;
        let pointer = GlobalLock(memory) as *mut u8;
        if pointer.is_null() {
            return None;
        }
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), pointer, bytes.len());
        let _ = GlobalUnlock(memory);
        Some(memory)
    }
}

/// An application reads `windows_format` of the offer held: ask for it, wait, set it.
fn render(windows_format: u32) {
    let Some((events, formats, offer)) =
        state(|win| (win.events.clone(), win.formats, win.offer.clone()))
    else {
        return;
    };
    let Some((serial, offered)) = offer else {
        return;
    };
    let format = if windows_format == u32::from(CF_UNICODETEXT.0) {
        ClipFormat::Text
    } else if windows_format == formats.png || windows_format == u32::from(CF_DIB.0) {
        ClipFormat::Png
    } else {
        return;
    };
    if !offered.contains(&format) {
        return;
    }
    let (reply, answer) = oneshot::channel();
    if events
        .send(Event::Wanted {
            serial,
            format,
            reply,
        })
        .is_err()
    {
        return;
    }
    let data = match wait(answer) {
        Ok(data) => data,
        Err(error) => {
            eprintln!("{LOG}: nothing to paste: {error}");
            return;
        }
    };
    let bytes = match format {
        ClipFormat::Text => {
            let text = convert::text_to_windows(&String::from_utf8_lossy(&data));
            let mut wide: Vec<u16> = text.encode_utf16().collect();
            wide.push(0);
            wide.iter().flat_map(|unit| unit.to_le_bytes()).collect()
        }
        ClipFormat::Png if windows_format == formats.png => data,
        ClipFormat::Png => match convert::png_to_dib(&data) {
            Ok(dib) => dib,
            Err(error) => {
                eprintln!("{LOG}: the picture cannot be pasted: {error}");
                return;
            }
        },
        ClipFormat::Unknown => return,
    };
    if let Some(memory) = global(&bytes) {
        // SAFETY: inside WM_RENDERFORMAT, the clipboard is open by the reader: the data
        // is set without opening it. Windows owns the memory from then on.
        let _ = unsafe { SetClipboardData(windows_format, HANDLE(memory.0)) };
    }
}

/// The content, up to [`RENDER_PATIENCE`]. The answer is a Tokio channel, which waits
/// without a deadline: a thread of its own waits on it and hands it over.
fn wait(answer: oneshot::Receiver<Data>) -> Data {
    let (sender, receiver) = std_mpsc::sync_channel(1);
    let spawned = std::thread::Builder::new()
        .name("clipboard-wait".to_owned())
        .spawn(move || {
            let _ = sender.send(
                answer
                    .blocking_recv()
                    .unwrap_or_else(|_| Err("the offer was dropped".to_owned())),
            );
        });
    if let Err(error) = spawned {
        return Err(format!("no thread to wait for the clipboard: {error}"));
    }
    receiver
        .recv_timeout(RENDER_PATIENCE)
        .unwrap_or_else(|_| Err("the other machine did not send the clipboard in time".into()))
}

/// What was copied here, in `format`, as it crosses.
fn read(window: HWND, copy: u64, format: ClipFormat, limit: usize) -> Data {
    let formats = state(|win| win.formats).ok_or("the clipboard window went")?;
    if !open_clipboard(window) {
        return Err("another application keeps the clipboard open".into());
    }
    // Only the copy announced, checked with the clipboard open so that nothing can come
    // between the check and the read: a newer copy, or one now marked not to be
    // monitored, is refused.
    // SAFETY: plain reads of the clipboard's state, which is open.
    let current = u64::from(unsafe { GetClipboardSequenceNumber() });
    if current != copy || unsafe { IsClipboardFormatAvailable(formats.exclude) }.is_ok() {
        // SAFETY: opened above.
        let _ = unsafe { CloseClipboard() };
        return Err(super::CHANGED.into());
    }
    let data = match format {
        ClipFormat::Text => {
            bytes_of(u32::from(CF_UNICODETEXT.0), limit.saturating_mul(4)).map(|bytes| {
                let wide: Vec<u16> = bytes
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|pair| u16::from_le_bytes(*pair))
                    .take_while(|unit| *unit != 0)
                    .collect();
                convert::text_to_wire(&String::from_utf16_lossy(&wide)).into_bytes()
            })
        }
        ClipFormat::Png => match bytes_of(formats.png, limit) {
            Ok(png) if convert::is_png(&png) => Ok(png),
            // A DIB is larger than its PNG: its bound is looser, the PNG's is checked.
            _ => bytes_of(u32::from(CF_DIB.0), limit.saturating_mul(8))
                .and_then(|dib| convert::dib_to_png(&dib)),
        },
        ClipFormat::Unknown => Err("not a format this reads".into()),
    };
    // SAFETY: opened above.
    let _ = unsafe { CloseClipboard() };
    data.and_then(|data| {
        if data.len() > limit {
            Err(too_large(limit))
        } else {
            Ok(data)
        }
    })
}

/// The data of `format`, copied out, no more than `limit` bytes. The clipboard is open.
fn bytes_of(format: u32, limit: usize) -> Data {
    // SAFETY: the clipboard is open by the caller; the handle is only read while locked.
    unsafe {
        let handle = GetClipboardData(format).map_err(|_| "not copied in that format")?;
        let memory = HGLOBAL(handle.0);
        let size = GlobalSize(memory);
        if size > limit {
            return Err(too_large(limit));
        }
        let pointer = GlobalLock(memory) as *const u8;
        if pointer.is_null() {
            return Err("the clipboard cannot be read".into());
        }
        let bytes = std::slice::from_raw_parts(pointer, size).to_vec();
        let _ = GlobalUnlock(memory);
        Ok(bytes)
    }
}
