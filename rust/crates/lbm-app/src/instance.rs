//! One editor per session. A contender activates the owner, or asks it to hand over
//! its draft and close. Only the lock holder may remove or bind the control socket.
use lbm_ipc::{
    framing::{read_frame, write_frame},
    instance::{InstanceLock, EDITOR},
};
use serde_json::{json, Value};
use std::{
    io,
    path::PathBuf,
    sync::{mpsc, Arc, Mutex},
    thread,
    time::{Duration, Instant},
};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    sync::oneshot,
};

const DEADLINE: Duration = Duration::from_secs(10);

pub enum Event {
    Activate(oneshot::Sender<Value>),
    Replace(oneshot::Sender<Value>),
    Close,
}

pub struct Inbox {
    events: mpsc::Receiver<Event>,
    wake: Arc<Mutex<Option<egui::Context>>>,
}
impl Inbox {
    pub fn attach(&self, ctx: egui::Context) {
        *self.wake.lock().unwrap() = Some(ctx.clone());
        ctx.request_repaint();
    }
    pub fn pending(&self) -> Vec<Event> {
        self.events.try_iter().collect()
    }
}

/// Drop the listener before releasing the lock, including on failed window creation.
pub struct Owner {
    stop: Option<oneshot::Sender<()>>,
    worker: Option<thread::JoinHandle<()>>,
    _lock: InstanceLock,
}
impl Drop for Owner {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

pub enum Launch {
    Open {
        owner: Owner,
        inbox: Inbox,
        draft: Option<Value>,
    },
    Activated,
}

pub struct Address {
    #[cfg(unix)]
    lock: PathBuf,
    #[cfg(windows)]
    mutex: String,
    endpoint: PathBuf,
}
impl Address {
    pub fn session() -> io::Result<Self> {
        #[cfg(unix)]
        {
            let shared = InstanceLock::default_path(EDITOR);
            let stem = shared
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("lbm-editor");
            let session = graphical_session(
                std::env::var("XDG_SESSION_ID").ok().as_deref(),
                std::env::var("WAYLAND_DISPLAY")
                    .or_else(|_| std::env::var("DISPLAY"))
                    .ok()
                    .as_deref(),
            );
            let lock = shared.with_file_name(format!("{stem}{session}.lock"));
            Ok(Self {
                endpoint: lock.with_extension("sock"),
                lock,
            })
        }
        #[cfg(windows)]
        {
            use windows::Win32::System::{
                RemoteDesktop::ProcessIdToSessionId, Threading::GetCurrentProcessId,
            };
            let mut session = 0;
            unsafe { ProcessIdToSessionId(GetCurrentProcessId(), &mut session) }
                .map_err(io::Error::other)?;
            Ok(Self {
                mutex: EDITOR.mutex.into(),
                endpoint: format!(r"\\.\pipe\LittleBigMouse-Editor-session-{session}").into(),
            })
        }
    }
    #[cfg(unix)]
    pub fn in_directory(dir: &std::path::Path) -> Self {
        Self {
            lock: dir.join(EDITOR.file),
            endpoint: dir.join("lbm-editor.sock"),
        }
    }
    fn acquire(&self) -> io::Result<Option<InstanceLock>> {
        #[cfg(unix)]
        {
            InstanceLock::acquire(&self.lock)
        }
        #[cfg(windows)]
        {
            InstanceLock::acquire_named(&self.mutex)
        }
    }
}

/// One editor per desktop, not per user: two graphical sessions of the same user share
/// a runtime directory, and an editor activated on the other one would be invisible.
#[cfg(unix)]
fn graphical_session(login: Option<&str>, display: Option<&str>) -> String {
    [login, display]
        .into_iter()
        .flatten()
        .filter(|part| !part.is_empty())
        .map(|part| {
            let clean: String = part
                .chars()
                .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
                .take(24)
                .collect();
            format!("-{clean}")
        })
        .collect()
}

pub fn start(address: Address, replace: bool) -> io::Result<Launch> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    let deadline = Instant::now() + DEADLINE;
    let mut draft = None;
    let replacing = replace;
    let mut asked_to_close = false;
    loop {
        if let Some(lock) = address.acquire()? {
            let (owner, inbox) = listen(address.endpoint, lock)?;
            return Ok(Launch::Open {
                owner,
                inbox,
                draft,
            });
        }
        {
            let request =
                json!({"Protocol":1, "Command": if replacing { "Replace" } else { "Activate" }})
                    .to_string();
            let result = runtime.block_on(async {
                tokio::time::timeout(Duration::from_secs(3), async {
                    let mut stream = connect(&address.endpoint).await?;
                    write_frame(&mut stream, &request).await?;
                    let response: Value = serde_json::from_str(&read_frame(&mut stream).await?)
                        .map_err(io::Error::other)?;
                    if response["Protocol"] != 1 || response["Ok"] != true {
                        return Err(io::Error::other("the existing editor refused the request"));
                    }
                    Ok::<_, io::Error>(response)
                })
                .await
                .map_err(|_| {
                    io::Error::new(
                        io::ErrorKind::TimedOut,
                        "the existing editor did not respond",
                    )
                })?
            });
            match result {
                // The draft stays here until this launch opens with it: if another one
                // takes the lock first, it is asked to hand over and close in turn.
                Ok(response) if replacing => {
                    if let Some(handed) = response.get("Draft").filter(|v| !v.is_null()) {
                        draft = Some(handed.clone());
                    }
                    asked_to_close = true;
                }
                Ok(_) => return Ok(Launch::Activated),
                // Holding a draft, every failure is a window closing or starting: retry.
                Err(_) if asked_to_close => {}
                Err(error)
                    if !matches!(
                        error.kind(),
                        io::ErrorKind::NotFound
                            | io::ErrorKind::ConnectionRefused
                            | io::ErrorKind::WouldBlock
                    ) && !(cfg!(windows) && error.raw_os_error() == Some(231)) =>
                {
                    return Err(error)
                }
                Err(_) => {} // The owner may still be creating its endpoint.
            }
        }
        if Instant::now() >= deadline {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "the existing LittleBigMouse window did not release its instance lock",
            ));
        }
        thread::sleep(Duration::from_millis(40));
    }
}

