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
//!   in the state, and relays the batches its hook writes to the remote's link — once the
//!   agent says the hook's zones lead to that remote alone ([`Service::hook_ready`]). **One
//!   remote at a time**: the selected remote (`SelectedRemote`) is the only one whose
//!   screens are merged and the only one the stream goes to — it stays selected while
//!   its link lives, and when it goes the linked remote with the lowest machine id
//!   takes its place. A paired remote whose address is known is called back while the
//!   mode is on, after one second, then twice as long at every failure, up to thirty.
//!
//! An unknown machine is shown as `Pending` and gets nothing but `Hello` until the user
//! accepts it (stored in `peers.json`, then `Paired` on the link) or refuses it.

mod actor;
pub mod hook_input;
pub mod local;
pub mod settings;
pub mod wallpapers;

use std::future::Future;
use std::net::{IpAddr, Ipv4Addr};
use std::path::PathBuf;
use std::pin::Pin;
use std::time::Duration;

use lbm_ipc::peer::{Peer, PeerDisplay, PeerWallpaper};
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
    /// The machine id of the one remote this machine drives now: the only one whose
    /// screens are in the layout and the one the input stream goes to. `None` when no
    /// remote is linked. Absent from the state of an agent older than the rule.
    #[serde(default)]
    pub selected_remote: Option<String>,
    /// Which choice of the selected remote this state reflects: a new number at every
    /// change of it (to another remote, another link of the same one, or none), never
    /// repeated in the life of the process. The agent tags what it tells the manager of
    /// the hook's zones with it ([`Service::hook_ready`]): a word about an earlier choice
    /// then says nothing about this one. Not in the API.
    #[serde(skip)]
    pub selection: u64,
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
    /// The screens the layout is to hold besides this machine's: those of the selected
    /// remote ([`PeersState::selected_remote`]), a paired machine linked as a remote of
    /// this one. One remote at a time: the input stream carries no destination, so a
    /// zone of any other remote would send its input to the selected one.
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
                    && self.selected_remote.as_deref() == Some(p.peer.machine_id.as_str())
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
    /// This machine's master asks for its monitor `display` (this machine's id of it, as
    /// sent in `Displays`) to be switched to the input `value` (docs/multi-pc.md, "Input
    /// switching"). Only a paired machine linked as this one's master is heard: the
    /// agent still checks the display is one of its own.
    SelectInput {
        from: String,
        display: String,
        value: u16,
    },
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
    /// What a remote of this machine's screens show (`PeerWallpapers`).
    Wallpapers(String),
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

/// Hands this machine's wallpapers to the manager (see [`Service::wallpaper_sink`]).
#[derive(Clone)]
pub struct WallpaperSink(mpsc::UnboundedSender<Command>);

impl WallpaperSink {
    pub fn send(&self, wallpapers: Vec<PeerWallpaper>) {
        let _ = self.0.send(Command::LocalWallpapers(wallpapers));
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

    /// Where this machine's wallpapers go once read, while the manager runs: a handle the
    /// agent hands to the task that reads and draws them, away from its loop. A handle
    /// outliving a restart of the manager sends into the void, which is harmless: the
    /// next link or display event reads them again.
    pub fn wallpaper_sink(&self) -> Option<WallpaperSink> {
        self.running
            .as_ref()
            .map(|running| WallpaperSink(running.commands.clone()))
    }

    /// Asks the remote `machine_id` to switch its monitor `display` to the input `value`
    /// (`SelectInput`), when it is linked as a remote of this machine. Sent and
    /// forgotten: the remote does not answer it, and what it shows next is the display
    /// change that follows. Whether it could be handed to the manager.
    pub fn select_input(&mut self, machine_id: &str, display: &str, value: u16) -> bool {
        let Some(running) = &self.running else {
            return false;
        };
        running
            .commands
            .send(Command::SelectInput {
                machine_id: machine_id.to_owned(),
                display: display.to_owned(),
                value,
            })
            .is_ok()
    }

    /// What the hook's zones are fit for: `Some(selection)` when every zone the hook holds,
    /// and every one it was sent and has not yet confirmed, leads to no machine but the
    /// remote selected in the state numbered `selection` ([`PeersState::selection`]);
    /// `None` when that is not known.
    ///
    /// The hook makes the other machines' zones enterable exactly while the manager holds
    /// a connection to its input endpoint, and a batch carries no destination: the
    /// manager connects only while this names its current selection, or a crossing into
    /// the zone of a remote no longer selected would send its `Enter`, clicks and keys to
    /// the one that is.
    pub fn hook_ready(&mut self, selection: Option<u64>) {
        if let Some(running) = &self.running {
            let _ = running.commands.send(Command::HookReady(selection));
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
                    PeerRequest::Wallpapers(machine_id) => {
                        Command::Wallpapers { machine_id, reply }
                    }
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
            selected_remote: None,
            selection: 0,
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
                        wallpaper_revision: 0,
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
                wallpaper_revision: 0,
            },
            role,
            state,
        }
    }

    fn state(enabled: bool, selected: &str, peers: Vec<PeerEntry>) -> PeersState {
        PeersState {
            enabled,
            port: 24817,
            name: "desk".into(),
            machine_id: None,
            listen_address: None,
            error: None,
            selected_remote: Some(selected.to_owned()),
            selection: 1,
            peers,
        }
    }

    fn ids(remotes: &[RemoteScreens]) -> Vec<&str> {
        remotes.iter().map(|r| r.machine_id.as_str()).collect()
    }

    #[test]
    fn only_the_selected_remote_is_merged_and_only_if_it_is_a_linked_paired_remote() {
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
            ids(&state(true, "d", peers.clone()).remote_screens()),
            ["d"]
        );
        assert_eq!(
            ids(&state(true, "c2", peers.clone()).remote_screens()),
            ["c2"]
        );
        for unfit in ["a", "b", "c", "e", "zz"] {
            assert!(
                state(true, unfit, peers.clone())
                    .remote_screens()
                    .is_empty(),
                "{unfit}"
            );
        }
        let mut none = state(true, "d", peers.clone());
        none.selected_remote = None;
        assert!(none.remote_screens().is_empty());
        assert!(state(false, "d", peers).remote_screens().is_empty());
    }
}
