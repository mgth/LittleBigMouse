//! The clipboard under Wayland, through `ext-data-control-v1`: the protocol a clipboard
//! manager uses, which sees and sets the selection without a window or the focus. KWin
//! offers it to every client (checked on Plasma 6.8: `ext_data_control_manager_v1`
//! version 1, and no `zwlr_data_control_manager_v1`); GNOME offers neither, and there
//! the sharing says so.
//!
//! A connection of its own on a thread of its own, which sleeps in `poll` on the
//! connection and a wake-up pipe: nothing happens unless the compositor or the sharing
//! says something. The content of a selection is read, or written for an application,
//! on a short-lived thread per transfer, since a pipe may take its time.
//!
//! Only the regular selection is shared: the primary selection (the middle click) has no
//! counterpart on Windows.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::os::fd::{AsFd, AsRawFd, OwnedFd};
use std::os::unix::net::UnixStream;
use std::sync::mpsc as std_mpsc;
use std::thread::JoinHandle;

use lbm_net::wire::ClipFormat;
use tokio::sync::{mpsc, oneshot};
use wayland_client::backend::{ObjectId, WaylandError};
use wayland_client::globals::{registry_queue_init, GlobalListContents};
use wayland_client::protocol::{wl_registry, wl_seat::WlSeat};
use wayland_client::{event_created_child, Connection, Dispatch, EventQueue, Proxy, QueueHandle};
use wayland_protocols::ext::data_control::v1::client::{
    ext_data_control_device_v1::{self, ExtDataControlDeviceV1},
    ext_data_control_manager_v1::ExtDataControlManagerV1,
    ext_data_control_offer_v1::{self, ExtDataControlOfferV1},
    ext_data_control_source_v1::{self, ExtDataControlSourceV1},
};

use super::{convert, Clipboard, Command, Data, Event};

const LOG: &str = "[lbm-agent] clipboard (Wayland)";

/// Offered beside what another machine holds, so that the selection it makes is known
/// for what it is when the compositor announces it back.
const MARKER: &str = "application/x-littlebigmouse-remote";
/// What KDE's password managers (KeePassXC among them) offer beside a secret: such a
/// selection is never announced (docs/multi-pc-etat.md, "Garde-fous").
const SECRET: &str = "x-kde-passwordManagerHint";

/// The text types read, best first: UTF-8 is the only encoding read.
const TEXT_READ: [&str; 3] = ["text/plain;charset=utf-8", "UTF8_STRING", "text/plain"];
/// The text types offered: UTF-8 under every name applications ask for it by.
const TEXT_OFFERED: [&str; 5] = [
    "text/plain;charset=utf-8",
    "UTF8_STRING",
    "text/plain",
    "STRING",
    "TEXT",
];
const PNG: &str = "image/png";
const URI_LIST: &str = "text/uri-list";
const GNOME_FILES: &str = "x-special/gnome-copied-files";

/// The format a MIME type of an offer is read as, if it is one shared.
fn format_of(mime: &str) -> Option<ClipFormat> {
    if TEXT_READ.contains(&mime) {
        Some(ClipFormat::Text)
    } else if mime == PNG {
        Some(ClipFormat::Png)
    } else if mime == URI_LIST {
        Some(ClipFormat::Files)
    } else {
        None
    }
}

/// What a selection offering `mimes` is shared as: nothing for a secret.
fn formats_of(mimes: &[String]) -> Vec<ClipFormat> {
    if mimes.iter().any(|m| m == SECRET) {
        return Vec::new();
    }
    let mut formats = Vec::new();
    for format in mimes.iter().filter_map(|m| format_of(m)) {
        if !formats.contains(&format) {
            formats.push(format);
        }
    }
    formats
}

/// The MIME type to read `format` as, among `mimes`.
fn mime_for(format: ClipFormat, mimes: &[String]) -> Option<&str> {
    let wanted: &[&str] = match format {
        ClipFormat::Text => &TEXT_READ,
        ClipFormat::Png => &[PNG],
        ClipFormat::Files => &[URI_LIST],
        ClipFormat::Unknown => &[],
    };
    wanted
        .iter()
        .find(|w| mimes.iter().any(|m| m == *w))
        .copied()
}

