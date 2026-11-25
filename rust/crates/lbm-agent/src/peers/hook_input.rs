//! The agent's end of the hook's input endpoint (`lbm_ipc::input::input_endpoint_beside`):
//! one connection, both directions, in a task of its own.
//!
//! - **Remote**: the batches a master sends arrive from the network and are written to
//!   the hook, which injects them. The connection is made at the first batch and made
//!   again after a failure; batches that find no connection are dropped, never queued
//!   for later — input replayed late is input nobody typed. When the link they came on
//!   ends, or a batch finds the queue full, the connection is closed ([`Sink::release`]):
//!   that is how the hook learns to release whatever the stream left pressed. The hook
//!   ignores a new connection's batches until an `Enter`; after a full queue the master
//!   is told so (`InputReset`, sent by the link's task). A batch the hook did not get
//!   ends the session the same way: one dropped for want of a connection, or one of a
//!   connection that ended for any other cause (the hook gone, a write that failed or
//!   took too long). Every batch is numbered when queued, and every link's task hears
//!   the number of the latest batch so lost ([`Sink::lost`]); the one whose master's
//!   session it belongs to says `InputReset`. A batch that crosses (an `Enter`) tries
//!   to connect at once, even within [`RETRY_AFTER`] of a failure: a hook that
//!   restarted is reached by the next crossing, not one a second later.
//! - **Master**: while a remote is linked ([`HookInput::set_target`]), the connection is
//!   kept, and every batch the hook writes to it is relayed to that remote's link with
//!   `try_input`, never a wait. A connection relays to one link for its whole life.
//!   Its end — the remote going away, another remote chosen, the hook gone, a batch
//!   the link has no room for, or the remote saying it ended the session
//!   ([`HookInput::restart`]) — is the end of the hop: the connection is closed, which
//!   brings the master's pointer home (docs/multi-pc.md), and the link it relayed to is
//!   told `Leave`, which makes the remote's hook release what the stream left pressed.
//!   No batch of a new connection is relayed before that `Leave` is queued.
//!
//! A batch is never dropped in the middle of a session: it may be a release, and a
//! remote that misses the release of Ctrl types every later key with Ctrl held. Where a
//! queue has no room, the session ends instead, on both sides.
//!
//! - **Several remotes** ([`HookInput::set_hosts`], docs/multi-pc.md "Several remotes at
//!   once"): with a hook that names the machine of each crossing, one connection relays
//!   to every drivable remote. Each batch is cut at its `Host` markers ([`by_host`]) and
//!   each run goes to its machine's target, without the marker. The connection outlives
//!   remotes linking and leaving; the end of a hop is per machine. A run its machine
//!   has no room or no target for breaks that machine's stream until its next `Enter`:
//!   its runs are dropped and it is told `Leave` with a send that waits; if the pointer
//!   is on it, the connection is closed and made again at once, which brings the
//!   pointer home ([`HookInput::restart`] does the same for a remote's `InputReset`).
//!   A batch that does not walk ends the connection.
//! - **Driving** ([`HookInput::driving`]): on a master, whether the remote the stream
//!   is relayed to holds the pointer now — an `Enter` relayed and no `Leave` since, nor
//!   the end of the connection — and, with several remotes, which one. Read off the
//!   batches as they pass ([`crossing`]), for the state's indicator of the machine that
//!   receives the input.
//!
//! The hook serves one client, the newest. A machine that is both a master and a remote
//! at once shares this one connection between the two; nothing more is attempted for
//! that case yet.

use std::collections::{BTreeMap, HashSet};
use std::io;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use lbm_ipc::input::{
    by_host, crossing, encode, read_batch, without_locks, write_batch, InputEvent,
};
use lbm_net::link::LinkSender;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::{mpsc, oneshot, watch};
use tokio::task::JoinHandle;
use tokio::time::Instant;

/// Batches waiting to be written to the hook: a few dozen milliseconds of a fast mouse.
/// Beyond it the hook is not keeping up: the stream is ended rather than queued further.
const QUEUE: usize = 64;

/// How long one connection attempt may take: a bound that is never reached in the
/// usual case, both platforms answering at once (see [`connect`]).
const CONNECT_TIMEOUT: Duration = Duration::from_millis(250);

