//! Single-client, per-user local IPC server.
//!
//! Windows uses a current-session named pipe whose DACL grants only the current
//! user and SYSTEM. Linux uses a 0600 Unix-domain socket. Both transports share
//! the same length-prefixed UTF-8 protocol, ordered command queue, and
//! non-blocking outbound queue.
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

    /// Give the connection up — but only if it is still ours. A connection that ends
    /// after being evicted must not take its successor's place with it.
    pub fn remove(&self, id: ClientId) {
        let mut held = self.client.lock().unwrap_or_else(|p| p.into_inner());
        if held.as_ref().is_some_and(|client| client.id == id) {
            *held = None;
        }
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
                let acceptor = match transport::Acceptor::bind(&endpoint) {
                    Ok(acceptor) => acceptor,
                    Err(error) => {
                        let _ = ready_tx.send(Err(error));
                        return;
                    }
                };
                let _ = ready_tx.send(Ok(()));
                tokio::spawn(command_worker(command_rx, worker_handle, shared));
                acceptor.run(accept_handle, shared).await;
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
async fn run_connection<S: LocalStream>(stream: S, server: ServerHandle) {
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

    server.remove(id);
    writer_task.abort();
}

#[cfg(windows)]
mod transport {
    use super::*;
    use std::os::windows::io::AsRawHandle;

    use tokio::net::windows::named_pipe::{NamedPipeServer, ServerOptions};
    use windows::core::{HSTRING, PWSTR};
    use windows::Win32::Foundation::{CloseHandle, LocalFree, BOOL, HANDLE, HLOCAL};
    use windows::Win32::Security::Authorization::{
        ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW,
        SDDL_REVISION_1,
    };
    use windows::Win32::Security::{
        GetTokenInformation, TokenUser, PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES, TOKEN_QUERY,
        TOKEN_USER,
    };
    use windows::Win32::System::Pipes::GetNamedPipeClientProcessId;
    use windows::Win32::System::RemoteDesktop::ProcessIdToSessionId;
    use windows::Win32::System::Threading::{
        GetCurrentProcess, GetCurrentProcessId, OpenProcessToken,
    };

    pub struct Acceptor {
        endpoint: String,
        first: NamedPipeServer,
    }

    impl Acceptor {
        pub fn bind(endpoint: &str) -> io::Result<Self> {
            Ok(Self {
                endpoint: endpoint.to_string(),
                first: create_pipe(endpoint, true)?,
            })
        }

        pub async fn run(self, server: ServerHandle, _shared: &'static Shared) {
            let mut next = Some(self.first);
            loop {
                let pipe = match next.take() {
                    Some(first) => first,
                    None => match create_pipe(&self.endpoint, false) {
                        Ok(pipe) => pipe,
                        Err(error) => {
                            // A transient failure (handle pressure, AV
                            // interference) must not kill IPC for the rest of
                            // the daemon's life: log, back off, retry.
                            eprintln!(
                                "[LittleBigMouse.Hook] local IPC accept error: {error}; retrying"
                            );
                            tokio::time::sleep(Duration::from_millis(500)).await;
                            continue;
                        }
                    },
                };
                if pipe.connect().await.is_err() {
                    continue;
                }
                if !client_is_current_session(&pipe) {
                    continue;
                }
                // Every connection is accepted: the newest client is the one that
                // matters, and refusing it would leave the hook driven by whoever got
                // there first — including a connection nobody is reading any more.
                tokio::spawn(run_connection(pipe, server.clone()));
            }
        }
    }

    pub fn default_endpoint() -> io::Result<String> {
        Ok(lbm_ipc::endpoint::pipe_name(current_session_id()?))
    }

    fn current_session_id() -> io::Result<u32> {
        let mut session = 0;
        unsafe { ProcessIdToSessionId(GetCurrentProcessId(), &mut session) }
            .map_err(io::Error::other)?;
        Ok(session)
    }

    fn client_is_current_session(pipe: &NamedPipeServer) -> bool {
        let handle = HANDLE(pipe.as_raw_handle());
        let mut client_pid = 0;
        let mut client_session = 0;
        unsafe { GetNamedPipeClientProcessId(handle, &mut client_pid) }.is_ok()
            && unsafe { ProcessIdToSessionId(client_pid, &mut client_session) }.is_ok()
            && current_session_id().is_ok_and(|session| session == client_session)
    }

    fn create_pipe(endpoint: &str, first: bool) -> io::Result<NamedPipeServer> {
        let sid = current_user_sid()?;
        let sddl = HSTRING::from(format!("D:P(A;;GA;;;SY)(A;;GA;;;{sid})"));
        let mut descriptor = PSECURITY_DESCRIPTOR::default();
        unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                &sddl,
                SDDL_REVISION_1,
                &mut descriptor,
                None,
            )
        }
        .map_err(io::Error::other)?;

        let mut attributes = SECURITY_ATTRIBUTES {
            nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: descriptor.0,
            bInheritHandle: BOOL(0),
        };
        let result = unsafe {
            ServerOptions::new()
                .first_pipe_instance(first)
                .reject_remote_clients(true)
                .create_with_security_attributes_raw(
                    endpoint,
                    (&mut attributes as *mut SECURITY_ATTRIBUTES).cast(),
                )
        };
        unsafe {
            let _ = LocalFree(HLOCAL(descriptor.0));
        }
        result
    }

    fn current_user_sid() -> io::Result<String> {
        let mut token = HANDLE::default();
        unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) }
            .map_err(io::Error::other)?;

        let result = (|| {
            let mut length = 0;
            let _ = unsafe { GetTokenInformation(token, TokenUser, None, 0, &mut length) };
            if length == 0 {
                return Err(io::Error::last_os_error());
            }
            let mut buffer = vec![0u8; length as usize];
            unsafe {
                GetTokenInformation(
                    token,
                    TokenUser,
                    Some(buffer.as_mut_ptr().cast()),
                    length,
                    &mut length,
                )
            }
            .map_err(io::Error::other)?;
            let user = unsafe { &*(buffer.as_ptr() as *const TOKEN_USER) };
            let mut text = PWSTR::null();
            unsafe { ConvertSidToStringSidW(user.User.Sid, &mut text) }
                .map_err(io::Error::other)?;
            let sid = unsafe { text.to_string() }.map_err(io::Error::other);
            unsafe {
                let _ = LocalFree(HLOCAL(text.0.cast()));
            }
            sid
        })();

        unsafe {
            let _ = CloseHandle(token);
        }
        result
    }
}

