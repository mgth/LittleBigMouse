//! The shared clipboard (docs/multi-pc-etat.md, "Presse-papiers partagé"), as the peer
//! manager runs it: what this machine announces of its clipboard, what it holds of
//! another's, and the transfers between them.
//!
//! Every copy is announced at once to every linked machine that shares, formats only
//! (`ClipboardOffer`); the content crosses when an application on the other side reads
//! it (`ClipboardRequest`, answered by a bulk transfer), and only then. The sharing is
//! independent of the pointer: it neither waits for a crossing nor looks at the input
//! stream, which it never delays (bulk leaves after input, `lbm_net::link`).
//!
//! Files (palier 3) are listed in the offer, folders walked, and fetched range by range
//! into the reception folder the first time an application here reads them; it is handed
//! the paths once they are all there (`clipboard_files`).
//!
//! The system's clipboard is opened only while the user lets it go one way or the other
//! and a machine that shares is linked, and closed as soon as either stops.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use lbm_net::link::LinkSender;
use lbm_net::wire::{Bulk, ClipFile, ClipFormat, Control};
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;
use tokio::time::Instant;

use super::clipboard_files::{self as files, Listed, Piece, Routes};
use crate::clipboard::{too_large, Clipboard, Command, Data, Event, Factory};

/// The default bound on one transfer: a screenshot of several 4K screens fits.
pub const DEFAULT_MAX_BYTES: u64 = 64 * 1024 * 1024;
/// The default bound on the files of one copy, all together.
pub const DEFAULT_FILES_MAX_BYTES: u64 = 4 * 1024 * 1024 * 1024;
/// The largest list of paths read from this machine's clipboard.
const PATHS_LIMIT: usize = 4 * 1024 * 1024;

/// How long a transfer may go without a piece before it is given up: the machine that
/// owes it may be gone without its link noticing yet, and an application here is waiting.
const PATIENCE: Duration = Duration::from_secs(20);

/// The formats the clipboard is shared in, in this build.
const SHARED: [ClipFormat; 3] = [ClipFormat::Text, ClipFormat::Png, ClipFormat::Files];

/// What a listing of the files copied here comes back as.
type Listing = (u64, Vec<ClipFormat>, Result<Vec<Listed>, String>);

/// What the user lets the clipboard do (`multi-pc.json`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Options {
    /// What is copied here is offered to the other machines.
    pub send: bool,
    /// What is copied on the other machines can be pasted here.
    pub receive: bool,
    /// The largest text or picture that crosses, either way.
    pub max_bytes: u64,
    /// The largest set of files that crosses, either way, all together.
    pub files_max_bytes: u64,
    /// Where the files of another machine are fetched to before they are pasted.
    pub reception: PathBuf,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            send: false,
            receive: false,
            max_bytes: DEFAULT_MAX_BYTES,
            files_max_bytes: DEFAULT_FILES_MAX_BYTES,
            reception: files::default_reception(),
        }
    }
}

/// What the state says of the shared clipboard.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct ClipboardState {
    pub send: bool,
    pub receive: bool,
    pub max_bytes: u64,
    #[serde(default)]
    pub files_max_bytes: u64,
    /// Where the files of another machine are fetched to.
    #[serde(default)]
    pub reception: String,
    /// The size of the files being fetched now, all together.
    #[serde(default)]
    pub fetching: Option<u64>,
    /// The system's clipboard is open: the option is on and a machine that shares is
    /// linked.
    pub active: bool,
    /// Why the clipboard cannot be shared here, or why the last transfer failed, until
    /// one succeeds.
    pub problem: Option<String>,
}

/// A linked machine that shares its clipboard (it announced `Clipboard` in `Hello`).
pub(super) struct Peer<'a> {
    pub machine_id: &'a str,
    pub id: u64,
    pub sender: &'a LinkSender,
}

/// What was copied here, as announced.
struct Local {
    serial: u64,
    formats: Vec<ClipFormat>,
    /// With [`ClipFormat::Files`]: what the offer lists, and where each is read from.
    files: Vec<Listed>,
}

