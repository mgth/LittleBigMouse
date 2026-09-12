//! A hook that hooks nothing: it listens on an endpoint, speaks the hook's protocol and
//! answers as `lbm-hook` does (`daemon/mod.rs`), without touching a single input
//! device. It is what `--fake-hook` runs, so the agent can be developed without
//! capturing the mice of whoever runs it, and what the tests talk to.
//!
//! The answers: `Hello` greets the asker, naming the layout held; `Listen` subscribes
//! and gets the current state; `Load` is put through the parser the daemon uses and
//! reported (`Loaded` with the document's own two zone counts, or `LoadFailed`), and
//! unhooks either way unless its frame also holds a `Run`; `Run` hooks (`Running`), or
//! is refused — reporting the state, as the daemon does — when nothing is loaded or the
//! layout is foreign; `Stop` unhooks (`Stopped`); `State` gets the state; `Shortcut` is
//! held; `Probe` gets an (empty) report; `Quit` unhooks and closes the endpoint. Every
//! command is recorded.
//!
//! Where it is tempting to make it simpler than the daemon, don't: a fake more willing
//! than the hook that ships lets a test pass on behaviour the product does not have,
//! and this one has hidden real bugs that way three times over — a greeting sent where
//! nobody was listening, a `Run` it granted unconditionally, and a `Load` it judged by
//! the look of the text.

use std::io;
use std::sync::{Arc, Mutex};

use lbm_ipc::framing::{read_frame, write_frame};
use lbm_ipc::protocol::{self, Command, Event};
use lbm_zones::ZonesLayout;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::mpsc;
use tokio::task::{AbortHandle, JoinHandle};

/// The fake's edge report: no zone, so no edge.
pub const PROBE_REPORT: &str = "<ProbeReport />";

#[derive(Default)]
struct State {
    hooked: bool,
    /// The layout it holds, as the real hook keeps it: the fingerprint of the last
    /// `Load` it accepted. A fake that always greeted with none would be agreeing
    /// with the agent rather than answering it.
    applied: String,
    /// And whether that layout is a foreign one, which the real daemon refuses to
    /// run: it would confine the local mouse inside a geometry that does not exist
    /// on this machine.
    applied_is_virtual: bool,
    /// The panic shortcut it was last told to adopt.
    shortcut: String,
    received: Vec<Command>,
    listeners: Vec<mpsc::UnboundedSender<String>>,
}

impl State {
    fn broadcast(&mut self, event: &Event) {
        let frame = protocol::event(event);
        self.listeners.retain(|l| l.send(frame.clone()).is_ok());
    }

    fn state(&self) -> Event {
        if self.hooked {
            Event::Running
        } else {
            Event::Stopped
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

    /// The panic shortcut it was last told to adopt; empty until it is told one.
    pub fn shortcut(&self) -> String {
        self.state.lock().unwrap().shortcut.clone()
    }

    /// Completes once a client sent `Quit`.
    pub async fn quit_requested(&self) {
        self.quit.notified().await;
    }

    /// Sends an event frame (see [`lbm_ipc::protocol`]) to every subscriber, as the real
    /// hook does when the system tells it something (a display change, a suspend).
    pub fn broadcast(&self, event: &Event) {
        self.state.lock().unwrap().broadcast(event);
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
                        let _ = out.send(protocol::event(&s.state()));
                    }
                    Command::State => {
                        let _ = out.send(protocol::event(&s.state()));
                    }
                    // Decided by the parser the daemon uses, not by the look of the
                    // text: an empty document is not the only one that fails, and the
                    // zone counts a `Loaded` reports are two different numbers.
                    Command::Load { zones: xml } => {
                        match ZonesLayout::from_xml(&xml) {
                            Some(layout) => {
                                s.applied = protocol::fingerprint(&xml);
                                s.applied_is_virtual = layout.virtual_layout;
                                s.broadcast(&Event::Loaded {
                                    zones: layout.zones.len(),
                                    main: layout.main_zones.len(),
                                    virtual_layout: layout.virtual_layout,
                                });
                            }
                            None => s.broadcast(&Event::LoadFailed),
                        }
                        // After the outcome, and whatever the outcome was. The daemon
                        // lets go *before* it looks at the document, so a Load that
                        // cannot be parsed takes the hook down too — but the letting go
                        // is a request its pump answers later, while the outcome is
                        // reported there and then. Hence this order on the wire.
                        if s.hooked && !rehooks {
                            s.hooked = false;
                            s.broadcast(&Event::Stopped);
                        }
                    }
                    // Refused where the real daemon refuses, and saying so as it does.
                    // A fake more willing than the hook that ships lets a test pass on
                    // behaviour the product does not have.
                    Command::Run if s.applied.is_empty() || s.applied_is_virtual => {
                        let state = s.state();
                        s.broadcast(&state);
                    }
                    Command::Run => {
                        s.hooked = true;
                        s.broadcast(&Event::Running);
                    }
                    Command::Stop => {
                        s.hooked = false;
                        s.broadcast(&Event::Stopped);
                    }
                    Command::Quit => {
                        s.hooked = false;
                        s.broadcast(&Event::Stopped);
                        quit = true;
                    }
                    // The fake speaks the protocol, handshake included: an agent that
                    // asks who is there has to get an answer here too.
                    //
                    // To the asker, as the real hook answers it — not to the
                    // subscribers. The agent opens with `[Hello, Listen]`, so at Hello
                    // time nobody is subscribed yet: broadcasting it sent it nowhere,
                    // and the agent, hearing nothing, took this fake for a hook older
                    // than the protocol and retired it every five seconds.
                    Command::Hello { .. } => {
                        let _ = out.send(protocol::event(&Event::Hello {
                            protocol: protocol::PROTOCOL,
                            version: "fake".to_owned(),
                            layout: s.applied.clone(),
                        }));
                    }
                    // Held, not dropped. The daemon adopts it and re-registers; what
                    // it makes of a blank one is its own policy, tested there. Here it
                    // is kept verbatim so a test can see what the agent actually sent —
                    // a known command has no business sharing an arm with unknown ones.
                    Command::Shortcut { text } => s.shortcut = text,
                    Command::Unknown => {}
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