fn mimes_offered(format: ClipFormat) -> &'static [&'static str] {
    match format {
        ClipFormat::Text => &TEXT_OFFERED,
        ClipFormat::Png => &[PNG],
        ClipFormat::Files => &[URI_LIST, GNOME_FILES],
        ClipFormat::Unknown => &[],
    }
}

pub(super) fn open(events: mpsc::UnboundedSender<Event>) -> Result<Box<dyn Clipboard>, String> {
    let connection = Connection::connect_to_env()
        .map_err(|error| format!("no Wayland compositor to share the clipboard with: {error}"))?;
    let (globals, queue) = registry_queue_init::<State>(&connection)
        .map_err(|error| format!("the Wayland registry cannot be read: {error}"))?;
    let handle = queue.handle();
    let manager: ExtDataControlManagerV1 = globals.bind(&handle, 1..=1, ()).map_err(|_| {
        "the compositor does not let the clipboard be shared (no ext-data-control-v1)".to_owned()
    })?;
    let seat: WlSeat = globals
        .bind(&handle, 1..=1, ())
        .map_err(|_| "the compositor has no seat".to_owned())?;
    let device = manager.get_data_device(&seat, &handle, ());
    let (wake_read, wake_write) =
        UnixStream::pair().map_err(|error| format!("no wake-up pipe: {error}"))?;
    wake_read
        .set_nonblocking(true)
        .map_err(|error| format!("no wake-up pipe: {error}"))?;
    let (commands, commands_rx) = std_mpsc::channel();
    let state = State {
        events,
        manager,
        device,
        mimes: HashMap::new(),
        selection: None,
        copy: 0,
        source: None,
    };
    let thread = std::thread::Builder::new()
        .name("clipboard".to_owned())
        .spawn(move || run(connection, queue, state, commands_rx, wake_read))
        .map_err(|error| format!("no clipboard thread: {error}"))?;
    Ok(Box::new(Open {
        commands: Some(commands),
        wake: wake_write,
        thread: Some(thread),
    }))
}

struct Open {
    commands: Option<std_mpsc::Sender<Command>>,
    wake: UnixStream,
    thread: Option<JoinHandle<()>>,
}

impl Clipboard for Open {
    fn command(&self, command: Command) {
        if let Some(commands) = &self.commands {
            if commands.send(command).is_ok() {
                let _ = (&self.wake).write(&[1]);
            }
        }
    }
}