/// The offer of another machine that this machine's clipboard holds.
struct Held {
    machine_id: String,
    link: u64,
    /// Its number on that machine.
    serial: u64,
    /// The number the clipboard here knows it by.
    local: u64,
    files: Vec<ClipFile>,
    fetch: FetchState,
}

/// Where the files of the offer held are.
enum FetchState {
    /// Not asked for yet, or a fetch failed and the next paste tries again.
    None,
    Running {
        task: JoinHandle<()>,
        folder: PathBuf,
        /// The applications waiting for the paths.
        waiting: Vec<oneshot::Sender<Data>>,
    },
    /// Fetched: the paths to hand over.
    Done(Vec<PathBuf>),
}

/// A transfer this machine asked for.
struct Incoming {
    machine_id: String,
    link: u64,
    data: Vec<u8>,
    reply: oneshot::Sender<Data>,
    progress: Instant,
}

/// What the sharing waits on, besides the peer manager's own messages.
pub(super) enum Wake {
    Clipboard(Event),
    /// A transfer this machine asked for may have gone quiet for too long.
    Patience(u64),
    /// What was copied here as files, listed (`serial`, the formats copied).
    Listed(u64, Vec<ClipFormat>, Result<Vec<Listed>, String>),
    /// The files of the offer held here under `local`, fetched or not.
    Fetched(u64, Result<Vec<PathBuf>, String>),
}

pub(super) struct Sharing {
    options: Options,
    factory: Factory,
    open: Option<Box<dyn Clipboard>>,
    events_sender: mpsc::UnboundedSender<Event>,
    events: mpsc::UnboundedReceiver<Event>,
    patience_sender: mpsc::UnboundedSender<u64>,
    patience: mpsc::UnboundedReceiver<u64>,
    /// What was last copied here, as announced. `None` once the clipboard holds another
    /// machine's offer, and while the files copied are being listed.
    local: Option<Local>,
    held: Option<Held>,
    /// The last number given to a copy here or an offer held: one counter for both.
    serial: u64,
    incoming: HashMap<u64, Incoming>,
    /// Transfers this machine sends, by the asker and its number for them.
    outgoing: HashMap<(String, u64), JoinHandle<()>>,
    /// The last number given to a transfer this machine asked for; shared with the
    /// fetches, which ask for theirs.
    transfers: Arc<AtomicU64>,
    /// The ranges the fetches wait for.
    routes: Routes,
    listed_sender: mpsc::UnboundedSender<Listing>,
    listed: mpsc::UnboundedReceiver<Listing>,
    fetched_sender: mpsc::UnboundedSender<(u64, Result<Vec<PathBuf>, String>)>,
    fetched: mpsc::UnboundedReceiver<(u64, Result<Vec<PathBuf>, String>)>,
    problem: Option<String>,
    /// Said by a sending task: how its transfer ended, for the state.
    outcome_sender: mpsc::UnboundedSender<Option<String>>,
    outcome: mpsc::UnboundedReceiver<Option<String>>,
}

impl Sharing {
    pub fn new(options: Options, factory: Factory) -> Sharing {
        let (events_sender, events) = mpsc::unbounded_channel();
        let (patience_sender, patience) = mpsc::unbounded_channel();
        let (outcome_sender, outcome) = mpsc::unbounded_channel();
        let (listed_sender, listed) = mpsc::unbounded_channel();
        let (fetched_sender, fetched) = mpsc::unbounded_channel();
        Sharing {
            options,
            factory,
            open: None,
            events_sender,
            events,
            patience_sender,
            patience,
            local: None,
            held: None,
            serial: 0,
            incoming: HashMap::new(),
            outgoing: HashMap::new(),
            transfers: Arc::new(AtomicU64::new(0)),
            routes: Routes::default(),
            listed_sender,
            listed,
            fetched_sender,
            fetched,
            problem: None,
            outcome_sender,
            outcome,
        }
    }

