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
//! Either way, when the user lets it on both sides, the clipboard is shared
//! ([`clipboard`]): copies are announced, their content crosses on demand.
//!
//! An unknown machine is shown as `Pending` and gets nothing but `Hello` until the user
//! accepts it (stored in `peers.json`, then `Paired` on the link) or refuses it.

mod actor;
pub mod clipboard;
pub mod clipboard_files;
pub mod hook_input;
pub mod local;
pub mod overlay;
pub mod settings;
pub mod wallpapers;

use std::future::Future;
use std::net::{IpAddr, Ipv4Addr};
use std::path::PathBuf;
use std::pin::Pin;
use std::time::Duration;

use lbm_ipc::peer::{InjectionBlock, Peer, PeerDisplay, PeerWallpaper};
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
    /// Why the last attempt to link with it failed, or why its last link ended badly;
    /// absent once a link with it is up again, and for a link ended on purpose.
    /// Additive: absent from an older agent's state.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub problem: Option<LinkProblem>,
    /// Why this remote does not show the rulers this machine's editor asked it to show
    /// (`PeerOverlay`); absent while it shows them, or when none were asked for.
    /// Additive: absent from an older agent's state.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overlay_error: Option<String>,
    /// A remote linked to this machine says Windows drops the input this machine sends
    /// it, and why (`Control::InjectionBlocked`). Kept
    /// until it says otherwise or the link ends. Additive: absent when none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub injection_blocked: Option<InjectionBlock>,
}

/// A remote where Windows drops the input this machine sends (the agent's state, for the
/// tray): its name here, the process whose window has the focus there, and why.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct BlockedRemote {
    pub name: String,
    #[serde(default)]
    pub process: String,
    #[serde(default)]
    pub cause: lbm_ipc::peer::BlockCause,
}

/// The linked remotes of `state` that say their injection is blocked.
pub fn blocked_remotes(state: &PeersState) -> Vec<BlockedRemote> {
    if !state.enabled {
        return Vec::new();
    }
    state
        .peers
        .iter()
        .filter(|p| p.peer.connected)
        .filter_map(|p| {
            p.injection_blocked.as_ref().map(|block| BlockedRemote {
                name: if p.peer.name.is_empty() {
                    p.peer.machine_id.clone()
                } else {
                    p.peer.name.clone()
                },
                process: block.process.clone(),
                cause: block.cause,
            })
        })
        .collect()
}

/// What went wrong between this machine and another, for the user to read: the kind,
/// for the frontend to put in its own words, and what the network or the other machine
/// said, verbatim.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LinkProblem {
    pub kind: ProblemKind,
    #[serde(default)]
    pub detail: String,
}

/// The kinds of [`LinkProblem`]. A frontend that meets a kind it does not know shows the
/// detail alone.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProblemKind {
    /// It could not be reached, or the connection broke: an address, a firewall, a
    /// machine asleep or off. The detail is the system's error.
    Network,
    /// It refused this machine; the detail is the reason it gave (the user refused the
    /// pairing, or it no longer trusts this machine).
    Refused,
    /// The pairing was not confirmed on the other machine in time: nobody accepted it
    /// there, or it let the connection go.
    NotConfirmed,
    /// Another machine answers at its address: its fingerprint is not the one paired.
    /// The detail is the fingerprint that answered.
    UnknownFingerprint,
    /// It speaks another version of the protocol, or something this one cannot read.
    Incompatible,
    /// The link was up and the other machine stopped answering.
    Lost,
}

impl LinkProblem {
    pub fn new(kind: ProblemKind, detail: impl Into<String>) -> LinkProblem {
        LinkProblem {
            kind,
            detail: detail.into(),
        }
    }

