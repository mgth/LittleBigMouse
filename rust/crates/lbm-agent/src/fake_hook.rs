//! A hook that hooks nothing: it listens on an endpoint, speaks the hook's protocol and
//! answers as `lbm-hook` does (`daemon/mod.rs`), without touching a single input
//! device. It is what `--fake-hook` runs, so the agent can be developed without
//! capturing the mice of whoever runs it, and what the tests talk to.
//!
//! The answers: `Listen` subscribes and gets the current state; `Load` is reported
//! (`Loaded`, or `LoadFailed` for an empty layout) and unhooks unless its frame also
//! holds a `Run`; `Run` hooks (`Running`); `Stop` unhooks (`Stopped`); `State` gets the
//! state; `Quit` unhooks and closes the endpoint. Every command is recorded.

use std::io;
use std::sync::{Arc, Mutex};

use lbm_ipc::framing::{read_frame, write_frame};
use lbm_ipc::protocol::{self, Command};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::mpsc;
use tokio::task::{AbortHandle, JoinHandle};

#[derive(Default)]
struct State {
    hooked: bool,
    received: Vec<Command>,
    listeners: Vec<mpsc::UnboundedSender<String>>,
}

impl State {
    fn broadcast(&mut self, frame: &str) {
        self.listeners.retain(|l| l.send(frame.to_owned()).is_ok());
    }

    fn state_frame(&self) -> &'static str {
        if self.hooked {
            protocol::RUNNING
        } else {
            protocol::STOPPED
        }
    }
}

/// A fake hook listening on an endpoint until dropped.
pub struct FakeHook {
    state: Arc<Mutex<State>>,
    quit: Arc<tokio::sync::Notify>,
    accepting: JoinHandle<()>,
    /// Every connection's task, closed with the fake.
    connections: Arc<Mutex<Vec<AbortHandle>>>,
    #[cfg(unix)]
    path: std::path::PathBuf,
}

impl FakeHook {
    /// Listens on `endpoint` (a socket path, or a `\\.\pipe\...` name on Windows), on the
    /// current tokio runtime.
    pub fn bind(endpoint: &str) -> io::Result<FakeHook> {
        let state = Arc::new(Mutex::new(State::default()));
        let quit = Arc::new(tokio::sync::Notify::new());
        let connections = Arc::new(Mutex::new(Vec::new()));
        let accepting = accept(endpoint, state.clone(), quit.clone(), connections.clone())?;
        Ok(FakeHook {
            state,
            quit,
            accepting,
            connections,
            #[cfg(unix)]
            path: endpoint.into(),
        })
    }

    /// The commands received so far, in order.
    pub fn received(&self) -> Vec<Command> {
        self.state.lock().unwrap().received.clone()
    }

    /// Whether the last answer was to hook.
    pub fn hooked(&self) -> bool {
        self.state.lock().unwrap().hooked
    }

    /// Completes once a client sent `Quit`.
    pub async fn quit_requested(&self) {
        self.quit.notified().await;
    }

    /// Sends an event frame (see [`lbm_ipc::protocol`]) to every subscriber, as the real
    /// hook does when the system tells it something (a display change, a suspend).
    pub fn broadcast(&self, frame: &str) {
        self.state.lock().unwrap().broadcast(frame);
    }
}

impl Drop for FakeHook {
    fn drop(&mut self) {
        self.accepting.abort();
        for connection in self.connections.lock().unwrap().drain(..) {
            connection.abort();
        }
        self.state.lock().unwrap().listeners.clear();
        #[cfg(unix)]
        let _ = std::fs::remove_file(&self.path);
    }
}

type Connections = Arc<Mutex<Vec<AbortHandle>>>;

#[cfg(unix)]
fn accept(
    endpoint: &str,
    state: Arc<Mutex<State>>,
    quit: Arc<tokio::sync::Notify>,
    connections: Connections,
) -> io::Result<JoinHandle<()>> {
    let _ = std::fs::remove_file(endpoint);
    let listener = tokio::net::UnixListener::bind(endpoint)?;
    Ok(tokio::spawn(async move {
        while let Ok((stream, _)) = listener.accept().await {
            let task = tokio::spawn(connection(stream, state.clone(), quit.clone()));
            connections.lock().unwrap().push(task.abort_handle());
        }
    }))
}

#[cfg(windows)]
fn accept(
    endpoint: &str,
    state: Arc<Mutex<State>>,
    quit: Arc<tokio::sync::Notify>,
    connections: Connections,
) -> io::Result<JoinHandle<()>> {
    use tokio::net::windows::named_pipe::ServerOptions;

    let endpoint = endpoint.to_owned();
    let mut server = ServerOptions::new()
        .first_pipe_instance(true)
        .create(&endpoint)?;
    Ok(tokio::spawn(async move {
        loop {
            if server.connect().await.is_err() {
                break;
            }
            let next = match ServerOptions::new().create(&endpoint) {
                Ok(next) => next,
                Err(_) => break,
            };
            let connected = std::mem::replace(&mut server, next);
            let task = tokio::spawn(connection(connected, state.clone(), quit.clone()));
            connections.lock().unwrap().push(task.abort_handle());
        }
    }))
}

async fn connection<S>(
    stream: S,
    state: Arc<Mutex<State>>,
    quit_requested: Arc<tokio::sync::Notify>,
) where
    S: AsyncRead + AsyncWrite + Send + 'static,
{
    let (mut reader, mut writer) = tokio::io::split(stream);
    let (out, mut out_rx) = mpsc::unbounded_channel::<String>();
    let writing = tokio::spawn(async move {
        while let Some(frame) = out_rx.recv().await {
            if write_frame(&mut writer, &frame).await.is_err() {
                break;
            }
        }
    });

    while let Ok(frame) = read_frame(&mut reader).await {
        let commands = protocol::parse(&frame);
        let rehooks = commands.iter().any(|c| matches!(c, Command::Run));
        let mut quit = false;
        {
            let mut s = state.lock().unwrap();
            for command in commands {
                s.received.push(command.clone());
                match command {
                    Command::Listen => {
                        s.listeners.push(out.clone());
                        let _ = out.send(s.state_frame().to_owned());
                    }
                    Command::State => {
                        let _ = out.send(s.state_frame().to_owned());
                    }
                    Command::Load(xml) if xml.is_empty() => s.broadcast(protocol::LOAD_FAILED),
                    Command::Load(xml) => {
                        let zones = xml.matches("<Zone ").count();
                        let virtual_layout = xml.contains(r#"Virtual="True""#);
                        s.broadcast(&protocol::loaded(zones, zones, virtual_layout));
                        if s.hooked && !rehooks {
                            s.hooked = false;
                            s.broadcast(protocol::STOPPED);
                        }
                    }
                    Command::Run => {
                        s.hooked = true;
                        s.broadcast(protocol::RUNNING);
                    }
                    Command::Stop => {
                        s.hooked = false;
                        s.broadcast(protocol::STOPPED);
                    }
                    Command::Quit => {
                        s.hooked = false;
                        s.broadcast(protocol::STOPPED);
                        quit = true;
                    }
                    Command::LoadFromFile(_)
                    | Command::Probe
                    | Command::Shortcut(_)
                    | Command::Unknown(_) => {}
                }
            }
        }
        if quit {
            quit_requested.notify_one();
            break;
        }
    }
    // Subscribers' clones of `out` would keep the writer waiting: stop it outright.
    writing.abort();
}
