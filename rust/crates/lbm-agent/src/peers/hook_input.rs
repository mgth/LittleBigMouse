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
//! - **Driving** ([`HookInput::driving`]): on a master, whether the remote the stream
//!   is relayed to holds the pointer now — an `Enter` relayed and no `Leave` since, nor
//!   the end of the connection. Read off the batches as they pass ([`crossing`]), for the
//!   state's indicator of the machine that receives the input.
//!
//! The hook serves one client, the newest. A machine that is both a master and a remote
//! at once shares this one connection between the two; nothing more is attempted for
//! that case yet.

use std::io;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use lbm_ipc::input::{crossing, encode, read_batch, without_locks, write_batch, InputEvent};
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
}

/// Where the network side hands batches to the hook, and the end of a stream.
pub struct Sink {
    messages: mpsc::Sender<Message>,
    /// The number the last batch queued was given, by any sink.
    numbered: Arc<AtomicU64>,
    /// The number of the latest batch the hook did not get, or got on a connection
    /// that ended since by no doing of the network side (not a [`Sink::release`]).
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
    /// it can neither be lost nor overtake those batches.
    pub async fn release(&self) {
        let _ = self.messages.send(Message::Release).await;
    }

    /// Resolves when a batch was lost since the last time it did (or since this sink
    /// was made), with the number of the latest one: dropped for want of a connection,
    /// or written on a connection that ended since by no doing of this side (the hook
    /// went, a write failed or took too long). Every batch up to that number is the
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
    messages: mpsc::Sender<Message>,
    numbered: Arc<AtomicU64>,
    lost: watch::Sender<u64>,
    target: watch::Sender<Option<Target>>,
    driving: watch::Receiver<Driving>,
}

/// The value behind [`HookInput::driving`]: whether the remote is driven, and the number
/// of the connection that said so — a relay that outlived its connection (aborted, but
/// running on another thread that very moment) cannot say it any more.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Driving {
    connection: u64,
    pub driven: bool,
}

type DrivingSender = std::sync::Arc<watch::Sender<Driving>>;

/// Says what connection `connection` saw, unless another one came since.
fn drive(driving: &watch::Sender<Driving>, connection: u64, driven: bool) {
    driving.send_if_modified(|now| {
        if now.connection != connection || now.driven == driven {
            return false;
        }
        now.driven = driven;
        true
    });
}

impl HookInput {
    /// The connection to the input endpoint `endpoint`; `None` when there is no hook to
    /// talk to, and then every batch is dropped.
    pub fn spawn(endpoint: Option<String>) -> HookInput {
        let (messages, inbox) = mpsc::channel(QUEUE);
        let (target, targets) = watch::channel(None);
        let (lost, _) = watch::channel(0);
        let (driving_tx, driving) = watch::channel(Driving::default());
        tokio::spawn(run(endpoint, inbox, targets, lost.clone(), driving_tx));
        HookInput {
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
    pub fn set_target(&self, target: Option<Target>) {
        self.target.send_replace(target);
    }

    /// Ends the hop under way and starts the next at once, to the same link: the remote
    /// ended its session (`InputReset`) and ignores what follows until an `Enter`, which
    /// only a new crossing sends. The connection is closed — the hook comes home, its
    /// keyboards ungrabbed — the link is told `Leave`, and a new connection is made.
    pub fn restart(&self) {
        self.target.send_modify(|_| {});
    }
}

/// An open connection: the write half here, the read half in a task that relays.
struct Connection {
    writer: Box<dyn AsyncWrite + Unpin + Send>,
    reading: JoinHandle<()>,
    lost: oneshot::Receiver<()>,
    /// The link the hook's batches go to, for the whole life of this connection.
    relaying_to: Option<Target>,
    /// The number of the last batch written to it (0: none).
    written: u64,
}

impl Drop for Connection {
    fn drop(&mut self) {
        // The read half goes with the write half: the hook sees one connection end.
        self.reading.abort();
    }
}

async fn run(
    endpoint: Option<String>,
    mut inbox: mpsc::Receiver<Message>,
    mut targets: watch::Receiver<Option<Target>>,
    lost: watch::Sender<u64>,
    driving: watch::Sender<Driving>,
) {
    let driving: DrivingSender = std::sync::Arc::new(driving);
    // Batches up to `number` did not reach the hook, or not for good: the links' tasks
    // are told, and the one whose session they belong to ends it.
    let tell = |number: u64| {
        lost.send_modify(|latest| *latest = (*latest).max(number));
    };
    // The connection ended by no doing of the network side: what was written to it is
    // the hook's no more.
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
    loop {
        let relaying = targets.borrow().is_some();
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
                Some(Message::Release) => close(&mut connection, &mut leaving, &driving),
                Some(Message::Batch { number, bytes, crosses }) => {
                    // A crossing is worth an attempt at any time: one a second at most.
                    let may_try = crosses || retry_at.is_none_or(|at| Instant::now() >= at);
                    if connection.is_none() && may_try {
                        let target = relay_target(&targets, &leaving);
                        connection = open(endpoint.as_deref(), target, &mut retry_at, &driving).await;
                    }
                    if let Some(open) = &mut connection {
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
            _ = async {
                match &mut connection {
                    Some(open) => { let _ = (&mut open.lost).await; }
                    None => std::future::pending().await,
                }
            } => {
                lose(&mut connection, &mut leaving, 0);
                retry_at = Some(Instant::now() + RETRY_AFTER);
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
                    // Another remote, or none: this hop of the stream is over, and the
                    // next connection, if any, relays to the new one from its start.
                    // A machine that is also a remote loses its own session with it.
                    lose(&mut connection, &mut leaving, 0);
                    retry_at = None;
                }
            }
            _ = tokio::time::sleep_until(retry), if waiting_to_retry => {}
        }
    }
}

/// Where a connection opened now relays: the current target, unless the last hop's
/// `Leave` is still on its way. A connection opened meanwhile (this machine also being
/// a remote, whose batches share the connection) relays nothing.
fn relay_target(
    targets: &watch::Receiver<Option<Target>>,
    leaving: &Option<JoinHandle<()>>,
) -> Option<Target> {
    match leaving {
        Some(_) => None,
        None => targets.borrow().clone(),
    }
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
    let Some(Target { link, .. }) = connection.take().and_then(|c| c.relaying_to.clone()) else {
        return;
    };
    let before = leaving.take();
    *leaving = Some(tokio::spawn(async move {
        if let Some(before) = before {
            let _ = before.await;
        }
        let _ = link.input(&encode(&[InputEvent::Leave])).await;
    }));
}

/// Connects, and starts relaying what the hook writes to `target`.
async fn open(
    endpoint: Option<&str>,
    target: Option<Target>,
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
            let reading = tokio::spawn(relay(
                reader,
                target.clone(),
                lost,
                (driving.clone(), number),
            ));
            Some(Connection {
                writer: Box::new(writer),
                reading,
                lost: lost_rx,
                relaying_to: target,
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
    lost: oneshot::Sender<()>,
    (driving, connection): (DrivingSender, u64),
) {
    while let Ok(batch) = read_batch(&mut reader).await {
        if let Some(Target { link, reads_locks }) = &target {
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
    let _ = lost.send(());
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