type Wake = Arc<Mutex<Option<egui::Context>>>;
fn emit(events: &mpsc::Sender<Event>, wake: &Wake, event: Event) -> io::Result<()> {
    events.send(event).map_err(io::Error::other)?;
    if let Some(ctx) = wake.lock().unwrap().as_ref() {
        ctx.request_repaint();
    }
    Ok(())
}
fn listen(endpoint: PathBuf, lock: InstanceLock) -> io::Result<(Owner, Inbox)> {
    let (events, received) = mpsc::channel();
    let wake: Wake = Arc::default();
    let waker = wake.clone();
    let (stop, stopped) = oneshot::channel();
    let (ready, started) = mpsc::sync_channel(1);
    let worker = thread::spawn(move || {
        let runtime = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(r) => r,
            Err(error) => {
                let _ = ready.send(Err(error));
                return;
            }
        };
        runtime.block_on(async {
            let listener = match Listener::bind(&endpoint) {
                Ok(l) => l,
                Err(e) => {
                    let _ = ready.send(Err(e));
                    return;
                }
            };
            let _ = ready.send(Ok(()));
            tokio::select! {
                _ = stopped => {},
                _ = serve(listener, &endpoint, &events, &waker) => {},
            }
        });
        #[cfg(unix)]
        let _ = std::fs::remove_file(endpoint);
    });
    let owner = Owner {
        stop: Some(stop),
        worker: Some(worker),
        _lock: lock,
    };
    started
        .recv_timeout(Duration::from_secs(3))
        .map_err(io::Error::other)??;
    Ok((
        owner,
        Inbox {
            events: received,
            wake,
        },
    ))
}