/// After a failed attempt, batches are dropped for this long before the next one: a
/// hook that is not there must not be asked a thousand times a second.
const RETRY_AFTER: Duration = Duration::from_secs(1);

/// A hook that takes longer than this to read a batch is not injecting anything useful:
/// the connection is closed, which also makes it release what it held.
const WRITE_TIMEOUT: Duration = Duration::from_secs(1);

enum Message {
    Batch {
        /// Its number, from [`Sink::try_batch`].
        number: u64,
        bytes: Vec<u8>,
        /// It carries an `Enter`: a connection is tried at once if there is none.
        crosses: bool,
    },
    Release,
}

/// The link the hook's batches go to, and whether the remote's hook reads the lock
/// keys' state (`lbm_ipc::input::LOCKS_CAPABILITY`): one that does not fails a whole
/// batch on it, so it is cut out of the batches relayed there.
#[derive(Clone)]
pub struct Target {
    pub link: LinkSender,
    pub reads_locks: bool,
    /// Which relay this is: the id of the link with the remote's agent, or of the link
    /// with its service. A machine whose relay changes while the pointer is on it ends
    /// that session on the old relay, and the new one gets only a fresh `Enter`.
    pub route: u64,
}

/// Where a master's hook stream goes: one remote, as with a hook that names no machine,
/// or every drivable one by machine id.
#[derive(Clone)]
enum Relay {
    One(Option<Target>),
    Hosts(BTreeMap<String, Target>),
}

/// The value behind the targets' watch. `epoch` counts [`HookInput::restart`]s: the
/// targets of several remotes change without ending the hop, a restart always ends it.
#[derive(Clone)]
struct Targets {
    relay: Relay,
    epoch: u64,
}

impl Targets {
    fn relaying(&self) -> bool {
        match &self.relay {
            Relay::One(target) => target.is_some(),
            Relay::Hosts(hosts) => !hosts.is_empty(),
        }
    }
}

/// Where the network side hands batches to the hook, and the end of a stream.
pub struct Sink {
    messages: mpsc::Sender<Message>,
    /// The number the last batch queued was given, by any sink.
    numbered: Arc<AtomicU64>,
    /// The number of the latest batch the hook did not get, or got on a connection
    /// that ended since — a [`Sink::release`] of any link's included.
    lost: watch::Receiver<u64>,
}

impl Sink {
    /// Queues a batch for the hook if there is room, and returns the number it was
    /// given: numbers grow in the order batches are queued. When there is no room the
    /// batch is lost, and the caller must end the stream ([`Sink::release`]): what
    /// follows it cannot be injected as if it had not been. `crosses`: the batch
    /// carries an `Enter`.
    pub fn try_batch(&self, batch: Vec<u8>, crosses: bool) -> Option<u64> {
        let number = self.numbered.fetch_add(1, Ordering::Relaxed) + 1;
        self.messages
            .try_send(Message::Batch {
                number,
                bytes: batch,
                crosses,
            })
            .ok()
            .map(|()| number)
    }

    /// The stream ended: closes the connection after the batches already queued, so
    /// the hook releases what they left pressed. Waits for room in the queue, so that
    /// it can neither be lost nor overtake those batches. Every batch written on that
    /// connection counts as lost ([`Sink::lost`]): the connection is shared, and the
    /// session of another master on it ends with it.
    pub async fn release(&self) {
        let _ = self.messages.send(Message::Release).await;
    }

    /// Resolves when a batch was lost since the last time it did (or since this sink
    /// was made), with the number of the latest one: dropped for want of a connection,
    /// or written on a connection that ended since (the hook went, a write failed or
    /// took too long, a link's stream ended). Every batch up to that number is the
    /// hook's no more: a new connection's are ignored until an `Enter`. Never resolves
    /// once the connection's task is gone.
    pub async fn lost(&mut self) -> u64 {
        if self.lost.changed().await.is_err() {
            std::future::pending::<()>().await;
        }
        *self.lost.borrow_and_update()
    }
}

/// The connection's task, and its two inputs.
pub struct HookInput {
    /// This machine's hook reads the lock keys' state: until it says so, the `Locks` of
    /// a master's batches are cut out of what is written to it.
    reads_locks: Arc<AtomicBool>,
    messages: mpsc::Sender<Message>,
    numbered: Arc<AtomicU64>,
    lost: watch::Sender<u64>,
    target: watch::Sender<Targets>,
    driving: watch::Receiver<Driving>,
}

