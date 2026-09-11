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
//! (`Connected`, then `Stopped`, which the reconciler answers).

use std::io;
use std::time::Duration;

use lbm_ipc::client::{self, DaemonMessage};
use lbm_ipc::framing::{read_frame, write_frame};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::mpsc;

/// Spacing between connection attempts (C#: `RetryDelay`).
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
    /// An event from the hook (unknown ones are dropped, as C# does).
    Message(DaemonMessage),
    /// The connection dropped; it is being re-established.
    Lost,
    /// No hook answers at the endpoint.
    Unreachable,
}

/// Sends commands to the hook over the current connection.
#[derive(Clone, Debug)]
pub struct HookClient {
    commands: mpsc::UnboundedSender<String>,
}

impl HookClient {
    /// Starts keeping a connection to the hook at `endpoint` (a socket path, or a
    /// `\\.\pipe\...` name on Windows), on the current tokio runtime. The connection
    /// lives as long as a [`HookClient`] clone does.
    pub fn spawn(endpoint: String) -> (HookClient, mpsc::UnboundedReceiver<HookSignal>) {
        let (commands, commands_rx) = mpsc::unbounded_channel();
        let (signals, signals_rx) = mpsc::unbounded_channel();
        tokio::spawn(keep_connected(endpoint, commands_rx, signals));
        (HookClient { commands }, signals_rx)
    }

    /// Sends a command frame (see [`lbm_ipc::client`]); dropped if not connected.
    pub fn send(&self, frame: String) {
        let _ = self.commands.send(frame);
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
    mut commands: mpsc::UnboundedReceiver<String>,
    signals: mpsc::UnboundedSender<HookSignal>,
) {
    let mut failures = 0u32;
    loop {
        let attempt = tokio::time::timeout(ATTEMPT_TIMEOUT, connect(&endpoint)).await;
        let stream = match attempt {
            Ok(Ok(stream)) => stream,
            _ => {
                if failures.is_multiple_of(UNREACHABLE_EVERY)
                    && signals.send(HookSignal::Unreachable).is_err()
                {
                    return;
                }
                failures = failures.wrapping_add(1);
                // Nobody to send to: what was asked meanwhile is dropped.
                tokio::time::sleep(RETRY_DELAY).await;
                loop {
                    match commands.try_recv() {
                        Ok(_) => continue,
                        Err(mpsc::error::TryRecvError::Empty) => break,
                        Err(mpsc::error::TryRecvError::Disconnected) => return,
                    }
                }
                continue;
            }
        };
        failures = 0;

        match serve(stream, &mut commands, &signals).await {
            Served::Dropped => {
                if signals.send(HookSignal::Lost).is_err() {
                    return;
                }
                tokio::time::sleep(RETRY_DELAY).await;
            }
            Served::Closed => return,
        }
    }
}

enum Served {
    /// The connection dropped.
    Dropped,
    /// Nobody holds a client or listens any more.
    Closed,
}

async fn serve<S>(
    stream: S,
    commands: &mut mpsc::UnboundedReceiver<String>,
    signals: &mpsc::UnboundedSender<HookSignal>,
) -> Served
where
    S: AsyncRead + AsyncWrite + Send + 'static,
{
    let (mut reader, mut writer) = tokio::io::split(stream);
    if signals.send(HookSignal::Connected).is_err() {
        return Served::Closed;
    }
    if write_frame(&mut writer, &client::listen()).await.is_err() {
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

    let served = loop {
        tokio::select! {
            frame = frames_rx.recv() => match frame {
                Some(frame) => {
                    if let Some(message) = client::parse_event(&frame) {
                        if signals.send(HookSignal::Message(message)).is_err() {
                            break Served::Closed;
                        }
                    }
                }
                None => break Served::Dropped,
            },
            command = commands.recv() => match command {
                Some(command) => {
                    if write_frame(&mut writer, &command).await.is_err() {
                        break Served::Dropped;
                    }
                }
                None => break Served::Closed,
            },
        }
    };
    reading.abort();
    served
}
