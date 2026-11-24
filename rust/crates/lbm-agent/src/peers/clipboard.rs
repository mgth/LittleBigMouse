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
//! The system's clipboard is opened only while the user lets it go one way or the other
//! and a machine that shares is linked, and closed as soon as either stops.

use std::collections::HashMap;
use std::time::Duration;

use lbm_net::link::LinkSender;
use lbm_net::wire::{Bulk, ClipFormat, Control};
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;
use tokio::time::Instant;

use crate::clipboard::{too_large, Clipboard, Command, Data, Event, Factory};

/// The default bound on one transfer: a screenshot of several 4K screens fits.
pub const DEFAULT_MAX_BYTES: u64 = 64 * 1024 * 1024;

/// How long a transfer may go without a piece before it is given up: the machine that
/// owes it may be gone without its link noticing yet, and an application here is waiting.
const PATIENCE: Duration = Duration::from_secs(20);

/// The formats the clipboard is shared in, in this build.
const SHARED: [ClipFormat; 2] = [ClipFormat::Text, ClipFormat::Png];

/// What the user lets the clipboard do (`multi-pc.json`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Options {
    /// What is copied here is offered to the other machines.
    pub send: bool,
    /// What is copied on the other machines can be pasted here.
    pub receive: bool,
    /// The largest content that crosses, either way.
    pub max_bytes: u64,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            send: false,
            receive: false,
            max_bytes: DEFAULT_MAX_BYTES,
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

/// The offer of another machine that this machine's clipboard holds.
#[derive(Clone, Debug, PartialEq)]
struct Held {
    machine_id: String,
    link: u64,
    /// Its number on that machine.
    serial: u64,
    /// The number the clipboard here knows it by.
    local: u64,
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
}

pub(super) struct Sharing {
    options: Options,
    factory: Factory,
    open: Option<Box<dyn Clipboard>>,
    events_sender: mpsc::UnboundedSender<Event>,
    events: mpsc::UnboundedReceiver<Event>,
    patience_sender: mpsc::UnboundedSender<u64>,
    patience: mpsc::UnboundedReceiver<u64>,
    /// What was last copied here, as announced: its number and formats. `None` once the
    /// clipboard holds another machine's offer.
    local: Option<(u64, Vec<ClipFormat>)>,
    held: Option<Held>,
    /// The last number given to a copy here or an offer held: one counter for both.
    serial: u64,
    incoming: HashMap<u64, Incoming>,
    /// Transfers this machine sends, by the asker and its number for them.
    outgoing: HashMap<(String, u64), JoinHandle<()>>,
    transfer: u64,
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
            transfer: 0,
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
        self.held = None;
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
        self.options = options;
        if was_receiving && !options.receive {
            if let Some(held) = self.held.take() {
                self.command(Command::Withdraw { serial: held.local });
            }
        }
        if was_sending && !options.send {
            // The other machines may hold what was copied here: they are told it is gone.
            for (_, task) in self.outgoing.drain() {
                task.abort();
            }
            for peer in peers {
                offer(peer.sender, self.serial, Vec::new());
            }
        }
        self.reconcile(peers);
        if !was_sending && options.send {
            if let Some((serial, formats)) = &self.local {
                for peer in peers {
                    offer(peer.sender, *serial, formats.clone());
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
        if let Some((serial, formats)) = &self.local {
            offer(peer.sender, *serial, formats.clone());
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
            let held = self.held.take().expect("checked above");
            self.command(Command::Withdraw { serial: held.local });
        }
        self.reconcile(peers);
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
                self.held = None;
                if self.local.is_none() && formats.is_empty() {
                    // Nothing shared before, nothing now: nobody needs telling.
                    return;
                }
                self.serial += 1;
                self.local = (!formats.is_empty()).then(|| (self.serial, formats.clone()));
                if self.options.send {
                    for peer in peers {
                        offer(peer.sender, self.serial, formats.clone());
                    }
                }
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
        let Some(peer) = peers
            .iter()
            .find(|p| p.machine_id == held.machine_id && p.id == held.link)
        else {
            let _ = reply.send(Err("the other machine left".into()));
            return;
        };
        self.transfer += 1;
        let transfer = self.transfer;
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
            Control::ClipboardOffer { serial, formats } => {
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
                let formats: Vec<ClipFormat> = formats
                    .into_iter()
                    .filter(|format| SHARED.contains(format))
                    .collect();
                if formats.is_empty() {
                    // That machine holds nothing it shares any more: what of it is held
                    // here goes too, rather than be pasted from a content that changed.
                    if self
                        .held
                        .as_ref()
                        .is_some_and(|h| h.machine_id == from.machine_id)
                    {
                        let held = self.held.take().expect("checked above");
                        self.command(Command::Withdraw { serial: held.local });
                    }
                    return;
                }
                self.serial += 1;
                self.local = None;
                self.held = Some(Held {
                    machine_id: from.machine_id.to_owned(),
                    link: from.id,
                    serial,
                    local: self.serial,
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
            } => self.serve(from, serial, format, transfer),
            Control::ClipboardAbort { transfer, reason } => {
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
            Some((current, formats)) if *current == serial && formats.contains(&format) => {}
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

    /// A piece of a transfer this machine asked for.
    pub fn bulk(&mut self, from: &Peer<'_>, piece: Bulk, peers: &[Peer<'_>]) {
        let transfer = piece.transfer;
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

fn offer(sender: &LinkSender, serial: u64, formats: Vec<ClipFormat>) {
    send(sender, Control::ClipboardOffer { serial, formats });
}
