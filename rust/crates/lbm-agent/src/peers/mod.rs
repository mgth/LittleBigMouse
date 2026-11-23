//! The multi-PC mode in the agent (docs/multi-pc.md, lot 5): the network node, the
//! pairings, and the relay of the input stream between the network and the hook.
//!
//! [`Service`] is what the agent's loop holds. It is synchronous and never waits: while
//! the mode is off it is a settings file and the pairing store, read when asked, and
//! **nothing runs** — no listener, no task, no timer. Turned on, it starts the manager
//! (`actor`), a task of its own that owns the listener, the pairings waiting for the
//! user, the links and the reconnections, and it forwards the agent's requests there;
//! the answers come back as futures the agent awaits in tasks of their own, the way it
//! answers the TV remote and VCP requests. What changes on the manager's side comes
//! back as a [`Notice`] carrying the whole state, which the agent pushes to its
//! subscribers.
//!
//! Roles are per link and stated in `Hello`: whoever connects is the master, whoever
//! listens the remote. Per link:
//!
//! - **this machine is the remote**: it sends its screens (`Displays`) as soon as the
//!   link is up and again whenever they change — the agent says when, from its own
//!   rebuild path — and relays every input batch, unread, to its hook's input endpoint
//!   (`hook_input`); when the link ends, that connection is closed so the hook releases
//!   what the stream left pressed;
//! - **this machine is the master**: it keeps the remote's latest screens and shows them
//!   in the state, and relays the batches its hook writes to the remote's link. **One
//!   remote at a time**: the stream goes to the most recently linked remote (lot 6 will
//!   route by zone). A paired remote whose address is known is called back while the
//!   mode is on, after one second, then twice as long at every failure, up to thirty.
//!
//! An unknown machine is shown as `Pending` and gets nothing but `Hello` until the user
//! accepts it (stored in `peers.json`, then `Paired` on the link) or refuses it.

mod actor;
pub mod hook_input;
pub mod local;
pub mod settings;

use std::future::Future;
use std::net::{IpAddr, Ipv4Addr};
use std::path::PathBuf;
use std::pin::Pin;
use std::time::Duration;

use lbm_ipc::peer::{Peer, PeerDisplay};
use lbm_net::identity::{Identity, IDENTITY_FILE};
use lbm_net::link;
use lbm_net::peers::PeerStore;
use lbm_net::wire::Role;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::{mpsc, oneshot};

use actor::Command;
pub use settings::Settings;

/// How long turning the mode off waits for the links to close.
const STOP_PATIENCE: Duration = Duration::from_secs(2);

/// Where a machine stands with this one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PeerState {
    /// In `peers.json`: trusted, linked or not.
    Paired,
    /// Connected and unknown, waiting for the user to accept or refuse it.
    Pending,
}

/// A machine as the frontends see it: [`Peer`], and where it stands.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct PeerEntry {
    #[serde(flatten)]
    pub peer: Peer,
    /// The role that machine plays on the link with this one.
    pub role: Role,
    pub state: PeerState,
}

/// The answer to `Peers`, and the body of the `Peers` event.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct PeersState {
    pub enabled: bool,
    pub port: u16,
    /// What this machine is called on the other machines.
    pub name: String,
    /// This machine's fingerprint, to compare with what the other machine shows when
    /// pairing. `None` while the mode has never been on (no identity is made before).
    pub machine_id: Option<String>,
    /// Where this node listens, while it does.
    pub listen_address: Option<String>,
    /// Why the mode is not fully working, when it is not.
    pub error: Option<String>,
    pub peers: Vec<PeerEntry>,
}

/// A machine this one is the master of, and the screens it reported: what the layout
/// merges (docs/multi-pc.md, "Model").
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct RemoteScreens {
    /// Its certificate fingerprint: the host of the sources merged from it.
    pub machine_id: String,
    /// What the map shows it as.
    pub name: String,
    pub displays: Vec<PeerDisplay>,
}