    /// Why a connection or a handshake failed. `pairing`: while waiting for the other
    /// machine to say `Paired`, where a connection let go of means nobody confirmed.
    pub fn of_link_error(error: &link::LinkError, pairing: bool) -> LinkProblem {
        use link::LinkError;
        match error {
            LinkError::Refused(reason) => LinkProblem::new(ProblemKind::Refused, reason.clone()),
            LinkError::Timeout("pairing") => {
                LinkProblem::new(ProblemKind::NotConfirmed, error.to_string())
            }
            LinkError::Closed(reason) if pairing => {
                LinkProblem::new(ProblemKind::NotConfirmed, reason.clone())
            }
            LinkError::Protocol(what) => LinkProblem::new(ProblemKind::Incompatible, what.clone()),
            LinkError::Io(_) | LinkError::Timeout(_) | LinkError::Closed(_) => {
                LinkProblem::new(ProblemKind::Network, error.to_string())
            }
        }
    }

    /// Why a live link ended, when it is a problem: a `Bye` or a close on this side is
    /// a link ended on purpose.
    pub fn of_close(reason: &link::CloseReason) -> Option<LinkProblem> {
        use link::CloseReason;
        match reason {
            CloseReason::Refused(why) => Some(LinkProblem::new(ProblemKind::Refused, why.clone())),
            CloseReason::Lost => Some(LinkProblem::new(ProblemKind::Lost, "it stopped answering")),
            CloseReason::Failed(why) => Some(LinkProblem::new(ProblemKind::Network, why.clone())),
            CloseReason::Bye(_) | CloseReason::Local(_) => None,
        }
    }
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
    /// The machine id of the remote that receives the input now — the pointer crossed
    /// to it and has not come back — or `None` when this machine does. Changes as the
    /// pointer crosses, by the stream's own `Enter` and `Leave`. Absent from the state
    /// of an older agent, which means "this machine".
    #[serde(default)]
    pub driven: Option<String>,
    /// The shortcut that locks the pointer on the machine it is on; empty when there
    /// is none. Absent from the state of an older agent.
    #[serde(default)]
    pub pointer_lock_shortcut: String,
    /// The pointer is locked on the machine it is on: [`Self::driven`], or this one.
    /// Said by the hook; absent from the state of an older agent.
    #[serde(default)]
    pub pointer_locked: bool,
    /// The shared clipboard: what the user lets it do, and whether it works. Absent
    /// from the state of an older agent.
    #[serde(default)]
    pub clipboard: clipboard::ClipboardState,
    /// The machine id of the master whose mouse and keyboard drive this machine now —
    /// its stream crossed onto this machine and has not left — or `None`. The other
    /// end of [`Self::driven`]: by the `Enter` and `Leave` of that stream as they reach
    /// the hook, the end of the stream (its link, or its session here) counting as a
    /// `Leave`. Absent from the state of an older agent, which means "nobody".
    #[serde(default)]
    pub driven_by: Option<String>,
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
    /// The machine ids of the paired machines linked now, either role; none while the
    /// mode is off.
    pub fn linked(&self) -> Vec<String> {
        self.peers
            .iter()
            .filter(|p| self.enabled && p.state == PeerState::Paired && p.peer.connected)
            .map(|p| p.peer.machine_id.clone())
            .collect()
    }

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
// The state is the notice sent most: boxing it would cost every one of them.
#[allow(clippy::large_enum_variant)]
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
        /// The master's number for the request, to give back with the answer.
        request: Option<u64>,
    },
    /// A remote of this machine says how the `SelectInput` this machine sent it went
    /// (`Control::InputSwitched`): `error` is why it could not, `None` when it did.
    /// `request` is the number this machine gave the request.
    InputSwitched {
        from: String,
        display: String,
        value: u16,
        request: Option<u64>,
        error: Option<String>,
    },
    /// A gesture on the interactive rulers this machine asked remote `from` to show
    /// (`Control::OverlayInput`): an `lbm_overlay::input::Input`, for the frontend that
    /// asked for them to act on.
    OverlayInput { from: String, input: Value },
}

/// A frontend's request, as far as the multi-PC mode goes.
#[derive(Clone, Debug, PartialEq)]
pub enum PeerRequest {
    State,
    Set {
        enabled: bool,
        port: Option<u16>,
        name: Option<String>,
        /// Replaces the pointer lock's shortcut when given; empty takes it away.
        pointer_lock_shortcut: Option<String>,
        /// Replaces what the shared clipboard may do when given.
        clipboard: ClipboardChange,
    },
    Connect(String),
    Accept(String),
    Refuse(String),
    Forget(String),
    /// What a remote of this machine's screens show (`PeerWallpapers`).
    Wallpapers(String),
}