impl Drop for Open {
    fn drop(&mut self) {
        // The thread sees the channel closed at its next wake-up.
        self.commands = None;
        let _ = (&self.wake).write(&[1]);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

struct State {
    events: mpsc::UnboundedSender<Event>,
    manager: ExtDataControlManagerV1,
    device: ExtDataControlDeviceV1,
    /// The MIME types of each offer, as the compositor lists them.
    mimes: HashMap<ObjectId, Vec<String>>,
    /// The selection now, when it is another application's: its offer and types.
    selection: Option<(ExtDataControlOfferV1, Vec<String>)>,
    /// The number of the last selection the compositor announced, whoever made it:
    /// what [`Event::Changed`] and [`Command::Read`] name a copy by.
    copy: u64,
    /// The selection this machine made for another machine's offer: the source, the
    /// offer's number, and the formats by MIME type.
    source: Option<(ExtDataControlSourceV1, u64, Vec<ClipFormat>)>,
}

impl State {
    fn command(&mut self, command: Command, handle: &QueueHandle<State>, connection: &Connection) {
        match command {
            Command::Offer { serial, formats } => {
                let source = self.manager.create_data_source(handle, ());
                for format in &formats {
                    for mime in mimes_offered(*format) {
                        source.offer((*mime).to_owned());
                    }
                }
                source.offer(MARKER.to_owned());
                self.device.set_selection(Some(&source));
                if let Some((previous, _, _)) = self.source.replace((source, serial, formats)) {
                    previous.destroy();
                }
            }
            Command::Withdraw { serial } => {
                if self.source.as_ref().is_some_and(|(_, s, _)| *s == serial) {
                    let (source, _, _) = self.source.take().expect("checked above");
                    self.device.set_selection(None);
                    source.destroy();
                }
            }
            Command::Read {
                copy,
                format,
                limit,
                reply,
            } => {
                // Only the selection announced, still in a format it was announced in:
                // a newer one, a secret since, or this machine's own offer is refused.
                let Some((offer, mimes)) = self
                    .selection
                    .as_ref()
                    .filter(|_| self.copy == copy)
                    .map(|(offer, mimes)| (offer, mimes))
                else {
                    let _ = reply.send(Err(super::CHANGED.into()));
                    return;
                };
                if !formats_of(mimes).contains(&format) {
                    let _ = reply.send(Err(super::CHANGED.into()));
                    return;
                }
                let Some(mime) = mime_for(format, mimes) else {
                    let _ = reply.send(Err("not copied in that format".into()));
                    return;
                };
                let (reader, writer) = match std::io::pipe() {
                    Ok(pipe) => pipe,
                    Err(error) => {
                        let _ = reply.send(Err(format!("no pipe: {error}")));
                        return;
                    }
                };
                offer.receive(mime.to_owned(), writer.as_fd());
                // The descriptor travels with the request: sent, this end is closed, so
                // that the reader sees the end once the application closes its own.
                let _ = connection.flush();
                drop(writer);
                spawn_read(reader, format, limit, reply);
            }
        }
    }
}

/// Reads what the application that owns the selection writes, on a thread of its own.
fn spawn_read(
    mut reader: std::io::PipeReader,
    format: ClipFormat,
    limit: usize,
    reply: oneshot::Sender<Data>,
) {
    let spawned = std::thread::Builder::new()
        .name("clipboard-read".to_owned())
        .spawn(move || {
            let mut data = Vec::new();
            let read = (&mut reader).take(limit as u64 + 1).read_to_end(&mut data);
            let result = match read {
                Err(error) => Err(format!("the clipboard cannot be read: {error}")),
                Ok(_) if data.len() > limit => Err(super::too_large(limit)),
                Ok(_) => match format {
                    ClipFormat::Text => {
                        Ok(convert::text_to_wire(&String::from_utf8_lossy(&data)).into_bytes())
                    }
                    ClipFormat::Files => {
                        let paths = convert::paths_from_uri_list(&data);
                        if paths.is_empty() {
                            Err("no file of this machine was copied".to_owned())
                        } else {
                            Ok(crate::peers::clipboard_files::pack_paths(&paths))
                        }
                    }
                    _ => Ok(data),
                },
            };
            let _ = reply.send(result);
        });
    if let Err(error) = spawned {
        eprintln!("{LOG}: no thread to read the clipboard: {error}");
    }
}

/// Writes what another machine sent into the pipe an application reads, on a thread of
/// its own: the content is asked for first, and may take a while to come.
fn spawn_write(fd: OwnedFd, mime: String, answer: oneshot::Receiver<Data>) {
    let spawned = std::thread::Builder::new()
        .name("clipboard-write".to_owned())
        .spawn(move || {
            let mut file = std::fs::File::from(fd);
            match answer.blocking_recv() {
                Ok(Ok(data)) => {
                    let data = if mime == URI_LIST || mime == GNOME_FILES {
                        let paths = crate::peers::clipboard_files::unpack_paths(&data);
                        if mime == URI_LIST {
                            convert::uri_list(&paths).into_bytes()
                        } else {
                            convert::gnome_copied_files(&paths).into_bytes()
                        }
                    } else {
                        data
                    };
                    let _ = file.write_all(&data);
                }
                Ok(Err(error)) => eprintln!("{LOG}: nothing to paste: {error}"),
                Err(_) => {}
            }
            // Dropped: the application reads the end of what it gets.
        });
    if let Err(error) = spawned {
        eprintln!("{LOG}: no thread to write the clipboard: {error}");
    }
}

fn run(
    connection: Connection,
    mut queue: EventQueue<State>,
    mut state: State,
    commands: std_mpsc::Receiver<Command>,
    mut wake: UnixStream,
) {
    let handle = queue.handle();
    let failed = |state: &State, error: String| {
        eprintln!("{LOG}: {error}");
        let _ = state.events.send(Event::Failed(error));
    };
    loop {
        // The wake-up first, then every command waiting, then whatever the compositor
        // said. In that order: a command sent after the queue was found empty has left a
        // byte in the pipe that the next poll sees, where draining after the queue would
        // swallow it and sleep with the command waiting.
        let mut drained = [0u8; 64];
        while matches!(wake.read(&mut drained), Ok(n) if n > 0) {}
        loop {
            match commands.try_recv() {
                Ok(command) => state.command(command, &handle, &connection),
                Err(std_mpsc::TryRecvError::Empty) => break,
                Err(std_mpsc::TryRecvError::Disconnected) => {
                    if let Some((source, _, _)) = state.source.take() {
                        source.destroy();
                    }
                    state.device.destroy();
                    let _ = connection.flush();
                    return;
                }
            }
        }
        if let Err(error) = queue.dispatch_pending(&mut state) {
            return failed(&state, format!("the compositor connection failed: {error}"));
        }
        if state.events.is_closed() {
            return;
        }
        if let Err(error) = connection.flush().or_else(would_block) {
            return failed(&state, format!("the compositor connection failed: {error}"));
        }
        let Some(guard) = queue.prepare_read() else {
            continue;
        };
        let mut fds = [
            libc::pollfd {
                fd: guard.connection_fd().as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            },
            libc::pollfd {
                fd: wake.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            },
        ];
        // SAFETY: two valid descriptors, for as long as the call.
        let polled = unsafe { libc::poll(fds.as_mut_ptr(), fds.len() as libc::nfds_t, -1) };
        if polled < 0 {
            let error = std::io::Error::last_os_error();
            if error.kind() != std::io::ErrorKind::Interrupted {
                return failed(&state, format!("poll failed: {error}"));
            }
            continue;
        }
        if fds[0].revents != 0 {
            if let Err(error) = guard.read().map(|_| ()).or_else(would_block) {
                return failed(&state, format!("the compositor connection failed: {error}"));
            }
        }
    }
}

fn would_block(error: WaylandError) -> Result<(), WaylandError> {
    match error {
        WaylandError::Io(io) if io.kind() == std::io::ErrorKind::WouldBlock => Ok(()),
        error => Err(error),
    }
}

impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for State {
    fn event(
        _: &mut Self,
        _: &wl_registry::WlRegistry,
        _: wl_registry::Event,
        _: &GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<WlSeat, ()> for State {
    fn event(
        _: &mut Self,
        _: &WlSeat,
        _: <WlSeat as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ExtDataControlManagerV1, ()> for State {
    fn event(
        _: &mut Self,
        _: &ExtDataControlManagerV1,
        _: <ExtDataControlManagerV1 as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ExtDataControlDeviceV1, ()> for State {
    fn event(
        state: &mut Self,
        _: &ExtDataControlDeviceV1,
        event: ext_data_control_device_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            ext_data_control_device_v1::Event::DataOffer { id } => {
                state.mimes.insert(id.id(), Vec::new());
            }
            ext_data_control_device_v1::Event::Selection { id } => {
                if let Some((previous, _)) = state.selection.take() {
                    previous.destroy();
                }
                state.copy += 1;
                let copy = state.copy;
                let Some(offer) = id else {
                    // The clipboard is empty: nothing to share any more.
                    if state.source.is_none() {
                        let _ = state.events.send(Event::Changed {
                            copy,
                            formats: Vec::new(),
                        });
                    }
                    return;
                };
                let mimes = state.mimes.remove(&offer.id()).unwrap_or_default();
                if mimes.iter().any(|m| m == MARKER) {
                    // The selection this machine made for another one: not news.
                    offer.destroy();
                    return;
                }
                let formats = formats_of(&mimes);
                state.selection = Some((offer, mimes));
                let _ = state.events.send(Event::Changed { copy, formats });
            }
            ext_data_control_device_v1::Event::PrimarySelection { id: Some(offer) } => {
                state.mimes.remove(&offer.id());
                offer.destroy();
            }
            ext_data_control_device_v1::Event::Finished => {
                let _ = state.events.send(Event::Failed(
                    "the compositor ended the clipboard device".into(),
                ));
            }
            _ => {}
        }
    }

    event_created_child!(State, ExtDataControlDeviceV1, [
        ext_data_control_device_v1::EVT_DATA_OFFER_OPCODE => (ExtDataControlOfferV1, ()),
    ]);
}

impl Dispatch<ExtDataControlOfferV1, ()> for State {
    fn event(
        state: &mut Self,
        offer: &ExtDataControlOfferV1,
        event: ext_data_control_offer_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let ext_data_control_offer_v1::Event::Offer { mime_type } = event {
            if let Some(mimes) = state.mimes.get_mut(&offer.id()) {
                mimes.push(mime_type);
            }
        }
    }
}

impl Dispatch<ExtDataControlSourceV1, ()> for State {
    fn event(
        state: &mut Self,
        source: &ExtDataControlSourceV1,
        event: ext_data_control_source_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let ours = state
            .source
            .as_ref()
            .filter(|(s, _, _)| s == source)
            .map(|(_, serial, formats)| (*serial, formats.clone()));
        match event {
            ext_data_control_source_v1::Event::Send { mime_type, fd } => {
                let Some((serial, formats)) = ours else {
                    return;
                };
                let Some(format) = formats
                    .iter()
                    .copied()
                    .find(|f| mimes_offered(*f).contains(&mime_type.as_str()))
                else {
                    // The marker, or a type never offered: the end of the pipe says it all.
                    return;
                };
                let (reply, answer) = oneshot::channel();
                let _ = state.events.send(Event::Wanted {
                    serial,
                    format,
                    reply,
                });
                spawn_write(fd, mime_type, answer);
            }
            ext_data_control_source_v1::Event::Cancelled => {
                // Another application took the clipboard: its selection follows.
                if ours.is_some() {
                    state.source = None;
                }
                source.destroy();
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mimes(list: &[&str]) -> Vec<String> {
        list.iter().map(|m| (*m).to_owned()).collect()
    }

    #[test]
    fn a_selection_is_shared_in_the_formats_it_offers_and_a_secret_in_none() {
        assert_eq!(
            formats_of(&mimes(&[
                "text/html",
                "text/plain",
                "UTF8_STRING",
                "image/png"
            ])),
            [ClipFormat::Text, ClipFormat::Png]
        );
        assert_eq!(formats_of(&mimes(&["image/jpeg"])), []);
        assert_eq!(
            formats_of(&mimes(&["text/uri-list", "text/plain"])),
            [ClipFormat::Files, ClipFormat::Text]
        );
        assert_eq!(
            formats_of(&mimes(&["text/plain;charset=utf-8", SECRET])),
            []
        );
    }

    #[test]
    fn text_is_read_as_utf8_first() {
        let offered = mimes(&["TEXT", "text/plain", "text/plain;charset=utf-8"]);
        assert_eq!(
            mime_for(ClipFormat::Text, &offered),
            Some("text/plain;charset=utf-8")
        );
        assert_eq!(mime_for(ClipFormat::Png, &offered), None);
        assert_eq!(mime_for(ClipFormat::Text, &mimes(&["STRING"])), None);
    }

    #[test]
    fn what_is_offered_reads_back_as_the_same_formats() {
        for format in [ClipFormat::Text, ClipFormat::Png, ClipFormat::Files] {
            let offered = mimes(mimes_offered(format));
            assert_eq!(formats_of(&offered), [format]);
        }
    }
}
