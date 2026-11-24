//! The connection to the hook — the agent's side of what the C# `LocalIpcClient` did.
//!
//! One connection carries both directions: on connecting it subscribes (`Listen`), then
//! events come in and commands go out on it, in the order they are given. (The C#
//! client opened a fresh connection per command, which is how commands asked for in one
//! order could reach the daemon in another.) The connection is kept up: when it drops
//! or cannot be made, it is retried every 100 ms, and the runtime is told —
//! [`HookSignal::Lost`] once when it drops (C# synthesizes a `Dead` event there),
//! [`HookSignal::Unreachable`] on the first failed attempt and then about every five
//! seconds, which is when a supervisor may launch a hook.
//!
//! Commands given while there is no connection are dropped, not queued: replayed after a
//! reconnection they would be stale, and a (re)connected hook asks for its layout anyway
//! (`Connected`, then `Stopped`, which the reconciler answers). That holds for every
//! command given before the caller *knows* of the connection, not only those given
//! while there was none: a frame sent while the old connection was dropping, or before
//! the caller read [`HookSignal::Connected`], was decided for a hook it no longer
//! describes. Written to the new one, the hook would apply zones the caller has counted
//! lost — a layout leading to a remote no longer selected, applied while the multi-PC
//! mode believes the border closed. So a new connection writes nothing until
//! [`HookSignals::recv`] has handed its `Connected` out.

use std::io;
use std::time::Duration;

use lbm_ipc::framing::{read_frame, write_frame};
use lbm_ipc::protocol::{self, Command, Event};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::mpsc;

/// Spacing between connection attempts (C#: `RetryDelay`).
/// How long a hook has to say who it is before it is taken for an older one.
const HANDSHAKE_PATIENCE: Duration = Duration::from_secs(5);

const RETRY_DELAY: Duration = Duration::from_millis(100);

/// Bound on one connection attempt (C#: `ConnectAttemptTimeout`): a Windows pipe
/// client waits for a server instead of failing.
const ATTEMPT_TIMEOUT: Duration = Duration::from_millis(250);

/// Failed attempts between two [`HookSignal::Unreachable`] (C#: `NotifyEveryAttempts`,
/// about every 5 s).
const UNREACHABLE_EVERY: u32 = 14;

/// What the connection tells the runtime.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HookSignal {
    /// Connected and subscribed.
    Connected,
    /// The hook said who it is, and which layout it holds (empty: none). Always
    /// before the state that follows it: the opening frame asks `Hello` then
    /// `Listen`, and the hook answers a frame's commands in the order they are in it.
    /// `reads_locks`: it reads the lock keys' state in the input stream
    /// (`lbm_ipc::input::LOCKS_CAPABILITY`).
    Greeted { layout: String, reads_locks: bool },
    /// An event from the hook (unknown ones are dropped, as C# does).
    Message(Event),
    /// The connection dropped; it is being re-established.
    Lost,
    /// No hook answers at the endpoint.
    Unreachable,
}

/// What goes to the connection task.
#[derive(Debug)]
enum Outgoing {
    Frame(String),
    /// Answered once everything queued before it is written (or dropped).
    Flush(tokio::sync::oneshot::Sender<()>),
    /// The caller has read the `Connected` of this connection: what it sends from now on
    /// was decided knowing of it.
    Seen(u64),
}

/// What the connection task says, with the connection it is about.
type Signal = (HookSignal, u64);

/// What the connection tells its caller, in order.
///
/// Reading a [`HookSignal::Connected`] here is what lets that connection carry
/// commands: until then, the hook it reaches is one the caller has not heard of.
#[derive(Debug)]
pub struct HookSignals {
    signals: mpsc::UnboundedReceiver<Signal>,
    /// Weak: the connection lives as long as a [`HookClient`] does, not as long as
    /// someone listens.
    commands: mpsc::WeakUnboundedSender<Outgoing>,
}

impl HookSignals {
    /// The next signal; `None` once the connection task is gone. Cancel-safe.
    pub async fn recv(&mut self) -> Option<HookSignal> {
        let (signal, connection) = self.signals.recv().await?;
        if signal == HookSignal::Connected {
            if let Some(commands) = self.commands.upgrade() {
                let _ = commands.send(Outgoing::Seen(connection));
            }
        }
        Some(signal)
    }
}

