//! Bounded, per-user local IPC server.
//!
//! Windows uses a current-session named pipe whose DACL grants only the current
//! user and SYSTEM. Linux uses a 0600 Unix-domain socket. Both transports share
//! the same length-prefixed UTF-8 protocol, four-client cap, ordered command
//! queue, and non-blocking outbound queues.

use std::collections::HashMap;
use std::io;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::{mpsc, oneshot, Semaphore};

use crate::daemon;
use crate::ipc::framing::{read_frame, write_frame};
use crate::shared::Shared;

pub type ClientId = u64;

const MAX_CLIENTS: usize = 4;
const COMMAND_QUEUE_CAPACITY: usize = 64;
const CLIENT_QUEUE_CAPACITY: usize = 16;
const COMMAND_TIMEOUT: Duration = Duration::from_secs(5);
const WRITE_TIMEOUT: Duration = Duration::from_secs(2);

struct ClientHandle {
    id: ClientId,
    outbound: mpsc::Sender<String>,
    listening: AtomicBool,
}

impl ClientHandle {
    fn new(id: ClientId, outbound: mpsc::Sender<String>) -> Self {
        Self {
            id,
            outbound,
            listening: AtomicBool::new(false),
        }
    }
}

struct InboundCommand {
    id: ClientId,
    message: String,
    completed: oneshot::Sender<bool>,
}

/// Cloneable synchronous facade used by daemon and hook callbacks.
#[derive(Clone)]
pub struct ServerHandle {
    registry: Arc<Mutex<HashMap<ClientId, Arc<ClientHandle>>>>,
    commands: mpsc::Sender<InboundCommand>,
}

impl ServerHandle {
    fn new(commands: mpsc::Sender<InboundCommand>) -> Self {
        Self {
            registry: Arc::new(Mutex::new(HashMap::new())),
            commands,
        }
    }

    fn insert(&self, client: Arc<ClientHandle>) -> bool {
        let mut registry = self.registry.lock().unwrap_or_else(|p| p.into_inner());
        if registry.len() >= MAX_CLIENTS {
            return false;
        }
        registry.insert(client.id, client);
        true
    }

    pub fn remove(&self, id: ClientId) {
        self.registry
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .remove(&id);
    }

    fn get(&self, id: ClientId) -> Option<Arc<ClientHandle>> {
        self.registry
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .get(&id)
            .cloned()
    }

    pub fn set_listening(&self, id: ClientId) {
        if let Some(client) = self.get(id) {
            client.listening.store(true, Ordering::SeqCst);
        }
    }

    pub fn send_to(&self, id: ClientId, message: &str) {
        if let Some(client) = self.get(id) {
            if client.outbound.try_send(message.to_string()).is_err() {
                self.remove(id);
            }
        }
    }

    /// Never blocks the hook/message-pump thread. Slow clients have a bounded
    /// queue and are disconnected rather than delaying input routing.
    pub fn broadcast(&self, message: &str) {
        let clients: Vec<Arc<ClientHandle>> = self
            .registry
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .values()
            .filter(|client| client.listening.load(Ordering::SeqCst))
            .cloned()
            .collect();

        for client in clients {
            if client.outbound.try_send(message.to_string()).is_err() {
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
    let acceptor = transport::Acceptor::bind(&endpoint)?;
    let (command_tx, command_rx) = mpsc::channel(COMMAND_QUEUE_CAPACITY);
    let handle = ServerHandle::new(command_tx);
    let worker_handle = handle.clone();
    let accept_handle = handle.clone();
    let diagnostic = endpoint.clone();

    std::thread::Builder::new()
        .name("lbm-local-ipc".to_string())
        .spawn(move || {
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()
                .expect("local IPC runtime");
            runtime.block_on(async move {
                tokio::spawn(command_worker(command_rx, worker_handle, shared));
                acceptor.run(accept_handle, shared).await;
            });
        })?;

    Ok((handle, diagnostic))
}

async fn command_worker(
    mut commands: mpsc::Receiver<InboundCommand>,
    server: ServerHandle,
    shared: &'static Shared,
) {
    while let Some(command) = commands.recv().await {
        let listening = daemon::receive_message(&command.message, command.id, &server, shared);
        let _ = command.completed.send(listening);
    }
}

async fn run_connection<S: LocalStream>(
    stream: S,
    server: ServerHandle,
    _permit: tokio::sync::OwnedSemaphorePermit,
) {
    static NEXT_ID: AtomicU64 = AtomicU64::new(1);
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let (mut reader, mut writer) = tokio::io::split(stream);
    let (outbound_tx, mut outbound_rx) = mpsc::channel::<String>(CLIENT_QUEUE_CAPACITY);
    let client = Arc::new(ClientHandle::new(id, outbound_tx));
    if !server.insert(client) {
        return;
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
        let message = match tokio::time::timeout(COMMAND_TIMEOUT, read_frame(&mut reader)).await {
            Ok(Ok(message)) => message,
            _ => break,
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
        let listening = match tokio::time::timeout(COMMAND_TIMEOUT, result).await {
            Ok(Ok(listening)) => listening,
            _ => break,
        };
        if listening {
            tokio::select! {
                _ = read_frame(&mut reader) => {}
                _ = &mut writer_task => {}
            }
            server.remove(id);
            writer_task.abort();
            return;
        }
    }

    server.remove(id);
    let _ = writer_task.await;
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
            let semaphore = Arc::new(Semaphore::new(MAX_CLIENTS));
            let mut next = Some(self.first);
            loop {
                let pipe = match next.take() {
                    Some(first) => first,
                    None => match create_pipe(&self.endpoint, false) {
                        Ok(pipe) => pipe,
                        Err(_) => break,
                    },
                };
                if pipe.connect().await.is_err() {
                    continue;
                }
                if !client_is_current_session(&pipe) {
                    continue;
                }
                let Ok(permit) = semaphore.clone().try_acquire_owned() else {
                    continue;
                };
                tokio::spawn(run_connection(pipe, server.clone(), permit));
            }
        }
    }

    pub fn default_endpoint() -> io::Result<String> {
        Ok(format!(
            r"\\.\pipe\LittleBigMouse-v1-session-{}",
            current_session_id()?
        ))
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
                std::fs::create_dir_all(parent)?;
                std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700))?;
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
            let semaphore = Arc::new(Semaphore::new(MAX_CLIENTS));
            while let Ok((stream, _)) = self.listener.accept().await {
                let Ok(permit) = semaphore.clone().try_acquire_owned() else {
                    continue;
                };
                tokio::spawn(run_connection(stream, server.clone(), permit));
            }
            let _ = std::fs::remove_file(&self.path);
        }
    }

    pub fn default_endpoint() -> io::Result<String> {
        let base = std::env::var_os("XDG_RUNTIME_DIR")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .or_else(|| crate::platform::paths::lbm_data_file(""))
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::NotFound, "no per-user runtime directory")
            })?;
        Ok(base
            .join("littlebigmouse-v1.sock")
            .to_string_lossy()
            .into_owned())
    }
}