/// What `SetMultiPc` changes of the shared clipboard; `None` leaves a field as it is.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ClipboardChange {
    pub send: Option<bool>,
    pub receive: Option<bool>,
    pub max_bytes: Option<u64>,
    pub files_max_bytes: Option<u64>,
    /// The reception folder; empty goes back to the default.
    pub folder: Option<String>,
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
    /// How the shared clipboard reaches this machine's: the system's, or a test's.
    pub clipboard: crate::clipboard::Factory,
}

impl Setup {
    pub fn new(input_endpoint: Option<String>) -> Setup {
        Setup {
            bind: IpAddr::V4(Ipv4Addr::UNSPECIFIED),
            input_endpoint,
            options: link::Options::default(),
            clipboard: crate::clipboard::Factory::system(),
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
    /// What this machine's hook last said it reads ([`Service::hook_reads_locks`]).
    hook_reads_locks: bool,
    /// What draws the rulers a master of this machine asks for.
    painter: overlay::Painter,
    /// Where this machine's Windows service takes the input stream
    /// ([`Service::set_secure_input`]).
    secure_input: Option<lbm_net::wire::SecureInput>,
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
            hook_reads_locks: false,
            painter: overlay::Painter::default(),
            secure_input: None,
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
    /// (`SelectInput`), when it is linked as a remote of this machine. The remote says
    /// how it went with `InputSwitched` ([`Notice::InputSwitched`]); an older one says
    /// nothing, and what it shows next is the display change that follows. Whether it
    /// could be handed to the manager.
    pub fn select_input(
        &mut self,
        machine_id: &str,
        display: &str,
        value: u16,
        request: Option<u64>,
    ) -> bool {
        let Some(running) = &self.running else {
            return false;
        };
        running
            .commands
            .send(Command::SelectInput {
                machine_id: machine_id.to_owned(),
                display: display.to_owned(),
                value,
                request,
            })
            .is_ok()
    }

    /// Tells this machine's master `machine_id` how its `SelectInput` for `display` and
    /// `value`, numbered `request`, went (`Control::InputSwitched`): `error` is why it
    /// failed. Whether it could be handed to the manager.
    pub fn input_switched(
        &mut self,
        machine_id: &str,
        display: &str,
        value: u16,
        request: Option<u64>,
        error: Option<String>,
    ) -> bool {
        let Some(running) = &self.running else {
            return false;
        };
        running
            .commands
            .send(Command::InputSwitched {
                machine_id: machine_id.to_owned(),
                display: display.to_owned(),
                value,
                request,
                error,
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
    /// Whether this machine's hook reads the lock keys' state in the input stream: until
    /// it says it does, the manager strips it from what a master sends to be injected
    /// here — an older hook would refuse the whole batch, the crossing with it.
    /// Where this machine's Windows service takes the input stream itself
    /// (docs/multi-pc-service-windows.md), told to masters in `Hello`; `None` where there
    /// is no service. The masters learn a change at their next link: a running manager is
    /// restarted, which drops the links and makes them anew.
    pub fn set_secure_input(&mut self, offer: Option<lbm_net::wire::SecureInput>) {
        if offer == self.secure_input {
            return;
        }
        self.secure_input = offer;
        if let Some(running) = self.running.take() {
            self.start(Some(running));
        }
    }

    pub fn hook_reads_locks(&mut self, reads: bool) {
        self.hook_reads_locks = reads;
        if let Some(running) = &self.running {
            let _ = running.commands.send(Command::HookReadsLocks(reads));
        }
    }

    /// This machine's hook says Windows drops what it injects for a master, or no longer
    /// does: every master linked that hears of it is told.
    pub fn injection_blocked(&mut self, block: Option<InjectionBlock>) {
        if let Some(running) = &self.running {
            let _ = running.commands.send(Command::InjectionBlocked(block));
        }
    }

    pub fn hook_locked(&mut self, locked: bool) {
        if let Some(running) = &self.running {
            let _ = running.commands.send(Command::PointerLocked(locked));
        }
    }

    pub fn hook_ready(&mut self, selection: Option<u64>) {
        if let Some(running) = &self.running {
            let _ = running.commands.send(Command::HookReady(selection));
        }
    }

    /// What draws the rulers a master of this machine asks for, from the next start of
    /// the manager on: for tests, which must not draw on the screens of the machine
    /// running them.
    pub fn set_overlay_painter(&mut self, painter: overlay::Painter) {
        self.painter = painter;
    }

    /// The rulers this machine's editor wants on the screens of its remote `machine_id`
    /// (`PeerOverlay`): `lbm_overlay::scene::Band`s in that remote's desktop pixels;
    /// none takes them down. `interactive`: they are dragged there, and what is done on
    /// them comes back as [`Notice::OverlayInput`]. Whether the manager runs to be told.
    pub fn overlay(&mut self, machine_id: &str, bands: Vec<Value>, interactive: bool) -> bool {
        let Some(running) = &self.running else {
            return false;
        };
        running
            .commands
            .send(Command::Overlay {
                machine_id: machine_id.to_owned(),
                bands,
                interactive,
            })
            .is_ok()
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
                pointer_lock_shortcut,
                clipboard,
            } => self.set(enabled, port, name, pointer_lock_shortcut, clipboard),
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
    fn set(
        &mut self,
        enabled: bool,
        port: Option<u16>,
        name: Option<String>,
        pointer_lock_shortcut: Option<String>,
        clipboard: ClipboardChange,
    ) -> Answer {
        let mut next = self.settings.clone();
        if let Some(send) = clipboard.send {
            next.clipboard_send = send;
        }
        if let Some(receive) = clipboard.receive {
            next.clipboard_receive = receive;
        }
        if let Some(max_bytes) = clipboard.max_bytes {
            next.clipboard_max_bytes = Some(max_bytes);
        }
        if let Some(max_bytes) = clipboard.files_max_bytes {
            next.clipboard_files_max_bytes = Some(max_bytes);
        }
        if let Some(folder) = clipboard.folder {
            let folder = folder.trim();
            next.clipboard_folder = (!folder.is_empty()).then(|| PathBuf::from(folder));
        }
        if let Some(shortcut) = pointer_lock_shortcut {
            let shortcut = shortcut.trim().to_owned();
            next.pointer_lock_shortcut = (!shortcut.is_empty()).then_some(shortcut);
        }
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
        // The shortcut and the clipboard are no reason to restart the manager and drop
        // its links.
        let shortcut = next.pointer_lock_shortcut.clone();
        let same_links = Settings {
            pointer_lock_shortcut: self.settings.pointer_lock_shortcut.clone(),
            clipboard_send: self.settings.clipboard_send,
            clipboard_receive: self.settings.clipboard_receive,
            clipboard_max_bytes: self.settings.clipboard_max_bytes,
            clipboard_files_max_bytes: self.settings.clipboard_files_max_bytes,
            clipboard_folder: self.settings.clipboard_folder.clone(),
            ..next.clone()
        } == self.settings;
        let unchanged = same_links && (self.running.is_some() || !enabled);
        self.settings = next;
        if let Some(running) = self.running.as_ref().filter(|_| unchanged) {
            let _ = running
                .commands
                .send(Command::PointerLockShortcut(shortcut.unwrap_or_default()));
            let _ = running
                .commands
                .send(Command::Clipboard(self.settings.clipboard()));
        }
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
            hook_reads_locks: self.hook_reads_locks,
            pointer_lock_shortcut: self
                .settings
                .pointer_lock_shortcut
                .clone()
                .unwrap_or_default(),
            displays: self.displays.clone(),
            notices: self.notices.clone(),
            painter: self.painter.clone(),
            clipboard: self.settings.clipboard(),
            clipboard_factory: self.setup.clipboard.clone(),
            secure_input: self.secure_input.clone(),
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
            driven: None,
            driven_by: None,
            pointer_lock_shortcut: self
                .settings
                .pointer_lock_shortcut
                .clone()
                .unwrap_or_default(),
            pointer_locked: false,
            clipboard: {
                let options = self.settings.clipboard();
                clipboard::ClipboardState {
                    send: options.send,
                    receive: options.receive,
                    max_bytes: options.max_bytes,
                    files_max_bytes: options.files_max_bytes,
                    reception: options.reception.to_string_lossy().into_owned(),
                    fetching: None,
                    active: false,
                    problem: None,
                }
            },
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
                    problem: None,
                    overlay_error: None,
                    injection_blocked: None,
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
            problem: None,
            overlay_error: None,
            injection_blocked: None,
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
            driven: None,
            driven_by: None,
            pointer_lock_shortcut: String::new(),
            pointer_locked: false,
            clipboard: Default::default(),
            peers,
        }
    }

    #[test]
    fn the_linked_machines_are_the_paired_connected_ones_of_either_role() {
        let peers = vec![
            entry("a", Role::Master, PeerState::Paired, true),
            entry("b", Role::Remote, PeerState::Paired, false),
            entry("c", Role::Remote, PeerState::Pending, true),
            entry("d", Role::Remote, PeerState::Paired, true),
        ];
        assert_eq!(state(true, "d", peers.clone()).linked(), ["a", "d"]);
        assert!(state(false, "d", peers).linked().is_empty());
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

    #[test]
    fn a_link_error_is_told_by_what_the_user_can_do_about_it() {
        use link::{CloseReason, LinkError};
        let kind = |error: LinkError, pairing| LinkProblem::of_link_error(&error, pairing).kind;
        let refused =
            LinkProblem::of_link_error(&LinkError::Refused("the user refused".into()), true);
        assert_eq!(
            refused,
            LinkProblem::new(ProblemKind::Refused, "the user refused")
        );
        assert_eq!(
            kind(LinkError::Timeout("pairing"), true),
            ProblemKind::NotConfirmed
        );
        assert_eq!(
            kind(LinkError::Closed("connection closed".into()), true),
            ProblemKind::NotConfirmed
        );
        assert_eq!(
            kind(LinkError::Closed("connection closed".into()), false),
            ProblemKind::Network
        );
        assert_eq!(
            kind(LinkError::Timeout("the handshake"), false),
            ProblemKind::Network
        );
        let refused_tcp = std::io::Error::from(std::io::ErrorKind::ConnectionRefused);
        assert_eq!(
            kind(LinkError::Io(refused_tcp), false),
            ProblemKind::Network
        );
        assert_eq!(
            kind(LinkError::Protocol("protocol 2".into()), false),
            ProblemKind::Incompatible
        );

        assert_eq!(
            LinkProblem::of_close(&CloseReason::Lost).map(|p| p.kind),
            Some(ProblemKind::Lost)
        );
        assert_eq!(
            LinkProblem::of_close(&CloseReason::Refused("forgotten".into())),
            Some(LinkProblem::new(ProblemKind::Refused, "forgotten"))
        );
        assert_eq!(
            LinkProblem::of_close(&CloseReason::Failed("reset".into())).map(|p| p.kind),
            Some(ProblemKind::Network)
        );
        // Ended on purpose: nothing to say.
        assert_eq!(
            LinkProblem::of_close(&CloseReason::Bye("quit".into())),
            None
        );
        assert_eq!(
            LinkProblem::of_close(&CloseReason::Local("off".into())),
            None
        );
    }

    #[test]
    fn a_problem_is_absent_from_the_state_unless_there_is_one() {
        let mut entry = entry("d", Role::Remote, PeerState::Paired, false);
        let json = serde_json::to_value(&entry).unwrap();
        assert!(json.get("Problem").is_none(), "{json}");
        entry.problem = Some(LinkProblem::new(ProblemKind::UnknownFingerprint, "ab12"));
        let json = serde_json::to_value(&entry).unwrap();
        assert_eq!(
            json["Problem"],
            serde_json::json!({"Kind": "UnknownFingerprint", "Detail": "ab12"})
        );
        // An older agent's entry reads.
        json.as_object().unwrap();
        let mut older = json.clone();
        older.as_object_mut().unwrap().remove("Problem");
        let read: PeerEntry = serde_json::from_value(older).unwrap();
        assert_eq!(read.problem, None);
    }
}
