//! Single-client, per-user local IPC server.
//!
//! Windows uses a current-session named pipe whose DACL grants only the current
//! user and SYSTEM. Linux uses a 0600 Unix-domain socket (both in [`transport`]).
//! Both transports share the same length-prefixed UTF-8 protocol, ordered command
//! queue, and non-blocking outbound queue.
//!
//! There is exactly one client, and it is the agent (v6): the frontends talk to the
//! agent, the agent talks to the hook, and nothing else has any business driving the
//! mice. The newest connection wins — see [`ServerHandle`].

use std::io;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::{mpsc, oneshot, Notify};

use crate::daemon;
use crate::ipc::framing::{read_frame, write_frame};
use crate::ipc::transport;
use crate::shared::Shared;

pub type ClientId = u64;

const COMMAND_QUEUE_CAPACITY: usize = 64;
const CLIENT_QUEUE_CAPACITY: usize = 16;
/// How long the daemon has to act on a command. Not how long a client may stay
/// silent: the one client holds its connection for as long as it lives, and asks
/// nothing between a display change and the next.
const COMMAND_TIMEOUT: Duration = Duration::from_secs(5);
const WRITE_TIMEOUT: Duration = Duration::from_secs(2);

struct ClientHandle {
    id: ClientId,
    outbound: mpsc::Sender<String>,
    listening: AtomicBool,
    /// Raised when a newer connection took this one's place. Notified with
    /// [`Notify::notify_one`], not `notify_waiters`: eviction lands whenever it
    /// lands, including while this connection is busy dispatching a command, and a
    /// wake-up nobody was waiting for yet must not be lost. It would leave an
    /// evicted agent holding a connection the hook still executes `Run` and `Quit`
    /// from.
    evicted: Arc<Notify>,
}

impl ClientHandle {
    fn new(id: ClientId, outbound: mpsc::Sender<String>) -> Self {
        Self {
            id,
            outbound,
            listening: AtomicBool::new(false),
            evicted: Arc::new(Notify::new()),
        }
    }
}

struct InboundCommand {
    id: ClientId,
    message: String,
    completed: oneshot::Sender<()>,
}

/// Cloneable synchronous facade used by daemon and hook callbacks.
///
/// One client at a time — the agent. The newest connection wins: an agent that was
/// restarted, or one that took over from a crashed predecessor, must be able to drive
/// the hook without waiting for the operating system to notice that the connection it
/// replaces is dead.
#[derive(Clone)]
pub struct ServerHandle {
    client: Arc<Mutex<Option<Arc<ClientHandle>>>>,
    commands: mpsc::Sender<InboundCommand>,
}

impl ServerHandle {
    fn new(commands: mpsc::Sender<InboundCommand>) -> Self {
        Self {
            client: Arc::new(Mutex::new(None)),
            commands,
        }
    }

    /// Take the connection, and hand back whoever had it so they can be told.
    fn adopt(&self, client: Arc<ClientHandle>) -> Option<Arc<ClientHandle>> {
        self.client
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .replace(client)
    }

    /// Give the connection up — but only if it is still ours, and say whether it was.
    ///
    /// A connection that ends after being evicted must not take its successor's place
    /// with it. The answer is also the only honest way to tell the two endings apart:
    /// after an eviction the newcomer has already replaced the handle, so nothing is
    /// removed here. A client that *was* still the current one is a client that really
    /// went away — which is what "bound to the agent" turns on.
    pub fn remove(&self, id: ClientId) -> bool {
        let mut held = self.client.lock().unwrap_or_else(|p| p.into_inner());
        if held.as_ref().is_some_and(|client| client.id == id) {
            *held = None;
            return true;
        }
        false
    }

    fn get(&self, id: ClientId) -> Option<Arc<ClientHandle>> {
        self.client
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .as_ref()
            .filter(|client| client.id == id)
            .cloned()
    }

    pub fn set_listening(&self, id: ClientId) {
        if let Some(client) = self.get(id) {
            client.listening.store(true, Ordering::SeqCst);
        }
    }

    pub fn send_to(&self, id: ClientId, event: &crate::ipc::protocol::Event) {
        let message = &crate::ipc::protocol::event(event);
        if let Some(client) = self.get(id) {
            if client.outbound.try_send(message.to_string()).is_err() {
                self.remove(id);
            }
        }
    }

    /// Never blocks the hook/message-pump thread. A client whose bounded queue is
    /// full is disconnected rather than allowed to delay input routing.
    pub fn broadcast(&self, event: &crate::ipc::protocol::Event) {
        let listener = self
            .client
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .as_ref()
            .filter(|client| client.listening.load(Ordering::SeqCst))
            .cloned();

        if let Some(client) = listener {
            let message = crate::ipc::protocol::event(event);
            if client.outbound.try_send(message).is_err() {
                self.remove(client.id);
            }
        }
    }
}

trait LocalStream: AsyncRead + AsyncWrite + Unpin + Send + 'static {}
impl<T> LocalStream for T where T: AsyncRead + AsyncWrite + Unpin + Send + 'static {}

/// Start the production endpoint and return its diagnostic name/path.
pub fn start(shared: &'static Shared) -> io::Result<(ServerHandle, String)> {
    start_with_endpoint(shared, transport::default_endpoint()?)
}