/// The value behind [`HookInput::driving`]: whether the remote is driven, which one
/// when the stream names them, and the number of the connection that said so — a relay
/// that outlived its connection (aborted, but running on another thread that very
/// moment) cannot say it any more.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Driving {
    connection: u64,
    pub driven: bool,
    /// The machine the pointer is on, with several remotes; `None` with one.
    pub host: Option<String>,
}

type DrivingSender = std::sync::Arc<watch::Sender<Driving>>;

/// Says what connection `connection` saw, unless another one came since.
fn drive(driving: &watch::Sender<Driving>, connection: u64, driven: bool) {
    drive_host(driving, connection, driven, None);
}

/// [`drive`], naming the machine driven.
fn drive_host(driving: &watch::Sender<Driving>, connection: u64, driven: bool, host: Option<&str>) {
    driving.send_if_modified(|now| {
        if now.connection != connection || (now.driven == driven && now.host.as_deref() == host) {
            return false;
        }
        now.driven = driven;
        now.host = host.map(str::to_owned);
        true
    });
}

impl HookInput {
    /// The connection to the input endpoint `endpoint`; `None` when there is no hook to
    /// talk to, and then every batch is dropped.
    pub fn spawn(endpoint: Option<String>) -> HookInput {
        let (messages, inbox) = mpsc::channel(QUEUE);
        let (target, targets) = watch::channel(Targets {
            relay: Relay::One(None),
            epoch: 0,
        });
        let (lost, _) = watch::channel(0);
        let (driving_tx, driving) = watch::channel(Driving::default());
        let reads_locks = Arc::new(AtomicBool::new(false));
        tokio::spawn(run(
            endpoint,
            inbox,
            targets,
            lost.clone(),
            driving_tx,
            reads_locks.clone(),
        ));
        HookInput {
            reads_locks,
            messages,
            numbered: Arc::default(),
            lost,
            target,
            driving,
        }
    }

    /// Whether the remote the stream is relayed to holds the pointer, as last told.
    pub fn driven(&self) -> bool {
        self.driving.borrow().driven
    }

    /// The machine the pointer is on, with several remotes ([`Self::set_hosts`]).
    pub fn driving_host(&self) -> Option<String> {
        let driving = self.driving.borrow();
        driving.host.clone().filter(|_| driving.driven)
    }

    /// Whether the remote the stream is relayed to is driven now (see the module): told
    /// when it changes, never asked again and again.
    pub fn driving(&self) -> watch::Receiver<Driving> {
        self.driving.clone()
    }

    /// A sink for one link: it hears of the connection's losses from now on only.
    pub fn sink(&self) -> Sink {
        Sink {
            messages: self.messages.clone(),
            numbered: self.numbered.clone(),
            lost: self.lost.subscribe(),
        }
    }

    /// The link the hook's own batches go to (this machine is its master), or none.
    /// Whether this machine's hook reads the lock keys' state, as it said when it
    /// greeted its agent.
    pub fn set_reads_locks(&self, reads: bool) {
        self.reads_locks.store(reads, Ordering::Relaxed);
    }

    pub fn set_target(&self, target: Option<Target>) {
        self.target
            .send_modify(|now| now.relay = Relay::One(target));
    }

    /// The remotes the hook's stream goes to, by machine id, for a hook that names the
    /// machine of every crossing. A change of them is not the end of a hop: the
    /// connection stays, a run for a machine no longer here is dropped (and brings the
    /// pointer home if it was there). From a single target, or to one, it is.
    pub fn set_hosts(&self, hosts: BTreeMap<String, Target>) {
        self.target
            .send_modify(|now| now.relay = Relay::Hosts(hosts));
    }

    /// Ends the hop under way and starts the next at once, to the same link: the remote
    /// ended its session (`InputReset`) and ignores what follows until an `Enter`, which
    /// only a new crossing sends. The connection is closed — the hook comes home, its
    /// keyboards ungrabbed — the link is told `Leave`, and a new connection is made.
    pub fn restart(&self) {
        self.target.send_modify(|now| now.epoch += 1);
    }
}