impl PeersState {
    /// The screens the layout is to hold besides this machine's: those of every paired
    /// machine linked as a remote of this one, in machine-id order so that the same
    /// machines always give the same layout.
    ///
    /// A machine this one is the remote of is never merged: its screens are its own
    /// master's business, and it sends none anyway. A pending machine is not trusted
    /// yet, and a linked remote that has not reported its screens has nothing to add.
    pub fn remote_screens(&self) -> Vec<RemoteScreens> {
        if !self.enabled {
            return Vec::new();
        }
        let mut remotes: Vec<RemoteScreens> = self
            .peers
            .iter()
            .filter(|p| {
                p.state == PeerState::Paired
                    && p.role == Role::Remote
                    && p.peer.connected
                    && !p.peer.displays.is_empty()
            })
            .map(|p| RemoteScreens {
                machine_id: p.peer.machine_id.clone(),
                name: p.peer.name.clone(),
                displays: p.peer.displays.clone(),
            })
            .collect();
        remotes.sort_by(|a, b| a.machine_id.cmp(&b.machine_id));
        remotes
    }
}

/// What the manager tells the agent.
#[derive(Clone, Debug)]
pub enum Notice {
    /// The state changed (a machine connected, paired, left, or reported its screens).
    Peers(PeersState),
}

/// A frontend's request, as far as the multi-PC mode goes.
#[derive(Clone, Debug, PartialEq)]
pub enum PeerRequest {
    State,
    Set {
        enabled: bool,
        port: Option<u16>,
        name: Option<String>,
    },
    Connect(String),
    Accept(String),
    Refuse(String),
    Forget(String),
}

/// An answer now, or one to await (in a task of the caller's, never its loop).
pub enum Answer {
    Now(Result<Value, String>),
    Later(Pin<Box<dyn Future<Output = Result<Value, String>> + Send>>),
}

/// What is fixed for the life of the agent.
#[derive(Clone, Debug)]
pub struct Setup {
    /// The address to listen on: every interface, unless a test says loopback.
    pub bind: IpAddr,
    /// The hook's input endpoint (`lbm_ipc::input::input_endpoint_beside`); `None`
    /// when there is no hook to inject into or capture from.
    pub input_endpoint: Option<String>,
    pub options: link::Options,
}

impl Setup {
    pub fn new(input_endpoint: Option<String>) -> Setup {
        Setup {
            bind: IpAddr::V4(Ipv4Addr::UNSPECIFIED),
            input_endpoint,
            options: link::Options::default(),
        }
    }
}

/// The running manager.
struct Running {
    commands: mpsc::UnboundedSender<Command>,
    /// Answered once the listener is up, or could not be.
    started: Option<oneshot::Receiver<()>>,
}

impl Running {
    /// Asks the manager to stop and waits for it to have closed its links.
    async fn stop(self) {
        let (done, wait) = oneshot::channel();
        if self.commands.send(Command::Shutdown(done)).is_ok() {
            let _ = tokio::time::timeout(STOP_PATIENCE, wait).await;
        }
    }
}

/// The multi-PC mode, as the agent's loop holds it.
pub struct Service {
    directory: PathBuf,
    setup: Setup,
    settings: Settings,
    running: Option<Running>,
    /// Why the mode is on and yet not running (an identity file that cannot be read).
    failure: Option<String>,
    /// This machine's screens, as last told: the manager starts with them.
    displays: Vec<PeerDisplay>,
    notices: mpsc::UnboundedSender<Notice>,
    notices_rx: Option<mpsc::UnboundedReceiver<Notice>>,
}

impl Service {
    /// The mode as `directory` (the configuration directory) says, started if it is on.
    /// Inside a Tokio runtime.
    pub fn new(directory: PathBuf, setup: Setup) -> Service {
        let settings = Settings::load(&directory);
        let (notices, notices_rx) = mpsc::unbounded_channel();
        let mut service = Service {
            directory,
            setup,
            settings,
            running: None,
            failure: None,
            displays: Vec::new(),
            notices,
            notices_rx: Some(notices_rx),
        };
        if service.settings.enabled {
            service.start(None);
        }
        service
    }

    /// The manager's notices, for the agent's loop: taken once.
    pub fn take_notices(&mut self) -> Option<mpsc::UnboundedReceiver<Notice>> {
        self.notices_rx.take()
    }

    /// Whether the mode is on: the agent only reads its screens for the remotes then.
    pub fn enabled(&self) -> bool {
        self.settings.enabled
    }

    /// This machine's screens, after a rebuild (or when the mode is turned on).
    pub fn local_displays(&mut self, displays: Vec<PeerDisplay>) {
        self.displays = displays.clone();
        if let Some(running) = &self.running {
            let _ = running.commands.send(Command::LocalDisplays(displays));
        }
    }