    pub fn state(&self) -> ClipboardState {
        ClipboardState {
            send: self.options.send,
            receive: self.options.receive,
            max_bytes: self.options.max_bytes,
            files_max_bytes: self.options.files_max_bytes,
            reception: self.options.reception.to_string_lossy().into_owned(),
            fetching: self.held.as_ref().and_then(|held| match held.fetch {
                FetchState::Running { .. } => Some(held.files.iter().map(|f| f.size).sum()),
                _ => None,
            }),
            active: self.open.is_some(),
            problem: self.problem.clone(),
        }
    }

    fn limit(&self) -> usize {
        usize::try_from(self.options.max_bytes).unwrap_or(usize::MAX)
    }

    /// The next thing to handle. Cancel-safe. A sending task's outcome is folded in here,
    /// as it only changes the state.
    pub async fn next(&mut self) -> Wake {
        loop {
            tokio::select! {
                Some(event) = self.events.recv() => return Wake::Clipboard(event),
                Some(transfer) = self.patience.recv() => return Wake::Patience(transfer),
                Some((serial, formats, listed)) = self.listed.recv() => {
                    return Wake::Listed(serial, formats, listed)
                }
                Some((local, fetched)) = self.fetched.recv() => return Wake::Fetched(local, fetched),
                Some(outcome) = self.outcome.recv() => self.problem = outcome,
            }
        }
    }