/// An open connection: the write half here, the read half in a task that relays.
struct Connection {
    writer: Box<dyn AsyncWrite + Unpin + Send>,
    reading: JoinHandle<()>,
    /// Resolves when the relay ended by itself; `true` asks for a new connection at once
    /// (a machine the pointer was on is gone or broken), not a second later.
    lost: oneshot::Receiver<bool>,
    /// The link the hook's batches go to, for the whole life of this connection — with
    /// one remote. With several, the target of the machine the pointer is on now, kept
    /// by the relay: the one told `Leave` when the connection ends.
    relaying_to: Arc<Mutex<Option<Target>>>,
    /// With several remotes: the `Leave`s still on their way to machines whose run had no
    /// room. Each new session there waits for its own, and the next connection for all.
    pending_leaves: PendingLeaves,
    /// The number of the last batch written to it (0: none).
    written: u64,
}

type PendingLeaves = Arc<Mutex<Vec<(String, JoinHandle<()>)>>>;

impl Drop for Connection {
    fn drop(&mut self) {
        // The read half goes with the write half: the hook sees one connection end.
        self.reading.abort();
    }
}

async fn run(
    endpoint: Option<String>,
    mut inbox: mpsc::Receiver<Message>,
    mut targets: watch::Receiver<Targets>,
    lost: watch::Sender<u64>,
    driving: watch::Sender<Driving>,
    reads_locks: Arc<AtomicBool>,
) {
    let driving: DrivingSender = std::sync::Arc::new(driving);
    // Batches up to `number` did not reach the hook, or not for good: the links' tasks
    // are told, and the one whose session they belong to ends it.
    let tell = |number: u64| {
        lost.send_modify(|latest| *latest = (*latest).max(number));
    };
    // The connection ended: what was written to it is the hook's no more.
    let lose =
        |connection: &mut Option<Connection>, leaving: &mut Option<JoinHandle<()>>, number: u64| {
            if let Some(open) = connection {
                let number = number.max(open.written);
                close(connection, leaving, &driving);
                tell(number);
            }
        };
    let mut connection: Option<Connection> = None;
    let mut retry_at: Option<Instant> = None;
    // The `Leave` of the last hop being queued on its link; no new connection relays
    // anything before it is.
    let mut leaving: Option<JoinHandle<()>> = None;
    let mut targets_open = true;
    // What the connection was made for: several remotes keep it across a change of
    // targets, as long as no restart came.
    let mut seen = {
        let now = targets.borrow();
        (matches!(now.relay, Relay::Hosts(_)), now.epoch)
    };
    loop {
        let relaying = targets.borrow().relaying();
        let may_try = retry_at.is_none_or(|at| Instant::now() >= at);
        if relaying && connection.is_none() && may_try && leaving.is_none() {
            connection = open(
                endpoint.as_deref(),
                relay_target(&targets, &leaving),
                &mut retry_at,
                &driving,
            )
            .await;
        }
        let waiting_to_retry = relaying && connection.is_none() && leaving.is_none();
        let retry = retry_at.unwrap_or_else(Instant::now);
        tokio::select! {
            message = inbox.recv() => match message {
                None => {
                    close(&mut connection, &mut leaving, &driving);
                    return;
                }
                // The stream of one link ended. The hook serves one session per
                // connection, shared by every master of this machine: closing it, so
                // that the hook releases what that link left pressed, ends the session
                // of any other master too. Their links are told, as of a loss; the one
                // releasing has ended its own already.
                Some(Message::Release) => lose(&mut connection, &mut leaving, 0),
                Some(Message::Batch { number, bytes, crosses }) => {
                    // A crossing is worth an attempt at any time: one a second at most.
                    let may_try = crosses || retry_at.is_none_or(|at| Instant::now() >= at);
                    if connection.is_none() && may_try {
                        let target = relay_target(&targets, &leaving);
                        connection = open(endpoint.as_deref(), target, &mut retry_at, &driving).await;
                    }
                    if let Some(open) = &mut connection {
                        // An older hook refuses a whole batch on an event it does not
                        // know, and the lock keys' state is one.
                        let bytes = if reads_locks.load(Ordering::Relaxed) {
                            std::borrow::Cow::Borrowed(&bytes[..])
                        } else {
                            without_locks(&bytes)
                        };
                        let written =
                            tokio::time::timeout(WRITE_TIMEOUT, write_batch(&mut open.writer, &bytes)).await;
                        if matches!(written, Ok(Ok(()))) {
                            open.written = number;
                        } else {
                            lose(&mut connection, &mut leaving, number);
                            retry_at = Some(Instant::now() + RETRY_AFTER);
                        }
                    } else {
                        // Dropped: never queued for later, input replayed late is input
                        // nobody typed. A session it belonged to is over.
                        tell(number);
                    }
                }
            },
            again = async {
                match &mut connection {
                    Some(open) => (&mut open.lost).await.unwrap_or(false),
                    None => std::future::pending().await,
                }
            } => {
                lose(&mut connection, &mut leaving, 0);
                retry_at = (!again).then(|| Instant::now() + RETRY_AFTER);
            }
            _ = async {
                match &mut leaving {
                    Some(task) => { let _ = task.await; }
                    None => std::future::pending().await,
                }
            } => leaving = None,
            changed = targets.changed(), if targets_open => {
                if changed.is_err() {
                    targets_open = false;
                } else {
                    let now = {
                        let now = targets.borrow_and_update();
                        (matches!(now.relay, Relay::Hosts(_)), now.epoch)
                    };
                    // Several remotes before and after, and no restart: the relay
                    // reads the new targets itself, and the hop goes on.
                    let goes_on = seen.0 && now == seen;
                    seen = now;
                    if !goes_on {
                        // Another remote, or none: this hop of the stream is over, and
                        // the next connection, if any, relays to the new one from its
                        // start. A machine that is also a remote loses its own session
                        // with it.
                        lose(&mut connection, &mut leaving, 0);
                        retry_at = None;
                    }
                }
            }
            _ = tokio::time::sleep_until(retry), if waiting_to_retry => {}
        }
    }
}