    /// Turns the mode off for the agent's way out: the peers get `Bye`.
    pub async fn stop(&mut self) {
        if let Some(running) = self.running.take() {
            running.stop().await;
        }
    }

    pub fn request(&mut self, request: PeerRequest) -> Answer {
        match request {
            PeerRequest::Set {
                enabled,
                port,
                name,
            } => self.set(enabled, port, name),
            PeerRequest::State => match &self.running {
                Some(running) => {
                    let (reply, answer) = oneshot::channel();
                    let sent = running.commands.send(Command::State(reply)).is_ok();
                    Answer::Later(Box::pin(async move {
                        if !sent {
                            return Err(STOPPED.to_owned());
                        }
                        let state = answer.await.map_err(|_| STOPPED.to_owned())?;
                        serde_json::to_value(state).map_err(|e| e.to_string())
                    }))
                }
                None => Answer::Now(
                    serde_json::to_value(self.offline_state()).map_err(|e| e.to_string()),
                ),
            },
            // Forgetting needs no network: done on the store when the mode is off.
            PeerRequest::Forget(machine_id) if self.running.is_none() => {
                let mut store = PeerStore::load(&self.directory);
                Answer::Now(match store.remove(&machine_id) {
                    Ok(true) => {
                        let _ = self.notices.send(Notice::Peers(self.offline_state()));
                        Ok(Value::Null)
                    }
                    Ok(false) => Err(format!("no machine {machine_id} is known")),
                    Err(error) => Err(format!("cannot forget {machine_id}: {error}")),
                })
            }
            request => {
                let Some(running) = &self.running else {
                    return Answer::Now(Err(self
                        .failure
                        .clone()
                        .unwrap_or_else(|| "the multi-PC mode is off".to_owned())));
                };
                let (reply, answer) = oneshot::channel();
                let command = match request {
                    PeerRequest::Connect(address) => Command::Connect { address, reply },
                    PeerRequest::Accept(machine_id) => Command::Accept { machine_id, reply },
                    PeerRequest::Refuse(machine_id) => Command::Refuse { machine_id, reply },
                    PeerRequest::Forget(machine_id) => Command::Forget { machine_id, reply },
                    PeerRequest::State | PeerRequest::Set { .. } => unreachable!("handled above"),
                };
                let sent = running.commands.send(command).is_ok();
                Answer::Later(Box::pin(async move {
                    if !sent {
                        return Err(STOPPED.to_owned());
                    }
                    answer.await.map_err(|_| STOPPED.to_owned())?
                }))
            }
        }
    }

    /// `SetMultiPc`: saved, then the manager (re)started or stopped to match. The
    /// answer is the state once that is done.
    fn set(&mut self, enabled: bool, port: Option<u16>, name: Option<String>) -> Answer {
        let mut next = self.settings.clone();
        next.enabled = enabled;
        if let Some(port) = port {
            next.port = port;
        }
        if let Some(name) = name {
            let name = name.trim().to_owned();
            next.name = (!name.is_empty()).then_some(name);
        }
        if let Err(error) = next.save(&self.directory) {
            return Answer::Now(Err(format!("cannot save the multi-PC settings: {error}")));
        }
        let unchanged = next == self.settings && (self.running.is_some() || !enabled);
        self.settings = next;
        if unchanged {
            return self.request(PeerRequest::State);
        }
        let previous = self.running.take();
        if enabled {
            self.start(previous);
            let started = self.running.as_mut().and_then(|r| r.started.take());
            let state = self.request(PeerRequest::State);
            let failure = self.failure.clone();
            return Answer::Later(Box::pin(async move {
                if let Some(failure) = failure {
                    return Err(failure);
                }
                if let Some(started) = started {
                    let _ = started.await;
                }
                match state {
                    Answer::Now(result) => result,
                    Answer::Later(state) => state.await,
                }
            }));
        }
        self.failure = None;
        let offline = self.offline_state();
        let notices = self.notices.clone();
        Answer::Later(Box::pin(async move {
            if let Some(previous) = previous {
                previous.stop().await;
            }
            let _ = notices.send(Notice::Peers(offline.clone()));
            serde_json::to_value(offline).map_err(|e| e.to_string())
        }))
    }