/// Sends commands to the hook over the current connection.
#[derive(Clone, Debug)]
pub struct HookClient {
    commands: mpsc::UnboundedSender<Outgoing>,
}

impl HookClient {
    /// Starts keeping a connection to the hook at `endpoint` (a socket path, or a
    /// `\\.\pipe\...` name on Windows), on the current tokio runtime. The connection
    /// lives as long as a [`HookClient`] clone does.
    pub fn spawn(endpoint: String) -> (HookClient, HookSignals) {
        let (commands, commands_rx) = mpsc::unbounded_channel();
        let (signals, signals_rx) = mpsc::unbounded_channel();
        tokio::spawn(keep_connected(endpoint, commands_rx, signals));
        let signals = HookSignals {
            signals: signals_rx,
            commands: commands.downgrade(),
        };
        (HookClient { commands }, signals)
    }

    /// Sends a command frame (see [`lbm_ipc::client`]); dropped if not connected, or
    /// connected to a hook whose [`HookSignal::Connected`] has not been read yet.
    pub fn send(&self, frame: String) {
        let _ = self.commands.send(Outgoing::Frame(frame));
    }

    /// Completes once every frame sent before is written to the hook, or dropped for
    /// want of one — what a leaving agent waits for, so its last command (`Quit`)
    /// is not lost with the process.
    pub async fn flush(&self) {
        let (done, done_rx) = tokio::sync::oneshot::channel();
        if self.commands.send(Outgoing::Flush(done)).is_ok() {
            let _ = done_rx.await;
        }
    }
}

#[cfg(unix)]
async fn connect(endpoint: &str) -> io::Result<tokio::net::UnixStream> {
    tokio::net::UnixStream::connect(endpoint).await
}

#[cfg(windows)]
async fn connect(endpoint: &str) -> io::Result<tokio::net::windows::named_pipe::NamedPipeClient> {
    tokio::net::windows::named_pipe::ClientOptions::new().open(endpoint)
}

async fn keep_connected(
    endpoint: String,
    mut commands: mpsc::UnboundedReceiver<Outgoing>,
    signals: mpsc::UnboundedSender<Signal>,
) {
    let mut failures = 0u32;
    let mut connection = 0u64;
    loop {
        let attempt = tokio::time::timeout(ATTEMPT_TIMEOUT, connect(&endpoint)).await;
        let stream = match attempt {
            Ok(Ok(stream)) => stream,
            _ => {
                if failures.is_multiple_of(UNREACHABLE_EVERY)
                    && signals.send((HookSignal::Unreachable, connection)).is_err()
                {
                    return;
                }
                failures = failures.wrapping_add(1);
                // Nobody to send to: what was asked meanwhile is dropped.
                tokio::time::sleep(RETRY_DELAY).await;
                loop {
                    match commands.try_recv() {
                        Ok(Outgoing::Flush(done)) => {
                            let _ = done.send(());
                        }
                        Ok(Outgoing::Frame(_) | Outgoing::Seen(_)) => continue,
                        Err(mpsc::error::TryRecvError::Empty) => break,
                        Err(mpsc::error::TryRecvError::Disconnected) => return,
                    }
                }
                continue;
            }
        };
        failures = 0;
        connection = connection.wrapping_add(1);

        match serve(stream, connection, &mut commands, &signals).await {
            Served::Dropped => {
                if signals.send((HookSignal::Lost, connection)).is_err() {
                    return;
                }
                tokio::time::sleep(RETRY_DELAY).await;
            }
            Served::Closed => return,
            // Say goodbye the only way it still understands, then let the supervisor
            // find no hook and launch one from beside this agent. Leaving it alone
            // would leave the mice held by a process nothing here can talk to.
            Served::Foreign => {
                retire_foreign_hook(&endpoint).await;
                if signals.send((HookSignal::Lost, connection)).is_err() {
                    return;
                }
                tokio::time::sleep(RETRY_DELAY).await;
            }
        }
    }
}

/// Open a fresh connection and send the farewell a pre-JSON hook understands. Best
/// effort by nature: a hook that ignores it is one the user will have to stop, and the
/// panic shortcut is how.
async fn retire_foreign_hook(endpoint: &str) {
    if let Ok(mut stream) = connect(endpoint).await {
        let _ = write_frame(&mut stream, protocol::LEGACY_QUIT).await;
    }
}

