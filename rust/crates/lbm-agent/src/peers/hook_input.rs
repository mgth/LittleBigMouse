//! The agent's end of the hook's input endpoint (`lbm_ipc::input::input_endpoint_beside`):
//! one connection, both directions, in a task of its own.
//!
//! - **Remote**: the batches a master sends arrive from the network and are written to
//!   the hook, which injects them. The connection is made at the first batch and made
//!   again after a failure; batches that find no connection are dropped, never queued
//!   for later — input replayed late is input nobody typed. When the link they came on
//!   ends, the connection is closed ([`Sink::release`]): that is how the hook learns to
//!   release whatever the stream left pressed.
//! - **Master**: while a remote is linked ([`HookInput::set_target`]), the connection is
//!   kept, and every batch the hook writes to it is relayed to that remote's link with
//!   `try_input` — a congested link costs input on that link, never a queue or a wait.
//!   When the remote goes away, the connection is closed: the hop of the stream has
//!   ended, which is what brings the master's pointer home (docs/multi-pc.md).
//!
//! The hook serves one client, the newest. A machine that is both a master and a remote
//! at once shares this one connection between the two; nothing more is attempted for
//! that case yet.

use std::io;
use std::time::Duration;

use lbm_ipc::input::{read_batch, write_batch};
use lbm_net::link::LinkSender;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::{mpsc, oneshot, watch};
use tokio::task::JoinHandle;
use tokio::time::Instant;

/// Batches waiting to be written to the hook: a few dozen milliseconds of a fast mouse.
/// Beyond it the hook is not keeping up, and batches are dropped rather than queued.
const QUEUE: usize = 64;

/// How long one connection attempt may take (a Windows pipe client waits for a server
/// instead of failing).
const CONNECT_TIMEOUT: Duration = Duration::from_millis(250);

/// After a failed attempt, batches are dropped for this long before the next one: a
/// hook that is not there must not be asked a thousand times a second.
const RETRY_AFTER: Duration = Duration::from_secs(1);

/// A hook that takes longer than this to read a batch is not injecting anything useful:
/// the connection is closed, which also makes it release what it held.
const WRITE_TIMEOUT: Duration = Duration::from_secs(1);

enum Message {
    Batch(Vec<u8>),
    Release,
}

/// Where the network side hands batches to the hook, and the end of a stream.
#[derive(Clone)]
pub struct Sink(mpsc::Sender<Message>);

impl Sink {
    /// Queues a batch for the hook if there is room; dropped otherwise.
    pub fn try_batch(&self, batch: Vec<u8>) -> bool {
        self.0.try_send(Message::Batch(batch)).is_ok()
    }

    /// The stream ended: closes the connection after the batches already queued, so
    /// the hook releases what they left pressed. Waits for room in the queue, so that
    /// it can neither be lost nor overtake those batches.
    pub async fn release(&self) {
        let _ = self.0.send(Message::Release).await;
    }
}

/// The connection's task, and its two inputs.
pub struct HookInput {
    sink: Sink,
    target: watch::Sender<Option<LinkSender>>,
}

impl HookInput {
    /// The connection to the input endpoint `endpoint`; `None` when there is no hook to
    /// talk to, and then every batch is dropped.
    pub fn spawn(endpoint: Option<String>) -> HookInput {
        let (messages, inbox) = mpsc::channel(QUEUE);
        let (target, targets) = watch::channel(None);
        tokio::spawn(run(endpoint, inbox, targets));
        HookInput {
            sink: Sink(messages),
            target,
        }
    }

    pub fn sink(&self) -> Sink {
        self.sink.clone()
    }

    /// The link the hook's own batches go to (this machine is its master), or none.
    pub fn set_target(&self, link: Option<LinkSender>) {
        self.target.send_replace(link);
    }
}