#[cfg(target_os = "linux")]
mod transport {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;

    use tokio::net::UnixListener;

    pub struct Acceptor {
        listener: UnixListener,
        path: PathBuf,
    }

    impl Acceptor {
        pub fn bind(endpoint: &str) -> io::Result<Self> {
            let path = PathBuf::from(endpoint);
            if let Some(parent) = path.parent() {
                // Only restrict a directory we create ourselves: the normal
                // parents (XDG_RUNTIME_DIR, the LBM data dir) already exist
                // with correct ownership, and chmod on a shared parent like
                // /tmp is EPERM. The socket itself is chmod 0600 below.
                if !parent.exists() {
                    std::fs::create_dir_all(parent)?;
                    std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700))?;
                }
            }
            match std::fs::remove_file(&path) {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(error),
            }
            let listener = UnixListener::bind(&path)?;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
            Ok(Self { listener, path })
        }

        pub async fn run(self, server: ServerHandle, _shared: &'static Shared) {
            while let Ok((stream, _)) = self.listener.accept().await {
                // Every connection is accepted: see the Windows side.
                tokio::spawn(run_connection(stream, server.clone()));
            }
            let _ = std::fs::remove_file(&self.path);
        }
    }

    pub fn default_endpoint() -> io::Result<String> {
        let runtime = std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from);
        let data = crate::platform::paths::lbm_data_file("");
        lbm_ipc::endpoint::socket_path(runtime.as_deref(), data.as_deref())
            .map(|path| path.to_string_lossy().into_owned())
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no per-user runtime directory"))
    }
}