/// What a connection opened now relays to: the current target, or the drivable
/// remotes, unless the last hop's `Leave` is still on its way. A connection opened
/// meanwhile (this machine also being a remote, whose batches share the connection)
/// relays nothing.
fn relay_target(
    targets: &watch::Receiver<Targets>,
    leaving: &Option<JoinHandle<()>>,
) -> Option<Relaying> {
    if leaving.is_some() {
        return None;
    }
    match &targets.borrow().relay {
        Relay::One(target) => target.clone().map(Relaying::One),
        Relay::Hosts(_) => Some(Relaying::Hosts(targets.clone())),
    }
}

/// What one connection relays to, for its whole life.
enum Relaying {
    One(Target),
    /// The drivable remotes, read again at every batch.
    Hosts(watch::Receiver<Targets>),
}

/// Ends a connection: the hook sees it go (and comes home, releasing what it holds),
/// and the link it relayed to is told `Leave`. That send waits for room, so it is
/// neither lost nor overtaken by a later connection's batches, which wait for it; it
/// ends at the latest with the link, whose end the remote takes as a `Leave` too.
fn close(
    connection: &mut Option<Connection>,
    leaving: &mut Option<JoinHandle<()>>,
    driving: &watch::Sender<Driving>,
) {
    // The relay that would have seen a `Leave` is gone with the connection, and
    // whatever it still says is about no connection any more.
    if connection.is_some() {
        driving.send_if_modified(|now| {
            now.connection += 1;
            std::mem::replace(&mut now.driven, false)
        });
    }
    let Some(open) = connection.take() else {
        return;
    };
    let target = open
        .relaying_to
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .take();
    let pending: Vec<JoinHandle<()>> = std::mem::take(
        &mut *open
            .pending_leaves
            .lock()
            .unwrap_or_else(|p| p.into_inner()),
    )
    .into_iter()
    .map(|(_, leave)| leave)
    .collect();
    if target.is_none() && pending.is_empty() {
        return;
    }
    let before = leaving.take();
    *leaving = Some(tokio::spawn(async move {
        if let Some(before) = before {
            let _ = before.await;
        }
        // Leaves of machines other than the one the pointer was on, already on their
        // way: the next connection relays nothing before they are queued either.
        for leave in pending {
            let _ = leave.await;
        }
        if let Some(Target { link, .. }) = target {
            let _ = link.input(&encode(&[InputEvent::Leave])).await;
        }
    }));
}