enum Served {
    /// The connection dropped.
    Dropped,
    /// Nobody holds a client or listens any more.
    Closed,
    /// There is a hook there, and it is not one this agent can drive — an upgrade
    /// left the old one running (D5: a hook outlives its agent). It is retired the
    /// only way it still understands, and the supervisor launches one of ours.
    ///
    /// Either it named a protocol that is not ours, or — the case a real upgrade
    /// produces — it said nothing at all: a hook that predates this protocol reads
    /// the opening frame as XML, finds no commands in it, and has nothing to answer.
    Foreign,
}

async fn serve<S>(
    stream: S,
    connection: u64,
    commands: &mut mpsc::UnboundedReceiver<Outgoing>,
    signals: &mpsc::UnboundedSender<Signal>,
) -> Served
where
    S: AsyncRead + AsyncWrite + Send + 'static,
{
    let signal = |signal: HookSignal| signals.send((signal, connection));
    let (mut reader, mut writer) = tokio::io::split(stream);
    if signal(HookSignal::Connected).is_err() {
        return Served::Closed;
    }
    // Whether the caller has read this connection's `Connected`: until then, every frame
    // it sends was meant for a hook it no longer reaches.
    let mut seen = false;
    // Who is there, then subscribe. Both in one frame: a hook that answers the first
    // understands the second, and one that answers neither is the case below.
    let opening = protocol::frame(&[
        Command::Hello {
            protocol: protocol::PROTOCOL,
        },
        Command::Listen,
    ]);
    if write_frame(&mut writer, &opening).await.is_err() {
        return Served::Dropped;
    }

    // Reading has its own task: a frame read half-way must not be abandoned because a
    // command was ready first.
    let (frames, mut frames_rx) = mpsc::unbounded_channel();
    let reading = tokio::spawn(async move {
        while let Ok(frame) = read_frame(&mut reader).await {
            if frames.send(frame).is_err() {
                break;
            }
        }
    });

    // A hook answers `Hello` off its IPC thread, so it answers at once or it is not
    // one of ours. Generous all the same: the cost of waiting is nothing, and the cost
    // of retiring a working hook is a cursor that stops routing until it comes back.
    let handshake_by = tokio::time::Instant::now() + HANDSHAKE_PATIENCE;
    let mut greeted = false;

    let served = loop {
        tokio::select! {
            _ = tokio::time::sleep_until(handshake_by), if !greeted => {
                eprintln!(
                    "[lbm-agent] the hook did not answer the handshake: it is older than \
                     this protocol; retiring it"
                );
                break Served::Foreign;
            }
            frame = frames_rx.recv() => match frame {
                Some(frame) => {
                    match protocol::parse_event(&frame) {
                        // The protocol number is the connection's business — above,
                        // only whether it is ours matters, and that is decided here.
                        // The layout it names is not: what to do about a hook already
                        // applying one is the reconciler's to decide.
                        Some(Event::Hello { protocol: theirs, version, layout, capabilities }) => {
                            greeted = true;
                            if theirs != protocol::PROTOCOL {
                                eprintln!(
                                    "[lbm-agent] the hook speaks protocol {theirs} (version \
                                     {version}), this agent speaks {}: retiring it",
                                    protocol::PROTOCOL
                                );
                                break Served::Foreign;
                            }
                            let reads_locks = capabilities
                                .iter()
                                .any(|c| c == lbm_ipc::input::LOCKS_CAPABILITY);
                            if signal(HookSignal::Greeted { layout, reads_locks }).is_err() {
                                break Served::Closed;
                            }
                        }
                        Some(message) => {
                            if signal(HookSignal::Message(message)).is_err() {
                                break Served::Closed;
                            }
                        }
                        None => {}
                    }
                }
                None => break Served::Dropped,
            },
            command = commands.recv() => match command {
                Some(Outgoing::Frame(command)) if seen => {
                    if write_frame(&mut writer, &command).await.is_err() {
                        break Served::Dropped;
                    }
                }
                Some(Outgoing::Frame(_)) => {
                    eprintln!(
                        "[lbm-agent] a command meant for a previous hook connection: dropped"
                    );
                }
                Some(Outgoing::Seen(seen_connection)) => {
                    seen |= seen_connection == connection;
                }
                Some(Outgoing::Flush(done)) => {
                    let _ = done.send(());
                }
                None => break Served::Closed,
            },
        }
    };
    reading.abort();
    served
}