/// Explicit endpoint variant used by integration tests.
pub fn start_with_endpoint(
    shared: &'static Shared,
    endpoint: String,
) -> io::Result<(ServerHandle, String)> {
    let (command_tx, command_rx) = mpsc::channel(COMMAND_QUEUE_CAPACITY);
    let handle = ServerHandle::new(command_tx);
    // Published here rather than by the caller: everything that speaks unprompted —
    // the hook thread, the rescue listener, a refused Run — reaches the client through
    // `Shared::broadcast`, and a caller that forgot this line would leave all of them
    // talking to nobody, silently. (An integration test is such a caller.)
    let _ = shared.server.set(handle.clone());
    let worker_handle = handle.clone();
    let accept_handle = handle.clone();
    let diagnostic = endpoint.clone();
    let (ready_tx, ready_rx) = std::sync::mpsc::sync_channel(1);

    std::thread::Builder::new()
        .name("lbm-local-ipc".to_string())
        .spawn(move || {
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()
                .expect("local IPC runtime");
            runtime.block_on(async move {
                // Tokio's Windows named-pipe constructor requires an active
                // reactor. Production starts IPC from synchronous main(), so
                // bind inside the runtime rather than before spawning it.
                let mut acceptor = match transport::Acceptor::bind(&endpoint) {
                    Ok(acceptor) => acceptor,
                    Err(error) => {
                        let _ = ready_tx.send(Err(error));
                        return;
                    }
                };
                let _ = ready_tx.send(Ok(()));
                tokio::spawn(command_worker(command_rx, worker_handle, shared));
                // Every connection is accepted: the newest client is the one that
                // matters, and refusing it would leave the hook driven by whoever got
                // there first — including a connection nobody is reading any more.
                while let Some(stream) = acceptor.accept().await {
                    tokio::spawn(run_connection(stream, accept_handle.clone(), shared));
                }
            });
        })?;

    ready_rx
        .recv()
        .map_err(|_| io::Error::other("local IPC thread exited before binding its endpoint"))??;

    Ok((handle, diagnostic))
}

async fn command_worker(
    mut commands: mpsc::Receiver<InboundCommand>,
    server: ServerHandle,
    shared: &'static Shared,
) {
    while let Some(command) = commands.recv().await {
        daemon::receive_message(&command.message, command.id, &server, shared);
        let _ = command.completed.send(());
    }
}

/// One client's connection, for as long as it lives.
///
/// It both listens and commands — the agent uses one connection for both, and a
/// subscription that stopped taking commands is a hook nobody can drive. There is no
/// idle deadline for the same reason: the agent is silent between a display change and
/// the next, and silence is not death. What ends the connection is the client going
/// away, the writer failing, or a newer client taking its place.
///
/// The last of those is not the client going away, and the difference is the whole of
/// "bound to the agent": a hook that left when a *newer* agent took it over would be
/// killed by the very agent that just adopted it.
async fn run_connection<S: LocalStream>(stream: S, server: ServerHandle, shared: &'static Shared) {
    static NEXT_ID: AtomicU64 = AtomicU64::new(1);
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let (mut reader, mut writer) = tokio::io::split(stream);
    let (outbound_tx, mut outbound_rx) = mpsc::channel::<String>(CLIENT_QUEUE_CAPACITY);
    let client = Arc::new(ClientHandle::new(id, outbound_tx));
    let evicted = client.evicted.clone();

    if let Some(previous) = server.adopt(client) {
        eprintln!(
            "[LittleBigMouse.Hook] a new client took the connection; client {} is done",
            previous.id
        );
        previous.evicted.notify_one();
    }

    let mut writer_task = tokio::spawn(async move {
        while let Some(message) = outbound_rx.recv().await {
            match tokio::time::timeout(WRITE_TIMEOUT, write_frame(&mut writer, &message)).await {
                Ok(Ok(())) => {}
                _ => break,
            }
        }
    });

    loop {
        let message = tokio::select! {
            read = read_frame(&mut reader) => match read {
                Ok(message) => message,
                Err(_) => break,
            },
            () = evicted.notified() => break,
            _ = &mut writer_task => break,
        };

        let (completed, result) = oneshot::channel();
        let command = InboundCommand {
            id,
            message,
            completed,
        };
        if server.commands.try_send(command).is_err() {
            break;
        }
        // The daemon has a deadline, the client does not: a command that hangs the
        // daemon must not hold the connection with it.
        if tokio::time::timeout(COMMAND_TIMEOUT, result).await.is_err() {
            break;
        }
    }

    // `remove` is the discriminator: after an eviction the newcomer has already taken
    // the handle, so nothing is removed here and this connection ending is not its
    // client going away.
    let was_the_client = server.remove(id);
    writer_task.abort();

    if was_the_client && shared.bound_to_agent.load(Ordering::SeqCst) {
        eprintln!(
            "[LittleBigMouse.Hook] the agent this hook belongs to is gone: \
             letting go of the mice and leaving"
        );
        // The unhook is what releases: the teardown pays the held buttons out on the
        // virtual pointer while it is still alive, then hands every mouse back.
        crate::hook::request_unhook(shared);
        crate::hook::request_quit(shared);
    }
}