/// Connects, and starts relaying what the hook writes to `target`.
async fn open(
    endpoint: Option<&str>,
    target: Option<Relaying>,
    retry_at: &mut Option<Instant>,
    driving: &DrivingSender,
) -> Option<Connection> {
    let endpoint = endpoint?;
    match tokio::time::timeout(CONNECT_TIMEOUT, connect(endpoint)).await {
        Ok(Ok(stream)) => {
            *retry_at = None;
            let (reader, writer) = tokio::io::split(stream);
            let (lost, lost_rx) = oneshot::channel();
            // A number of its own, told nobody: it has not driven anything yet.
            let mut number = 0;
            driving.send_if_modified(|now| {
                now.connection += 1;
                number = now.connection;
                false
            });
            let relaying_to = Arc::new(Mutex::new(match &target {
                Some(Relaying::One(target)) => Some(target.clone()),
                _ => None,
            }));
            let pending_leaves = PendingLeaves::default();
            let reading = match target {
                Some(Relaying::Hosts(targets)) => tokio::spawn(relay_hosts(
                    reader,
                    targets,
                    (relaying_to.clone(), pending_leaves.clone()),
                    lost,
                    (driving.clone(), number),
                )),
                one => tokio::spawn(relay(
                    reader,
                    one.map(|one| match one {
                        Relaying::One(target) => target,
                        Relaying::Hosts(_) => unreachable!("matched above"),
                    }),
                    lost,
                    (driving.clone(), number),
                )),
            };
            Some(Connection {
                writer: Box::new(writer),
                reading,
                lost: lost_rx,
                relaying_to,
                pending_leaves,
                written: 0,
            })
        }
        _ => {
            *retry_at = Some(Instant::now() + RETRY_AFTER);
            None
        }
    }
}

/// The hook's batches to `target`, as they come, until the hook goes or the link takes
/// one no more: a batch dropped would be a press or a release the remote never sees,
/// so a full queue ends the connection instead (and with it the session, see
/// [`close`]). With no target they are read and let go: nothing was sent anywhere.
async fn relay(
    mut reader: impl AsyncRead + Unpin,
    target: Option<Target>,
    lost: oneshot::Sender<bool>,
    (driving, connection): (DrivingSender, u64),
) {
    while let Ok(batch) = read_batch(&mut reader).await {
        if let Some(Target {
            link, reads_locks, ..
        }) = &target
        {
            let batch = match reads_locks {
                true => std::borrow::Cow::Borrowed(batch.as_slice()),
                false => without_locks(&batch),
            };
            if link.try_input(&batch).is_err() {
                break;
            }
            if let Some(entered) = crossing(&batch) {
                drive(&driving, connection, entered);
            }
        }
    }
    drive(&driving, connection, false);
    let _ = lost.send(false);
}