/// An open connection: the write half here, the read half in a task that relays.
struct Connection {
    writer: Box<dyn AsyncWrite + Unpin + Send>,
    reading: JoinHandle<()>,
    lost: oneshot::Receiver<()>,
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
    mut targets: watch::Receiver<Option<LinkSender>>,
) {
    let mut connection: Option<Connection> = None;
    let mut retry_at: Option<Instant> = None;
    let mut targets_open = true;
    loop {
        let relaying = targets.borrow().is_some();
        let may_try = retry_at.is_none_or(|at| Instant::now() >= at);
        if relaying && connection.is_none() && may_try {
            connection = open(endpoint.as_deref(), &targets, &mut retry_at).await;
        }
        let waiting_to_retry = relaying && connection.is_none();
        let retry = retry_at.unwrap_or_else(Instant::now);
        tokio::select! {
            message = inbox.recv() => match message {
                None => return,
                Some(Message::Release) => connection = None,
                Some(Message::Batch(batch)) => {
                    let may_try = retry_at.is_none_or(|at| Instant::now() >= at);
                    if connection.is_none() && may_try {
                        connection = open(endpoint.as_deref(), &targets, &mut retry_at).await;
                    }
                    if let Some(open) = &mut connection {
                        let written =
                            tokio::time::timeout(WRITE_TIMEOUT, write_batch(&mut open.writer, &batch)).await;
                        if !matches!(written, Ok(Ok(()))) {
                            connection = None;
                            retry_at = Some(Instant::now() + RETRY_AFTER);
                        }
                    }
                }
            },
            _ = async {
                match &mut connection {
                    Some(open) => { let _ = (&mut open.lost).await; }
                    None => std::future::pending().await,
                }
            } => {
                connection = None;
                retry_at = Some(Instant::now() + RETRY_AFTER);
            }
            changed = targets.changed(), if targets_open => {
                if changed.is_err() {
                    targets_open = false;
                } else if targets.borrow().is_none() {
                    // The remote is gone: so is this hop of the stream.
                    connection = None;
                    retry_at = None;
                }
            }
            _ = tokio::time::sleep_until(retry), if waiting_to_retry => {}
        }
    }
}

/// Connects, and starts relaying what the hook writes to the current target.
async fn open(
    endpoint: Option<&str>,
    targets: &watch::Receiver<Option<LinkSender>>,
    retry_at: &mut Option<Instant>,
) -> Option<Connection> {
    let endpoint = endpoint?;
    match tokio::time::timeout(CONNECT_TIMEOUT, connect(endpoint)).await {
        Ok(Ok(stream)) => {
            *retry_at = None;
            let (reader, writer) = tokio::io::split(stream);
            let (lost, lost_rx) = oneshot::channel();
            let reading = tokio::spawn(relay(reader, targets.clone(), lost));
            Some(Connection {
                writer: Box::new(writer),
                reading,
                lost: lost_rx,
            })
        }
        _ => {
            *retry_at = Some(Instant::now() + RETRY_AFTER);
            None
        }
    }
}

/// The hook's batches to the linked remote, as they come. A batch the link has no room
/// for is dropped there (`try_input`): the next report says where the pointer is now.
async fn relay(
    mut reader: impl AsyncRead + Unpin,
    targets: watch::Receiver<Option<LinkSender>>,
    lost: oneshot::Sender<()>,
) {
    while let Ok(batch) = read_batch(&mut reader).await {
        let target = targets.borrow().clone();
        if let Some(link) = target {
            let _ = link.try_input(&batch);
        }
    }
    let _ = lost.send(());
}

trait Stream: AsyncRead + AsyncWrite + Unpin + Send {}
impl<S: AsyncRead + AsyncWrite + Unpin + Send> Stream for S {}

#[cfg(unix)]
async fn connect(endpoint: &str) -> io::Result<Box<dyn Stream>> {
    Ok(Box::new(tokio::net::UnixStream::connect(endpoint).await?))
}

#[cfg(windows)]
async fn connect(endpoint: &str) -> io::Result<Box<dyn Stream>> {
    Ok(Box::new(
        tokio::net::windows::named_pipe::ClientOptions::new().open(endpoint)?,
    ))
}