    /// Starts the manager, once `previous` (if any) has let go of the port.
    fn start(&mut self, previous: Option<Running>) {
        self.failure = None;
        // Made the first time the mode is turned on, never before: an agent that never
        // used the mode leaves no identity behind.
        let identity = match Identity::load_or_create(&self.directory) {
            Ok(identity) => identity,
            Err(error) => {
                let failure = format!("the machine identity cannot be read: {error}");
                eprintln!("[lbm-agent] multi-PC: {failure}");
                self.failure = Some(failure);
                // Whatever ran before is stopped all the same: the settings changed.
                if let Some(previous) = previous {
                    tokio::spawn(previous.stop());
                }
                let _ = self.notices.send(Notice::Peers(self.offline_state()));
                return;
            }
        };
        let start = actor::Start {
            identity,
            store: PeerStore::load(&self.directory),
            name: self.settings.effective_name(),
            port: self.settings.port,
            bind: self.setup.bind,
            options: self.setup.options.clone(),
            input_endpoint: self.setup.input_endpoint.clone(),
            displays: self.displays.clone(),
            notices: self.notices.clone(),
        };
        let (commands, commands_rx) = mpsc::unbounded_channel();
        let (started, started_rx) = oneshot::channel();
        tokio::spawn(async move {
            if let Some(previous) = previous {
                previous.stop().await;
            }
            actor::run(start, commands_rx, started).await;
        });
        self.running = Some(Running {
            commands,
            started: Some(started_rx),
        });
    }

    /// The state while no manager runs: the settings and the store, nothing connected.
    fn offline_state(&self) -> PeersState {
        let identity = std::fs::read(self.directory.join(IDENTITY_FILE))
            .ok()
            .and_then(|pem| Identity::from_pem(&pem).ok());
        PeersState {
            enabled: self.settings.enabled,
            port: self.settings.port,
            name: self.settings.effective_name(),
            machine_id: identity.map(|identity| identity.fingerprint().to_owned()),
            listen_address: None,
            error: self.failure.clone(),
            peers: PeerStore::load(&self.directory)
                .peers()
                .iter()
                .map(|paired| PeerEntry {
                    peer: Peer {
                        machine_id: paired.machine_id.clone(),
                        name: paired.name.clone(),
                        address: paired.address.clone(),
                        connected: false,
                        displays: Vec::new(),
                    },
                    role: paired.role,
                    state: PeerState::Paired,
                })
                .collect(),
        }
    }
}

const STOPPED: &str = "the multi-PC manager stopped";

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: &str, role: Role, state: PeerState, connected: bool) -> PeerEntry {
        PeerEntry {
            peer: Peer {
                machine_id: id.to_owned(),
                name: id.to_owned(),
                address: String::new(),
                connected,
                displays: vec![PeerDisplay {
                    id: "eDP-1".into(),
                    connector: "eDP-1".into(),
                    name: String::new(),
                    edid: None,
                    x: 0,
                    y: 0,
                    width: 1920,
                    height: 1080,
                    scale: 1.0,
                    effective_dpi: 96.0,
                    orientation: 0,
                    frequency: 60.0,
                    primary: true,
                    attached: true,
                    width_mm: 344.0,
                    height_mm: 194.0,
                    input_source: None,
                }],
            },
            role,
            state,
        }
    }

    fn state(enabled: bool, peers: Vec<PeerEntry>) -> PeersState {
        PeersState {
            enabled,
            port: 24817,
            name: "desk".into(),
            machine_id: None,
            listen_address: None,
            error: None,
            peers,
        }
    }

    fn ids(remotes: &[RemoteScreens]) -> Vec<&str> {
        remotes.iter().map(|r| r.machine_id.as_str()).collect()
    }

    #[test]
    fn only_the_linked_paired_remotes_of_this_machine_are_merged_in_machine_id_order() {
        let mut silent = entry("e", Role::Remote, PeerState::Paired, true);
        silent.peer.displays.clear();
        let peers = vec![
            entry("d", Role::Remote, PeerState::Paired, true),
            entry("a", Role::Master, PeerState::Paired, true),
            entry("b", Role::Remote, PeerState::Paired, false),
            entry("c", Role::Remote, PeerState::Pending, true),
            entry("c2", Role::Remote, PeerState::Paired, true),
            silent,
        ];
        assert_eq!(
            ids(&state(true, peers.clone()).remote_screens()),
            ["c2", "d"]
        );
        assert!(state(false, peers).remote_screens().is_empty());
    }
}