async fn serve(
    mut listener: Listener,
    endpoint: &std::path::Path,
    events: &mpsc::Sender<Event>,
    wake: &Wake,
) {
    while let Ok(mut stream) = listener.accept(endpoint).await {
        let result = tokio::time::timeout(Duration::from_secs(3), async {
            let request: Value =
                serde_json::from_str(&read_frame(&mut stream).await?).map_err(io::Error::other)?;
            if request["Protocol"] != 1 {
                return Err(io::Error::other("unsupported editor protocol"));
            }
            let (answer, response) = oneshot::channel();
            let replace = match request["Command"].as_str() {
                Some("Activate") => {
                    emit(events, wake, Event::Activate(answer))?;
                    false
                }
                Some("Replace") => {
                    emit(events, wake, Event::Replace(answer))?;
                    true
                }
                _ => return Err(io::Error::other("unknown editor command")),
            };
            let draft = response.await.map_err(io::Error::other)?;
            write_frame(
                &mut stream,
                &json!({"Protocol":1, "Ok":true, "Draft":draft}).to_string(),
            )
            .await?;
            // Only close once the successor has received the handover. It waits for
            // the actual lock release before constructing any native window.
            if replace {
                emit(events, wake, Event::Close)?;
            }
            Ok::<_, io::Error>(replace)
        })
        .await;
        match result {
            // Handed over: a closing editor takes no more requests, so the successor
            // finds the next owner rather than this one again.
            Ok(Ok(true)) => return,
            Ok(Err(error)) => eprintln!("[lbm-app] instance request: {error}"),
            _ => {}
        }
    }
}

trait Duplex: AsyncRead + AsyncWrite + Unpin + Send {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send> Duplex for T {}
type Stream = Box<dyn Duplex>;
#[cfg(unix)]
struct Listener(tokio::net::UnixListener);
#[cfg(unix)]
impl Listener {
    fn bind(path: &std::path::Path) -> io::Result<Self> {
        use std::os::unix::fs::PermissionsExt;
        match std::fs::remove_file(path) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(e),
        }
        let listener = tokio::net::UnixListener::bind(path)?;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
        Ok(Self(listener))
    }
    async fn accept(&mut self, _: &std::path::Path) -> io::Result<Stream> {
        Ok(Box::new(self.0.accept().await?.0))
    }
}
#[cfg(unix)]
async fn connect(path: &std::path::Path) -> io::Result<Stream> {
    Ok(Box::new(tokio::net::UnixStream::connect(path).await?))
}
#[cfg(windows)]
struct Listener(tokio::net::windows::named_pipe::NamedPipeServer);
#[cfg(windows)]
impl Listener {
    fn bind(path: &std::path::Path) -> io::Result<Self> {
        Ok(Self(
            tokio::net::windows::named_pipe::ServerOptions::new()
                .first_pipe_instance(true)
                .reject_remote_clients(true)
                .create(path)?,
        ))
    }
    async fn accept(&mut self, path: &std::path::Path) -> io::Result<Stream> {
        self.0.connect().await?;
        let next = tokio::net::windows::named_pipe::ServerOptions::new()
            .reject_remote_clients(true)
            .create(path)?;
        Ok(Box::new(std::mem::replace(&mut self.0, next)))
    }
}
#[cfg(windows)]
async fn connect(path: &std::path::Path) -> io::Result<Stream> {
    use std::os::windows::io::AsRawHandle;
    use windows::Win32::{
        Foundation::HANDLE, System::Pipes::GetNamedPipeServerProcessId,
        UI::WindowsAndMessaging::AllowSetForegroundWindow,
    };
    let pipe = tokio::net::windows::named_pipe::ClientOptions::new().open(path)?;
    let mut pid = 0;
    // The newly launched process owns the foreground permission. Give it only to
    // the editor at the other end of this pipe before asking that editor to focus.
    unsafe {
        if GetNamedPipeServerProcessId(HANDLE(pipe.as_raw_handle()), &mut pid).is_ok() {
            let _ = AllowSetForegroundWindow(pid);
        }
    }
    Ok(Box::new(pipe))
}

#[cfg(all(test, unix))]
mod tests {
    #[test]
    fn each_graphical_session_has_its_own_editor() {
        let one = super::graphical_session(Some("2"), Some("wayland-0"));
        let other = super::graphical_session(Some("5"), Some("wayland-0"));
        let nested = super::graphical_session(Some("2"), Some("wayland-1"));
        assert_eq!(one, "-2-wayland_0");
        assert_ne!(one, other);
        assert_ne!(one, nested);
        assert_eq!(super::graphical_session(None, Some("")), "");
        assert_eq!(super::graphical_session(Some("../x"), None), "-___x");
    }
}