    /// Opens the system's clipboard or closes it, as the options and the machines
    /// linked now call for.
    pub fn reconcile(&mut self, peers: &[Peer<'_>]) {
        let wanted = (self.options.send || self.options.receive) && !peers.is_empty();
        if wanted == self.open.is_some() {
            return;
        }
        if !wanted {
            self.close();
            return;
        }
        match self.factory.open(self.events_sender.clone()) {
            Ok(open) => {
                eprintln!("[lbm-agent] multi-PC: the clipboard is shared");
                self.open = Some(open);
                self.problem = None;
            }
            Err(error) => {
                eprintln!("[lbm-agent] multi-PC: the clipboard cannot be shared: {error}");
                self.problem = Some(error);
            }
        }
    }

    fn close(&mut self) {
        if self.open.take().is_some() {
            eprintln!("[lbm-agent] multi-PC: the clipboard is no longer shared");
        }
        self.local = None;
        self.let_go(None);
        for (_, incoming) in self.incoming.drain() {
            let _ = incoming
                .reply
                .send(Err("the clipboard is no longer shared".into()));
        }
        for (_, task) in self.outgoing.drain() {
            task.abort();
        }
    }

    /// The user changed the options. Turning sending on offers what was copied last.
    pub fn set_options(&mut self, options: Options, peers: &[Peer<'_>]) {
        let was_sending = self.options.send;
        let was_receiving = self.options.receive;
        let (send, receive) = (options.send, options.receive);
        self.options = options;
        if was_receiving && !receive {
            if let Some(held) = self.let_go(Some(peers)) {
                self.command(Command::Withdraw { serial: held.local });
            }
        }
        if was_sending && !send {
            // The other machines may hold what was copied here: they are told it is gone.
            for (_, task) in self.outgoing.drain() {
                task.abort();
            }
            for peer in peers {
                withdraw(peer.sender, self.serial);
            }
        }
        self.reconcile(peers);
        if !was_sending && send {
            if let Some(local) = &self.local {
                for peer in peers {
                    offer(peer.sender, local);
                }
            }
        }
    }

    /// A machine that shares was linked: it is told what was copied here last.
    pub fn linked(&mut self, peer: &Peer<'_>, peers: &[Peer<'_>]) {
        self.reconcile(peers);
        if !self.options.send || self.open.is_none() {
            return;
        }
        if let Some(local) = &self.local {
            offer(peer.sender, local);
        }
    }

    /// The link `id` with `machine_id` ended: what it owed is given up, and an offer of
    /// its held here is let go of — a paste must find nothing rather than wait.
    pub fn unlinked(&mut self, machine_id: &str, id: u64, peers: &[Peer<'_>]) {
        let gone: Vec<u64> = self
            .incoming
            .iter()
            .filter(|(_, i)| i.machine_id == machine_id && i.link == id)
            .map(|(transfer, _)| *transfer)
            .collect();
        for transfer in gone {
            if let Some(incoming) = self.incoming.remove(&transfer) {
                let _ = incoming.reply.send(Err("the other machine left".into()));
            }
        }
        self.outgoing.retain(|(asker, _), task| {
            let theirs = asker == machine_id;
            if theirs {
                task.abort();
            }
            !theirs
        });
        if self
            .held
            .as_ref()
            .is_some_and(|h| h.machine_id == machine_id && h.link == id)
        {
            if let Some(held) = self.let_go(Some(peers)) {
                self.command(Command::Withdraw { serial: held.local });
            }
        }
        self.reconcile(peers);
    }

    /// Lets go of the offer held, if any: a fetch of its files under way stops, the
    /// machine sending them is told when it is still among `peers`, and what was fetched
    /// so far is removed. What a fetch finished stays: a paste may be copying from it.
    fn let_go(&mut self, peers: Option<&[Peer<'_>]>) -> Option<Held> {
        let mut held = self.held.take()?;
        if let FetchState::Running {
            task,
            folder,
            waiting,
        } = std::mem::replace(&mut held.fetch, FetchState::None)
        {
            task.abort();
            for reply in waiting {
                let _ = reply.send(Err("the files are no longer on the clipboard".into()));
            }
            let mut routes = self.routes.lock().unwrap();
            let ranges: Vec<u64> = routes
                .iter()
                .filter(|(_, route)| route.fetch == held.local)
                .map(|(transfer, _)| *transfer)
                .collect();
            for transfer in ranges {
                routes.remove(&transfer);
                if let Some(peer) = peers.and_then(|peers| {
                    peers
                        .iter()
                        .find(|p| p.machine_id == held.machine_id && p.id == held.link)
                }) {
                    send(
                        peer.sender,
                        Control::ClipboardAbort {
                            transfer,
                            reason: "the files are no longer wanted".into(),
                        },
                    );
                }
            }
            drop(routes);
            tokio::task::spawn_blocking(move || {
                let _ = std::fs::remove_dir_all(folder);
            });
        }
        Some(held)
    }

    fn command(&self, command: Command) {
        if let Some(open) = &self.open {
            open.command(command);
        }
    }

    /// What the system's clipboard said.
    pub fn clipboard(&mut self, event: Event, peers: &[Peer<'_>]) {
        match event {
            Event::Changed(formats) => {
                let formats: Vec<ClipFormat> = formats
                    .into_iter()
                    .filter(|format| SHARED.contains(format))
                    .collect();
                // Copied here: no other machine's offer is held any more.
                self.let_go(Some(peers));
                if self.local.is_none() && formats.is_empty() {
                    // Nothing shared before, nothing now: nobody needs telling.
                    return;
                }
                self.serial += 1;
                if formats.contains(&ClipFormat::Files) {
                    // Announced once listed: the other side needs the list to ask.
                    self.local = None;
                    self.list(self.serial, formats);
                    return;
                }
                self.announce(formats, Vec::new(), peers);
            }
            Event::Wanted {
                serial,
                format,
                reply,
            } => self.want(serial, format, reply, peers),
            Event::Failed(error) => {
                eprintln!("[lbm-agent] multi-PC: the clipboard stopped: {error}");
                self.close();
                self.problem = Some(error);
            }
        }
    }

    /// Announces what was copied here under the current serial.
    fn announce(&mut self, formats: Vec<ClipFormat>, files: Vec<Listed>, peers: &[Peer<'_>]) {
        let local = Local {
            serial: self.serial,
            formats,
            files,
        };
        if self.options.send {
            for peer in peers {
                offer(peer.sender, &local);
            }
        }
        self.local = (!local.formats.is_empty()).then_some(local);
    }

    /// Reads the paths copied here and walks them, away from the loop.
    fn list(&self, serial: u64, formats: Vec<ClipFormat>) {
        let (reply, answer) = oneshot::channel();
        self.command(Command::Read {
            format: ClipFormat::Files,
            limit: PATHS_LIMIT,
            reply,
        });
        let max_bytes = self.options.files_max_bytes;
        let listed = self.listed_sender.clone();
        tokio::spawn(async move {
            let result = match answer.await {
                Ok(Ok(paths)) => {
                    let roots = files::paths_of(&paths);
                    files::blocking(move || files::walk(&roots, max_bytes)).await
                }
                Ok(Err(error)) => Err(error),
                Err(_) => Err("the clipboard went".into()),
            };
            let _ = listed.send((serial, formats, result));
        });
    }

    /// The files copied here are listed: announced with them, or without when they cannot
    /// be offered.
    pub fn listed(
        &mut self,
        serial: u64,
        mut formats: Vec<ClipFormat>,
        listed: Result<Vec<Listed>, String>,
        peers: &[Peer<'_>],
    ) {
        if serial != self.serial || self.held.is_some() {
            // Copied over meanwhile.
            return;
        }
        let files = match listed {
            Ok(files) if !files.is_empty() => files,
            Ok(_) => {
                formats.retain(|f| *f != ClipFormat::Files);
                Vec::new()
            }
            Err(reason) => {
                eprintln!("[lbm-agent] multi-PC: the files copied are not offered: {reason}");
                self.problem = Some(reason);
                formats.retain(|f| *f != ClipFormat::Files);
                Vec::new()
            }
        };
        self.announce(formats, files, peers);
    }

    /// The files of the offer held under `local` were fetched, or could not be.
    pub fn fetched(&mut self, local: u64, result: Result<Vec<PathBuf>, String>) {
        let Some(held) = self.held.as_mut().filter(|h| h.local == local) else {
            return;
        };
        let FetchState::Running { waiting, .. } =
            std::mem::replace(&mut held.fetch, FetchState::None)
        else {
            return;
        };
        match result {
            Ok(paths) => {
                eprintln!(
                    "[lbm-agent] multi-PC: {} file(s) fetched from the clipboard of {}",
                    held.files.len(),
                    held.machine_id
                );
                for reply in waiting {
                    let _ = reply.send(Ok(files::lines_of(&paths)));
                }
                held.fetch = FetchState::Done(paths);
                self.problem = None;
            }
            Err(reason) => {
                eprintln!("[lbm-agent] multi-PC: the files could not be fetched: {reason}");
                for reply in waiting {
                    let _ = reply.send(Err(reason.clone()));
                }
                self.problem = Some(reason);
            }
        }
    }

    /// An application here reads the files of the offer held: handed over at once when
    /// fetched, else fetched first.
    fn want_files(&mut self, reply: oneshot::Sender<Data>, peers: &[Peer<'_>]) {
        let Some(held) = self.held.as_mut() else {
            let _ = reply.send(Err("that offer is no longer held".into()));
            return;
        };
        match &mut held.fetch {
            FetchState::Done(paths) => {
                let _ = reply.send(Ok(files::lines_of(paths)));
                return;
            }
            FetchState::Running { waiting, .. } => {
                waiting.push(reply);
                return;
            }
            FetchState::None => {}
        }
        let Some(peer) = peers
            .iter()
            .find(|p| p.machine_id == held.machine_id && p.id == held.link)
        else {
            let _ = reply.send(Err("the other machine left".into()));
            return;
        };
        let folder = files::folder_for(&self.options.reception, held.local);
        let fetch = files::Fetch {
            link: peer.sender.clone(),
            machine_id: held.machine_id.clone(),
            link_id: held.link,
            serial: held.serial,
            local: held.local,
            files: held.files.clone(),
            folder: folder.clone(),
            routes: self.routes.clone(),
            transfers: self.transfers.clone(),
        };
        let fetched = self.fetched_sender.clone();
        let local = held.local;
        let task = tokio::spawn(async move {
            let made = folder.clone();
            let result = match files::blocking(move || {
                std::fs::create_dir_all(&made)
                    .map_err(|error| format!("{} cannot be made: {error}", made.display()))
            })
            .await
            {
                Ok(()) => files::fetch(fetch).await,
                Err(error) => Err(error),
            };
            if result.is_err() {
                let folder = folder.clone();
                let _ = files::blocking(move || {
                    let _ = std::fs::remove_dir_all(folder);
                    Ok(())
                })
                .await;
            }
            let _ = fetched.send((local, result));
        });
        held.fetch = FetchState::Running {
            task,
            folder: files::folder_for(&self.options.reception, local),
            waiting: vec![reply],
        };
    }

    /// An application here reads the offer `serial`: the machine it came from is asked.
    fn want(
        &mut self,
        serial: u64,
        format: ClipFormat,
        reply: oneshot::Sender<Data>,
        peers: &[Peer<'_>],
    ) {
        let Some(held) = self.held.as_ref().filter(|h| h.local == serial) else {
            let _ = reply.send(Err("that offer is no longer held".into()));
            return;
        };
        if format == ClipFormat::Files {
            return self.want_files(reply, peers);
        }
        let Some(peer) = peers
            .iter()
            .find(|p| p.machine_id == held.machine_id && p.id == held.link)
        else {
            let _ = reply.send(Err("the other machine left".into()));
            return;
        };
        let transfer = self.transfers.fetch_add(1, Ordering::Relaxed) + 1;
        self.incoming.insert(
            transfer,
            Incoming {
                machine_id: held.machine_id.clone(),
                link: held.link,
                data: Vec::new(),
                reply,
                progress: Instant::now(),
            },
        );
        send(
            peer.sender,
            Control::ClipboardRequest {
                serial: held.serial,
                format,
                transfer,
                item: None,
                offset: 0,
                length: None,
            },
        );
        self.arm(transfer, PATIENCE);
    }

    fn arm(&self, transfer: u64, after: Duration) {
        let patience = self.patience_sender.clone();
        tokio::spawn(async move {
            tokio::time::sleep(after).await;
            let _ = patience.send(transfer);
        });
    }

    /// A transfer's patience ran out, unless it moved since.
    pub fn patience(&mut self, transfer: u64, peers: &[Peer<'_>]) {
        let Some(incoming) = self.incoming.get(&transfer) else {
            return;
        };
        let quiet = incoming.progress.elapsed();
        if quiet < PATIENCE {
            self.arm(transfer, PATIENCE - quiet);
            return;
        }
        let incoming = self.incoming.remove(&transfer).expect("checked above");
        let reason = "the other machine did not send the clipboard in time".to_owned();
        self.fail(incoming, transfer, reason, peers);
    }

    /// Gives up a transfer this machine asked for: the application waiting is told, and
    /// so is the machine sending it.
    fn fail(&mut self, incoming: Incoming, transfer: u64, reason: String, peers: &[Peer<'_>]) {
        eprintln!("[lbm-agent] multi-PC: a clipboard transfer failed: {reason}");
        if let Some(peer) = peers
            .iter()
            .find(|p| p.machine_id == incoming.machine_id && p.id == incoming.link)
        {
            send(
                peer.sender,
                Control::ClipboardAbort {
                    transfer,
                    reason: reason.clone(),
                },
            );
        }
        let _ = incoming.reply.send(Err(reason.clone()));
        self.problem = Some(reason);
    }

    /// A clipboard message from a linked machine that shares.
    pub fn control(&mut self, from: &Peer<'_>, message: Control) {
        match message {
            Control::ClipboardOffer {
                serial,
                formats,
                files,
            } => {
                if !self.options.receive || self.open.is_none() {
                    return;
                }
                // Offers are queued as they come and may overtake each other: an older
                // one of the same link's is old news.
                if self.held.as_ref().is_some_and(|h| {
                    h.machine_id == from.machine_id && h.link == from.id && serial < h.serial
                }) {
                    return;
                }
                let mut formats: Vec<ClipFormat> = formats
                    .into_iter()
                    .filter(|format| SHARED.contains(format))
                    .collect();
                if formats.contains(&ClipFormat::Files) {
                    if let Err(reason) = files::acceptable(&files, self.options.files_max_bytes) {
                        eprintln!("[lbm-agent] multi-PC: the files offered are refused: {reason}");
                        self.problem = Some(reason);
                        formats.retain(|f| *f != ClipFormat::Files);
                    }
                }
                if formats.is_empty() {
                    // That machine holds nothing it shares any more: what of it is held
                    // here goes too, rather than be pasted from a content that changed.
                    if self
                        .held
                        .as_ref()
                        .is_some_and(|h| h.machine_id == from.machine_id)
                    {
                        if let Some(held) = self.let_go(None) {
                            self.command(Command::Withdraw { serial: held.local });
                        }
                    }
                    return;
                }
                self.let_go(None);
                self.serial += 1;
                self.local = None;
                let files = if formats.contains(&ClipFormat::Files) {
                    files
                } else {
                    Vec::new()
                };
                self.held = Some(Held {
                    machine_id: from.machine_id.to_owned(),
                    link: from.id,
                    serial,
                    local: self.serial,
                    files,
                    fetch: FetchState::None,
                });
                self.command(Command::Offer {
                    serial: self.serial,
                    formats,
                });
            }
            Control::ClipboardRequest {
                serial,
                format,
                transfer,
                item,
                offset,
                length,
            } => match item {
                Some(item) if format == ClipFormat::Files => {
                    self.serve_range(from, serial, transfer, item, offset, length)
                }
                _ => self.serve(from, serial, format, transfer),
            },
            Control::ClipboardAbort { transfer, reason } => {
                if let Some(route) = self.routes.lock().unwrap().get(&transfer) {
                    if route.machine_id == from.machine_id && route.link == from.id {
                        let _ = route.pieces.send(Piece::Abort(if reason.is_empty() {
                            "the other machine gave up".to_owned()
                        } else {
                            reason.clone()
                        }));
                    }
                }
                if let Some(task) = self
                    .outgoing
                    .remove(&(from.machine_id.to_owned(), transfer))
                {
                    task.abort();
                }
                if self
                    .incoming
                    .get(&transfer)
                    .is_some_and(|i| i.machine_id == from.machine_id && i.link == from.id)
                {
                    let incoming = self.incoming.remove(&transfer).expect("checked above");
                    let reason = if reason.is_empty() {
                        "the other machine gave up".to_owned()
                    } else {
                        reason
                    };
                    eprintln!("[lbm-agent] multi-PC: a clipboard transfer failed: {reason}");
                    let _ = incoming.reply.send(Err(reason.clone()));
                    self.problem = Some(reason);
                }
            }
            _ => {}
        }
    }

    /// Another machine reads what was copied here: read it, and send it in a task of its
    /// own, as the network takes it.
    fn serve(&mut self, from: &Peer<'_>, serial: u64, format: ClipFormat, transfer: u64) {
        let sender = from.sender.clone();
        let refuse = |reason: &str| {
            send(
                &sender,
                Control::ClipboardAbort {
                    transfer,
                    reason: reason.to_owned(),
                },
            )
        };
        if !self.options.send || self.open.is_none() {
            return refuse("this machine does not share its clipboard");
        }
        match &self.local {
            Some(local)
                if local.serial == serial
                    && local.formats.contains(&format)
                    && format != ClipFormat::Files => {}
            _ => return refuse("the clipboard changed since"),
        }
        let (reply, answer) = oneshot::channel();
        let limit = self.limit();
        self.command(Command::Read {
            format,
            limit,
            reply,
        });
        self.outgoing.retain(|_, task| !task.is_finished());
        let outcome = self.outcome_sender.clone();
        let task = tokio::spawn(async move {
            let data = answer
                .await
                .unwrap_or_else(|_| Err("the clipboard went".to_owned()));
            let data = data.and_then(|data| {
                if data.len() > limit {
                    Err(too_large(limit))
                } else {
                    Ok(data)
                }
            });
            match data {
                Ok(data) => {
                    if sender.bulk(transfer, &data).await.is_ok() {
                        let _ = outcome.send(None);
                    }
                }
                Err(reason) => {
                    eprintln!("[lbm-agent] multi-PC: the clipboard cannot be sent: {reason}");
                    let _ = sender
                        .control(&Control::ClipboardAbort {
                            transfer,
                            reason: reason.clone(),
                        })
                        .await;
                    let _ = outcome.send(Some(reason));
                }
            }
        });
        self.outgoing
            .insert((from.machine_id.to_owned(), transfer), task);
    }

    /// Another machine reads a range of a file copied here.
    fn serve_range(
        &mut self,
        from: &Peer<'_>,
        serial: u64,
        transfer: u64,
        item: u32,
        offset: u64,
        length: Option<u64>,
    ) {
        let sender = from.sender.clone();
        let source = self
            .local
            .as_ref()
            .filter(|local| local.serial == serial && self.options.send && self.open.is_some())
            .and_then(|local| local.files.get(item as usize))
            .filter(|listed| !listed.entry.folder)
            .map(|listed| listed.source.clone());
        let Some(source) = source else {
            return send(
                &sender,
                Control::ClipboardAbort {
                    transfer,
                    reason: "the clipboard changed since".into(),
                },
            );
        };
        let length = length.unwrap_or(files::RANGE).min(files::RANGE);
        self.outgoing.retain(|_, task| !task.is_finished());
        let task = tokio::spawn(async move {
            match files::blocking(move || files::read_range(&source, offset, length)).await {
                Ok(data) => {
                    let _ = sender.bulk(transfer, &data).await;
                }
                Err(reason) => {
                    eprintln!("[lbm-agent] multi-PC: a file cannot be sent: {reason}");
                    let _ = sender
                        .control(&Control::ClipboardAbort { transfer, reason })
                        .await;
                }
            }
        });
        self.outgoing
            .insert((from.machine_id.to_owned(), transfer), task);
    }

    /// A piece of a transfer this machine asked for.
    pub fn bulk(&mut self, from: &Peer<'_>, piece: Bulk, peers: &[Peer<'_>]) {
        let transfer = piece.transfer;
        if let Some(route) = self.routes.lock().unwrap().get(&transfer) {
            if route.machine_id == from.machine_id && route.link == from.id {
                let _ = route.pieces.send(Piece::Data(piece));
            }
            return;
        }
        let limit = self.limit();
        let Some(incoming) = self
            .incoming
            .get_mut(&transfer)
            .filter(|i| i.machine_id == from.machine_id && i.link == from.id)
        else {
            // A transfer given up meanwhile: its last pieces still come.
            return;
        };
        incoming.progress = Instant::now();
        if incoming.data.len() + piece.data.len() > limit {
            let incoming = self.incoming.remove(&transfer).expect("checked above");
            let reason = too_large(limit);
            return self.fail(incoming, transfer, reason, peers);
        }
        incoming.data.extend_from_slice(&piece.data);
        if piece.last {
            let incoming = self.incoming.remove(&transfer).expect("checked above");
            let _ = incoming.reply.send(Ok(incoming.data));
            self.problem = None;
        }
    }
}

/// Queued in a task of its own: a control message may wait for room on a busy link, and
/// the manager's loop may not.
fn send(sender: &LinkSender, message: Control) {
    let sender = sender.clone();
    tokio::spawn(async move {
        let _ = sender.control(&message).await;
    });
}

fn offer(sender: &LinkSender, local: &Local) {
    send(
        sender,
        Control::ClipboardOffer {
            serial: local.serial,
            formats: local.formats.clone(),
            files: local.files.iter().map(|l| l.entry.clone()).collect(),
        },
    );
}

/// Tells the other machine that nothing copied here is shared any more.
fn withdraw(sender: &LinkSender, serial: u64) {
    send(
        sender,
        Control::ClipboardOffer {
            serial,
            formats: Vec::new(),
            files: Vec::new(),
        },
    );
}