/// The hook's batches to the drivable remotes, cut at their `Host` markers. Ends when
/// the hook goes, a batch does not walk, or the machine the pointer is on can take its
/// run no more (no room, or no target: its link went) — then asking for a new
/// connection at once, which brings the pointer home. A run for any other machine that
/// cannot be taken breaks that machine's stream until its next `Enter`: dropped, and
/// the machine is told `Leave` with a send that waits — which that `Enter` waits for,
/// and the next connection too ([`close`]). The machine the pointer is on is told its
/// `Leave` by [`close`], after everything already on its way.
///
/// The machine the pointer is on changing relay (its service taking the stream from its
/// agent, or giving it back) ends the hop the same way: the old relay is told `Leave`,
/// the pointer comes home, and the new relay gets only the next crossing's `Enter`.
async fn relay_hosts(
    mut reader: impl AsyncRead + Unpin,
    targets: watch::Receiver<Targets>,
    (relaying_to, pending_leaves): (Arc<Mutex<Option<Target>>>, PendingLeaves),
    lost: oneshot::Sender<bool>,
    (driving, connection): (DrivingSender, u64),
) {
    // The machine the last marker named, and the one the pointer is on.
    let mut marked: Option<String> = None;
    let mut entered: Option<String> = None;
    let mut broken: HashSet<String> = HashSet::new();
    let mut again = false;
    'batches: while let Ok(batch) = read_batch(&mut reader).await {
        let Some(runs) = by_host(&batch) else {
            eprintln!("[lbm-agent] multi-PC: the hook wrote a batch that does not read");
            break;
        };
        for (host, run) in runs {
            if let Some(host) = host {
                marked = Some(host.to_owned());
            }
            // Before any marker: no destination, so nowhere to go.
            let Some(host) = marked.as_deref() else {
                continue;
            };
            let edge = crossing(run);
            if edge == Some(true) {
                // A fresh session over there: what broke the last one is past.
                broken.remove(host);
            }
            if broken.contains(host) {
                continue;
            }
            let on_it = entered.as_deref() == Some(host);
            let target = match &targets.borrow().relay {
                Relay::Hosts(hosts) => hosts.get(host).cloned(),
                Relay::One(_) => None,
            };
            if on_it {
                let entered_by = relaying_to
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .as_ref()
                    .map(|t| t.route);
                if target.as_ref().is_some_and(|t| Some(t.route) != entered_by) {
                    // The session is the old relay's: it is told Leave by `close`.
                    eprintln!(
                        "[lbm-agent] multi-PC: the input stream to {host} changed its way \
                         there: the pointer comes home for a fresh crossing"
                    );
                    again = true;
                    break 'batches;
                }
            }
            if edge == Some(true) {
                // A new session there starts after the Leave of its last one is queued.
                let waiting: Vec<JoinHandle<()>> = {
                    let mut pending = pending_leaves.lock().unwrap_or_else(|p| p.into_inner());
                    let (mine, others) = std::mem::take(&mut *pending)
                        .into_iter()
                        .partition(|(machine, _)| machine == host);
                    *pending = others;
                    mine.into_iter().map(|(_, leave)| leave).collect()
                };
                for leave in waiting {
                    let _ = leave.await;
                }
            }
            let taken = match &target {
                Some(Target {
                    link, reads_locks, ..
                }) => {
                    let run = match reads_locks {
                        true => std::borrow::Cow::Borrowed(run),
                        false => without_locks(run),
                    };
                    link.try_input(&run).is_ok()
                }
                None => false,
            };
            if !taken {
                broken.insert(host.to_owned());
                if on_it || edge == Some(true) {
                    eprintln!(
                        "[lbm-agent] multi-PC: {host} takes the input stream no more: \
                         the pointer comes home"
                    );
                    if !on_it {
                        // An Enter refused: that machine holds nothing of this session,
                        // but a Leave tells it so if the Enter half went.
                        *relaying_to.lock().unwrap_or_else(|p| p.into_inner()) = target;
                    }
                    // `close` tells it Leave, in order, before any new connection.
                    again = true;
                    break 'batches;
                }
                if let Some(Target { link, .. }) = &target {
                    // What it holds is released by a Leave that is not lost, and that
                    // nothing of a later session there overtakes.
                    let link = link.clone();
                    let leave = tokio::spawn(async move {
                        let _ = link.input(&encode(&[InputEvent::Leave])).await;
                    });
                    pending_leaves
                        .lock()
                        .unwrap_or_else(|p| p.into_inner())
                        .push((host.to_owned(), leave));
                }
                continue;
            }
            match edge {
                Some(true) => {
                    entered = Some(host.to_owned());
                    *relaying_to.lock().unwrap_or_else(|p| p.into_inner()) = target;
                }
                Some(false) if on_it => {
                    entered = None;
                    *relaying_to.lock().unwrap_or_else(|p| p.into_inner()) = None;
                }
                _ => {}
            }
            if edge.is_some() {
                drive_host(&driving, connection, entered.is_some(), entered.as_deref());
            }
        }
    }
    drive(&driving, connection, false);
    let _ = lost.send(again);
}

trait Stream: AsyncRead + AsyncWrite + Unpin + Send {}
impl<S: AsyncRead + AsyncWrite + Unpin + Send> Stream for S {}

#[cfg(unix)]
async fn connect(endpoint: &str) -> io::Result<Box<dyn Stream>> {
    Ok(Box::new(tokio::net::UnixStream::connect(endpoint).await?))
}

/// Answers at once, never waiting for a server: `NotFound` when no hook serves the
/// pipe, `ERROR_PIPE_BUSY` in the moment between the hook accepting a client and
/// offering its next instance. Either way the batch that asked is dropped, as with no
/// hook; a crossing then ends in `InputReset`.
#[cfg(windows)]
async fn connect(endpoint: &str) -> io::Result<Box<dyn Stream>> {
    Ok(Box::new(
        tokio::net::windows::named_pipe::ClientOptions::new().open(endpoint)?,
    ))
}
