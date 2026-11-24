//! The peer manager's task while the mode is on: the listener, the pairings waiting for
//! the user, the live links, the reconnections. It owns all of them and nothing else
//! touches them; the agent's loop talks to it through [`Command`]s and hears from it
//! through [`Notice`]s, so that nothing the network does can hold that loop up.

use std::collections::{HashMap, HashSet};
use std::net::{IpAddr, SocketAddr};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use lbm_ipc::input::{InputEvent, LOCKS_CAPABILITY};
use lbm_ipc::peer::{Peer, PeerDisplay, PeerWallpaper};
use lbm_net::identity::Identity;
use lbm_net::link::{
    self, CloseReason, Event, Link, LinkError, LinkSender, Listener, PeerInfo, Pending,
};
use lbm_net::peers::{PairedPeer, PeerStore};
use lbm_net::wire::{Control, Hello, Role};
use serde_json::Value;
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinSet;

use super::hook_input::{HookInput, Sink, Target};
use super::{LinkProblem, Notice, PeerEntry, PeerState, PeersState, ProblemKind};

/// The first wait before calling a remote back, doubled at every failure up to
/// [`REDIAL_MAX`].
const REDIAL_FIRST: Duration = Duration::from_secs(1);
const REDIAL_MAX: Duration = Duration::from_secs(30);

/// How long the links are given to say `Bye` when the mode is turned off.
const SHUTDOWN_PATIENCE: Duration = Duration::from_secs(1);

/// The last number given to a choice of the selected remote ([`PeersState::selection`]).
/// One counter for the process, not per manager: a manager turned off and on again must
/// not take a word the agent said about the previous one's choice for its own.
static SELECTIONS: AtomicU64 = AtomicU64::new(0);

pub(super) type Reply = oneshot::Sender<Result<Value, String>>;

/// What the agent asks of the manager.
pub(super) enum Command {
    State(oneshot::Sender<PeersState>),
    Connect {
        address: String,
        reply: Reply,
    },
    Accept {
        machine_id: String,
        reply: Reply,
    },
    Refuse {
        machine_id: String,
        reply: Reply,
    },
    Forget {
        machine_id: String,
        reply: Reply,
    },
    /// This machine's screens changed (or are known for the first time).
    LocalDisplays(Vec<PeerDisplay>),
    /// What this machine's screens show, drawn small, as last read.
    LocalWallpapers(Vec<PeerWallpaper>),
    /// The wallpapers a remote of this machine last sent (`PeerWallpapers`).
    Wallpapers {
        machine_id: String,
        reply: Reply,
    },
    /// What the hook's zones are fit for (`Service::hook_ready`).
    HookReady(Option<u64>),
    /// The pointer lock's shortcut changed; empty when there is none.
    PointerLockShortcut(String),
    /// The hook locked the pointer, or let it go (`Service::hook_locked`).
    PointerLocked(bool),
    /// Whether this machine's hook reads the lock keys' state (`Service::hook_reads_locks`).
    HookReadsLocks(bool),
    /// `SelectInput` to a remote of this machine.
    SelectInput {
        machine_id: String,
        display: String,
        value: u16,
        request: Option<u64>,
    },
    /// `InputSwitched` to this machine's master `machine_id`.
    InputSwitched {
        machine_id: String,
        display: String,
        value: u16,
        request: Option<u64>,
        error: Option<String>,
    },
    /// Close everything and end; answered once done.
    Shutdown(oneshot::Sender<()>),
}

/// What the manager's own tasks report back to it.
enum Internal {
    Dialed {
        address: String,
        /// The paired machine this was a reconnection to, if it was one.
        expected: Option<String>,
        // Boxed: a connection mid-handshake is the one large message on this channel.
        result: Result<Box<Pending>, LinkError>,
        reply: Option<Reply>,
    },
    Accepted {
        machine_id: String,
        result: Result<Link, LinkError>,
    },
    Control {
        id: u64,
        machine_id: String,
        message: Control,
    },
    Closed {
        id: u64,
        machine_id: String,
        reason: CloseReason,
    },
    /// A master's stream crossed onto this machine (`driving`) or left it: its `Enter`
    /// and `Leave` as they reach the hook, a session ended here, the end of the link.
    Driving {
        id: u64,
        machine_id: String,
        driving: bool,
    },
    Expired {
        machine_id: String,
        serial: u64,
    },
    Redial(String),
}

/// What the manager starts with.
pub(super) struct Start {
    pub identity: Identity,
    pub store: PeerStore,
    pub name: String,
    pub port: u16,
    pub bind: IpAddr,
    pub options: link::Options,
    pub input_endpoint: Option<String>,
    pub hook_reads_locks: bool,
    pub pointer_lock_shortcut: String,
    pub displays: Vec<PeerDisplay>,
    pub notices: mpsc::UnboundedSender<Notice>,
}

/// An unknown machine, waiting for the user to say whether it is trusted.
struct Waiting {
    pending: Pending,
    /// The address the user typed to reach it; empty when it connected to us.
    address: String,
    serial: u64,
}

/// A live link.
struct Live {
    id: u64,
    sender: LinkSender,
    info: PeerInfo,
    /// The screens it last reported, with their revision (a remote's only).
    displays: Option<(u64, Vec<PeerDisplay>)>,
    /// The wallpapers it last sent (a remote's only): their revision on that link, the
    /// stamp the frontends know them by, and the pictures.
    wallpapers: Option<(u64, u64, Vec<PeerWallpaper>)>,
}

struct Actor {
    identity: Identity,
    store: PeerStore,
    name: String,
    port: u16,
    options: link::Options,
    listen_address: Option<SocketAddr>,
    listen_error: Option<String>,
    pending: HashMap<String, Waiting>,
    links: HashMap<String, Live>,
    /// Machines whose `accept` is under way.
    accepting: HashSet<String>,
    /// Paired remotes being dialled back, and those waiting to be.
    dialing: HashSet<String>,
    redial_scheduled: HashSet<String>,
    backoff: HashMap<String, Duration>,
    /// Why the last attempt to link with a paired machine failed, or its last link ended
    /// badly, by machine id: shown in the state until a link with it is up again.
    problems: HashMap<String, LinkProblem>,
    /// This machine's screens, and how many times they changed.
    displays: Vec<PeerDisplay>,
    revision: u64,
    /// This machine's wallpapers, and how many times they changed.
    wallpapers: Vec<PeerWallpaper>,
    wallpaper_revision: u64,
    /// The last stamp given to wallpapers received from a remote. One counter for every
    /// link, never reset while the mode is on: a remote's own revision starts over when
    /// it restarts, and a frontend comparing it would keep pictures that changed.
    wallpaper_stamp: u64,
    hook: HookInput,
    /// The remote the hook's stream goes to and whose screens are in the layout (this
    /// machine is its master), with the id of its link: one at a time (see
    /// [`Actor::select`]).
    selected: Option<(String, u64)>,
    /// The number of the current choice of `selected` ([`PeersState::selection`]).
    selection: u64,
    /// The choice the hook's zones were last said to be fit for ([`Command::HookReady`]).
    hook_ready: Option<u64>,
    /// The remote the hook's stream is relayed to now: the selected one, once the hook's
    /// zones lead nowhere else (see [`Actor::retarget`]).
    relaying: Option<(String, u64)>,
    /// What the state says of the pointer lock ([`PeersState::pointer_lock_shortcut`],
    /// [`PeersState::pointer_locked`]).
    pointer_lock_shortcut: String,
    pointer_locked: bool,
    /// The master whose stream drives this machine now, with the id of its link
    /// ([`PeersState::driven_by`]).
    driven_by: Option<(String, u64)>,
    internal: mpsc::UnboundedSender<Internal>,
    notices: mpsc::UnboundedSender<Notice>,
    published: Option<PeersState>,
    next_id: u64,
    next_serial: u64,
    /// Connections being made or accepted, and timers (see [`Actor::spawn`]).
    tasks: JoinSet<()>,
}

/// What this node can do beyond the protocol's baseline: its hook reads the master's
/// lock keys (the hook ships with this agent, which retires one of another protocol).
fn with_capabilities(mut hello: Hello) -> Hello {
    hello.capabilities.push(LOCKS_CAPABILITY.to_owned());
    hello
}

/// Runs the manager until it is told to stop or the agent drops its end. `started` is
/// answered once the listener is up (or could not be), so that whoever turned the mode
/// on can report the outcome.
pub(super) async fn run(
    start: Start,
    mut commands: mpsc::UnboundedReceiver<Command>,
    started: oneshot::Sender<()>,
) {
    let (internal, mut internal_rx) = mpsc::unbounded_channel();
    let hello = with_capabilities(Hello::new(
        start.name.clone(),
        env!("CARGO_PKG_VERSION"),
        Role::Remote,
    ));
    // Every node with the mode on listens; whoever connects is a master, so this side
    // of those links says it is the remote.
    let mut listener = match link::listen(
        SocketAddr::new(start.bind, start.port),
        &start.identity,
        hello,
        start.options.clone(),
    )
    .await
    {
        Ok(listener) => Some(listener),
        Err(error) => {
            eprintln!(
                "[lbm-agent] multi-PC: cannot listen on port {}: {error}",
                start.port
            );
            None
        }
    };
    let mut actor = Actor {
        listen_address: listener.as_ref().map(Listener::local_addr),
        listen_error: match &listener {
            Some(_) => None,
            None => Some(format!("cannot listen on port {}", start.port)),
        },
        identity: start.identity,
        store: start.store,
        name: start.name,
        port: start.port,
        options: start.options,
        pending: HashMap::new(),
        links: HashMap::new(),
        accepting: HashSet::new(),
        dialing: HashSet::new(),
        redial_scheduled: HashSet::new(),
        backoff: HashMap::new(),
        problems: HashMap::new(),
        // A list handed over at start is this machine's screens already: revision 1, so
        // the first change after it is told apart.
        revision: u64::from(!start.displays.is_empty()),
        displays: start.displays,
        wallpapers: Vec::new(),
        wallpaper_revision: 0,
        wallpaper_stamp: 0,
        hook: {
            let hook = HookInput::spawn(start.input_endpoint);
            hook.set_reads_locks(start.hook_reads_locks);
            hook
        },
        selected: None,
        selection: SELECTIONS.fetch_add(1, Ordering::Relaxed) + 1,
        hook_ready: None,
        relaying: None,
        pointer_lock_shortcut: start.pointer_lock_shortcut,
        pointer_locked: false,
        driven_by: None,
        internal,
        notices: start.notices,
        published: None,
        next_id: 0,
        next_serial: 0,
        tasks: JoinSet::new(),
    };
    if let Some(address) = actor.listen_address {
        eprintln!(
            "[lbm-agent] multi-PC: listening on {address} as {} ({})",
            actor.name,
            actor.identity.fingerprint()
        );
    }
    let _ = started.send(());
    actor.call_remotes_back();
    // The pointer going to the remote and coming back: only the state changes.
    let mut driving = actor.hook.driving();

    loop {
        actor.publish();
        tokio::select! {
            command = commands.recv() => match command {
                Some(Command::Shutdown(done)) => {
                    drop(listener.take());
                    actor.shutdown().await;
                    let _ = done.send(());
                    return;
                }
                Some(command) => actor.command(command),
                None => {
                    drop(listener.take());
                    actor.shutdown().await;
                    return;
                }
            },
            incoming = async {
                match &mut listener {
                    Some(listener) => listener.next().await,
                    None => std::future::pending().await,
                }
            } => match incoming {
                Some(Ok(pending)) => actor.admit(pending, String::new()),
                // A stranger who could not finish the handshake: its failure, not ours.
                Some(Err(error)) => eprintln!("[lbm-agent] multi-PC: an incoming connection failed: {error}"),
                None => listener = None,
            },
            Some(internal) = internal_rx.recv() => actor.internal(internal),
            Ok(()) = driving.changed() => {}
        }
    }
}

impl Actor {
    fn hello(&self) -> Hello {
        with_capabilities(Hello::new(
            self.name.clone(),
            env!("CARGO_PKG_VERSION"),
            Role::Master,
        ))
    }

    /// Tells the agent when what it would show changed.
    fn publish(&mut self) {
        let state = self.state();
        if self.published.as_ref() == Some(&state) {
            return;
        }
        let _ = self.notices.send(Notice::Peers(state.clone()));
        self.published = Some(state);
    }

    fn state(&self) -> PeersState {
        let mut peers: Vec<PeerEntry> = self
            .store
            .peers()
            .iter()
            .map(|paired| {
                let live = self.links.get(&paired.machine_id);
                PeerEntry {
                    peer: Peer {
                        machine_id: paired.machine_id.clone(),
                        name: paired.name.clone(),
                        address: paired.address.clone(),
                        connected: live.is_some(),
                        displays: live
                            .and_then(|live| live.displays.as_ref())
                            .map(|(_, displays)| displays.clone())
                            .unwrap_or_default(),
                        wallpaper_revision: live
                            .and_then(|live| live.wallpapers.as_ref())
                            .map_or(0, |(_, stamp, _)| *stamp),
                    },
                    role: paired.role,
                    state: PeerState::Paired,
                    problem: self.problems.get(&paired.machine_id).cloned(),
                }
            })
            .collect();
        let mut waiting: Vec<_> = self.pending.iter().collect();
        waiting.sort_by_key(|(_, waiting)| waiting.serial);
        peers.extend(waiting.into_iter().map(|(machine_id, waiting)| {
            let info = waiting.pending.peer();
            PeerEntry {
                peer: Peer {
                    machine_id: machine_id.clone(),
                    name: info.hello.name.clone(),
                    address: if waiting.address.is_empty() {
                        info.address.to_string()
                    } else {
                        waiting.address.clone()
                    },
                    connected: false,
                    displays: Vec::new(),
                    wallpaper_revision: 0,
                },
                role: info.hello.role,
                state: PeerState::Pending,
                problem: None,
            }
        }));
        PeersState {
            enabled: true,
            port: self.port,
            name: self.name.clone(),
            machine_id: Some(self.identity.fingerprint().to_owned()),
            listen_address: self.listen_address.map(|a| a.to_string()),
            error: self.listen_error.clone(),
            selected_remote: self
                .selected
                .as_ref()
                .map(|(machine_id, _)| machine_id.clone()),
            selection: self.selection,
            driven: self
                .relaying
                .as_ref()
                .filter(|_| self.hook.driven())
                .map(|(machine_id, _)| machine_id.clone()),
            pointer_lock_shortcut: self.pointer_lock_shortcut.clone(),
            pointer_locked: self.pointer_locked,
            driven_by: self
                .driven_by
                .as_ref()
                .map(|(machine_id, _)| machine_id.clone()),
            peers,
        }
    }

    fn command(&mut self, command: Command) {
        match command {
            Command::State(reply) => {
                let _ = reply.send(self.state());
            }
            Command::Connect { address, reply } => {
                let address = with_default_port(&address);
                self.dial(address, None, Some(reply));
            }
            Command::Accept { machine_id, reply } => {
                let _ = reply.send(self.accept_waiting(&machine_id));
            }
            Command::Refuse { machine_id, reply } => {
                let _ = reply.send(match self.pending.remove(&machine_id) {
                    Some(waiting) => {
                        eprintln!("[lbm-agent] multi-PC: refused {machine_id}");
                        tokio::spawn(waiting.pending.refuse("the user refused the pairing"));
                        Ok(Value::Null)
                    }
                    None => Err(format!("no machine {machine_id} is waiting to be paired")),
                });
            }
            Command::Forget { machine_id, reply } => {
                let _ = reply.send(self.forget(&machine_id));
            }
            Command::LocalDisplays(displays) => self.local_displays(displays),
            Command::LocalWallpapers(wallpapers) => self.local_wallpapers(wallpapers),
            Command::Wallpapers { machine_id, reply } => {
                let _ = reply.send(self.remote_wallpapers(&machine_id));
            }
            Command::PointerLockShortcut(shortcut) => self.pointer_lock_shortcut = shortcut,
            Command::PointerLocked(locked) => self.pointer_locked = locked,
            Command::HookReadsLocks(reads) => self.hook.set_reads_locks(reads),
            Command::HookReady(selection) => {
                self.hook_ready = selection;
                self.retarget();
            }
            Command::SelectInput {
                machine_id,
                display,
                value,
                request,
            } => self.select_input(&machine_id, display, value, request),
            Command::InputSwitched {
                machine_id,
                display,
                value,
                request,
                error,
            } => self.input_switched(&machine_id, display, value, request, error),
            // Handled by the loop, which owns the listener.
            Command::Shutdown(done) => {
                let _ = done.send(());
            }
        }
    }

    fn internal(&mut self, internal: Internal) {
        match internal {
            Internal::Dialed {
                address,
                expected,
                result,
                reply,
            } => self.dialed(address, expected, result, reply),
            Internal::Accepted { machine_id, result } => self.accepted(machine_id, result),
            Internal::Control {
                id,
                machine_id,
                message,
            } => self.control(id, &machine_id, message),
            Internal::Closed {
                id,
                machine_id,
                reason,
            } => self.closed(id, &machine_id, reason),
            Internal::Driving {
                id,
                machine_id,
                driving,
            } => self.driving(id, machine_id, driving),
            Internal::Expired { machine_id, serial } => {
                if self
                    .pending
                    .get(&machine_id)
                    .is_some_and(|w| w.serial == serial)
                {
                    eprintln!("[lbm-agent] multi-PC: nobody answered the pairing of {machine_id}");
                    self.pending.remove(&machine_id);
                }
            }
            Internal::Redial(machine_id) => {
                self.redial_scheduled.remove(&machine_id);
                self.call_back(&machine_id);
            }
        }
    }

    //==================//
    // Making links     //
    //==================//

    /// Connects to `address` as its master, in a task of its own.
    fn dial(&mut self, address: String, expected: Option<String>, reply: Option<Reply>) {
        if let Some(machine_id) = &expected {
            self.dialing.insert(machine_id.clone());
        }
        let identity = self.identity.clone();
        let hello = self.hello();
        let options = self.options.clone();
        let internal = self.internal.clone();
        self.spawn(async move {
            let result = link::connect(address.as_str(), &identity, hello, options)
                .await
                .map(Box::new);
            let _ = internal.send(Internal::Dialed {
                address,
                expected,
                result,
                reply,
            });
        });
    }

    fn dialed(
        &mut self,
        address: String,
        expected: Option<String>,
        result: Result<Box<Pending>, LinkError>,
        reply: Option<Reply>,
    ) {
        if let Some(machine_id) = &expected {
            self.dialing.remove(machine_id);
        }
        let answer = |reply: Option<Reply>, result: Result<Value, String>| {
            if let Some(reply) = reply {
                let _ = reply.send(result);
            }
        };
        let pending = match result {
            Ok(pending) => *pending,
            Err(error) => {
                // A paired remote is dialled again and again while it is away: say so
                // the first time only. Saying nothing at all hid a stored address with
                // no port behind a peer that simply never linked.
                let first_failure = expected
                    .as_ref()
                    .is_none_or(|machine_id| !self.backoff.contains_key(machine_id));
                if first_failure {
                    eprintln!("[lbm-agent] multi-PC: cannot connect to {address}: {error}");
                }
                answer(reply, Err(format!("cannot connect to {address}: {error}")));
                if let Some(machine_id) = expected {
                    self.problem(&machine_id, LinkProblem::of_link_error(&error, false));
                    self.schedule_redial(&machine_id);
                }
                return;
            }
        };
        let fingerprint = pending.peer().fingerprint.clone();
        if let Some(machine_id) = expected {
            if fingerprint != machine_id {
                // Someone else answers at that address now: not the machine the user
                // paired, and not one to pair behind their back.
                eprintln!(
                    "[lbm-agent] multi-PC: {address} is no longer {machine_id} but {fingerprint}"
                );
                tokio::spawn(pending.refuse("not the machine paired at this address"));
                self.problem(
                    &machine_id,
                    LinkProblem::new(ProblemKind::UnknownFingerprint, fingerprint),
                );
                self.schedule_redial(&machine_id);
                return;
            }
        }
        if pending.peer().hello.role != Role::Remote {
            answer(
                reply,
                Err(format!(
                    "{address} answered as a master: the machine connected to must be a remote"
                )),
            );
            tokio::spawn(pending.refuse("both ends of this link would be masters"));
            return;
        }
        let entry = self.entry_for(&pending, &address);
        self.admit(pending, address);
        answer(
            reply,
            serde_json::to_value(entry).map_err(|error| error.to_string()),
        );
    }

    /// How a machine being admitted will show, for the answer to `PeerConnect`.
    fn entry_for(&self, pending: &Pending, address: &str) -> PeerEntry {
        let info = pending.peer();
        PeerEntry {
            peer: Peer {
                machine_id: info.fingerprint.clone(),
                name: info.hello.name.clone(),
                address: address.to_owned(),
                connected: false,
                displays: Vec::new(),
                wallpaper_revision: 0,
            },
            role: info.hello.role,
            state: if self.store.get(&info.fingerprint).is_some() {
                PeerState::Paired
            } else {
                PeerState::Pending
            },
            problem: None,
        }
    }

    /// A machine that said who it is: trusted if the store knows it, otherwise kept
    /// until the user decides. `address` is the one typed to reach it, empty for a
    /// machine that connected to this one.
    fn admit(&mut self, pending: Pending, address: String) {
        let info = pending.peer().clone();
        let fingerprint = info.fingerprint.clone();
        if fingerprint == self.identity.fingerprint() {
            tokio::spawn(pending.refuse("a machine cannot pair with itself"));
            return;
        }
        // A link is a master and a remote. The listener says Remote, a dialler says
        // Master: two that say the same have both misread their part.
        if address.is_empty() && info.hello.role != Role::Master {
            tokio::spawn(pending.refuse("only a master connects to another machine"));
            return;
        }
        if let Some(paired) = self.store.get(&fingerprint).cloned() {
            // What it says of itself now wins over what was stored, and an address the
            // user just typed over the one they typed before.
            let updated = PairedPeer {
                name: info.hello.name.clone(),
                role: info.hello.role,
                address: if address.is_empty() {
                    paired.address.clone()
                } else {
                    address
                },
                ..paired.clone()
            };
            if updated != paired {
                if let Err(error) = self.store.insert(updated) {
                    eprintln!(
                        "[lbm-agent] multi-PC: cannot update {}: {error}",
                        paired.machine_id
                    );
                }
            }
            self.accept(pending);
            return;
        }
        eprintln!(
            "[lbm-agent] multi-PC: {} ({fingerprint}, {}) waits to be paired",
            info.hello.name, info.address
        );
        self.next_serial += 1;
        let serial = self.next_serial;
        // The other side gives up after its pairing timeout; so does this one, and the
        // connection held meanwhile is let go.
        let internal = self.internal.clone();
        let timeout = self.options.pairing_timeout;
        let machine_id = fingerprint.clone();
        self.spawn(async move {
            tokio::time::sleep(timeout).await;
            let _ = internal.send(Internal::Expired { machine_id, serial });
        });
        // A newer connection from the same machine replaces the older one, dropped here.
        self.pending.insert(
            fingerprint,
            Waiting {
                pending,
                address,
                serial,
            },
        );
    }

    /// The user trusts a machine that is waiting: stored, then accepted.
    fn accept_waiting(&mut self, machine_id: &str) -> Result<Value, String> {
        let waiting = self
            .pending
            .remove(machine_id)
            .ok_or_else(|| format!("no machine {machine_id} is waiting to be paired"))?;
        let info = waiting.pending.peer();
        let paired = PairedPeer {
            machine_id: machine_id.to_owned(),
            name: info.hello.name.clone(),
            address: waiting.address.clone(),
            role: info.hello.role,
        };
        // Stored before the link is up: the other side may take its time, and the user
        // has said yes on this one.
        self.store
            .insert(paired)
            .map_err(|error| format!("cannot store the pairing: {error}"))?;
        eprintln!("[lbm-agent] multi-PC: the user paired {machine_id}");
        self.accept(waiting.pending);
        Ok(Value::Null)
    }

    /// Says `Paired` and waits for the other side to say it, in a task of its own: on a
    /// first pairing, that is a user reading a fingerprint on the other machine.
    fn accept(&mut self, pending: Pending) {
        let machine_id = pending.peer().fingerprint.clone();
        self.accepting.insert(machine_id.clone());
        let internal = self.internal.clone();
        self.spawn(async move {
            let result = pending.accept().await;
            let _ = internal.send(Internal::Accepted { machine_id, result });
        });
    }

    /// A task that only matters while the manager runs — a connection being made or
    /// accepted, a timer: aborted with it, so that a mode turned off leaves no task
    /// waiting on the network or the clock. The finished ones are reaped here.
    fn spawn(&mut self, task: impl std::future::Future<Output = ()> + Send + 'static) {
        while self.tasks.try_join_next().is_some() {}
        self.tasks.spawn(task);
    }

    fn accepted(&mut self, machine_id: String, result: Result<Link, LinkError>) {
        self.accepting.remove(&machine_id);
        let link = match result {
            Ok(link) => link,
            Err(error) => {
                eprintln!("[lbm-agent] multi-PC: no link with {machine_id}: {error}");
                self.problem(&machine_id, LinkProblem::of_link_error(&error, true));
                self.schedule_redial(&machine_id);
                return;
            }
        };
        if self.store.get(&machine_id).is_none() {
            // Forgotten while the other side was making up its mind.
            let sender = link.sender();
            tokio::spawn(async move { sender.close("forgotten").await });
            return;
        }
        self.backoff.remove(&machine_id);
        self.problems.remove(&machine_id);
        let (info, sender, events) = link.into_parts();
        self.next_id += 1;
        let id = self.next_id;
        eprintln!(
            "[lbm-agent] multi-PC: linked with {} ({machine_id}) as its {}",
            info.hello.name,
            match info.hello.role {
                Role::Master => "remote",
                Role::Remote => "master",
            }
        );
        tokio::spawn(forward(
            id,
            machine_id.clone(),
            info.hello.role,
            events,
            sender.clone(),
            self.internal.clone(),
            self.hook.sink(),
        ));
        // This machine is the remote: its screens, from the first moment.
        if info.hello.role == Role::Master {
            self.send_displays(&sender);
            // What was last read: the agent reads again for a master that links, and a
            // newer reading follows under a higher revision if the desktop changed since.
            if self.wallpaper_revision > 0 {
                self.send_wallpapers(&sender);
            }
        }
        let live = Live {
            id,
            sender,
            info,
            displays: None,
            wallpapers: None,
        };
        if let Some(older) = self.links.insert(machine_id, live) {
            tokio::spawn(async move {
                older.sender.close("replaced by a newer connection").await;
            });
        }
        self.select();
    }

    /// Calls back every paired remote this machine knows an address for.
    fn call_remotes_back(&mut self) {
        let remotes: Vec<String> = self
            .store
            .peers()
            .iter()
            .filter(|p| p.role == Role::Remote && !p.address.is_empty())
            .map(|p| p.machine_id.clone())
            .collect();
        for machine_id in remotes {
            self.call_back(&machine_id);
        }
    }

    /// Dials a paired remote, unless it is linked or being linked already.
    fn call_back(&mut self, machine_id: &str) {
        let Some(paired) = self.store.get(machine_id) else {
            return;
        };
        if paired.role != Role::Remote
            || paired.address.is_empty()
            || self.links.contains_key(machine_id)
            || self.accepting.contains(machine_id)
            || self.dialing.contains(machine_id)
        {
            return;
        }
        let address = paired.address.clone();
        self.dial(address, Some(machine_id.to_owned()), None);
    }

    /// Tries a paired remote again later, waiting twice as long as the last time, up to
    /// thirty seconds. Only while the mode is on: the timer reports to this task, which
    /// is gone once it is off.
    fn schedule_redial(&mut self, machine_id: &str) {
        let Some(paired) = self.store.get(machine_id) else {
            return;
        };
        if paired.role != Role::Remote
            || paired.address.is_empty()
            || self.links.contains_key(machine_id)
            || !self.redial_scheduled.insert(machine_id.to_owned())
        {
            return;
        }
        let delay = self
            .backoff
            .get(machine_id)
            .copied()
            .unwrap_or(REDIAL_FIRST);
        self.backoff
            .insert(machine_id.to_owned(), (delay * 2).min(REDIAL_MAX));
        let internal = self.internal.clone();
        let machine_id = machine_id.to_owned();
        self.spawn(async move {
            tokio::time::sleep(delay).await;
            let _ = internal.send(Internal::Redial(machine_id));
        });
    }

    //==================//
    // Live links       //
    //==================//

    fn control(&mut self, id: u64, machine_id: &str, message: Control) {
        let Some(live) = self.links.get_mut(machine_id).filter(|l| l.id == id) else {
            return;
        };
        match message {
            Control::Displays { revision, displays } if live.info.hello.role == Role::Remote => {
                if live
                    .displays
                    .as_ref()
                    .is_none_or(|(known, _)| revision > *known)
                {
                    eprintln!(
                        "[lbm-agent] multi-PC: {} reports {} display(s) (revision {revision})",
                        live.info.hello.name,
                        displays.len()
                    );
                    live.displays = Some((revision, displays));
                }
            }
            Control::Wallpapers {
                revision,
                wallpapers,
            } if live.info.hello.role == Role::Remote => {
                let newer = live
                    .wallpapers
                    .as_ref()
                    .is_none_or(|(known, _, _)| revision > *known);
                let changed = live
                    .wallpapers
                    .as_ref()
                    .is_none_or(|(_, _, known)| *known != wallpapers);
                if newer && changed {
                    self.wallpaper_stamp += 1;
                    live.wallpapers = Some((revision, self.wallpaper_stamp, wallpapers));
                } else if let Some((known, _, _)) = live.wallpapers.as_mut().filter(|_| newer) {
                    // The same pictures again: nothing for the frontends to fetch.
                    *known = revision;
                }
            }
            // The remote ended the session of the stream it was being sent: what goes on
            // being sent is ignored there until an `Enter`. The hop ends here too, so that
            // the pointer comes home and the next crossing starts a session afresh.
            Control::InputReset
                if live.info.hello.role == Role::Remote
                    && self.relaying.as_ref() == Some(&(machine_id.to_owned(), id)) =>
            {
                eprintln!(
                    "[lbm-agent] multi-PC: {} ended the input session: the pointer comes home",
                    live.info.hello.name
                );
                self.hook.restart();
            }
            // Only this machine's master may switch its monitors: a remote of this
            // machine has no say over them, and an unpaired machine never gets a link.
            Control::SelectInput {
                display,
                value,
                request,
            } if live.info.hello.role == Role::Master => {
                eprintln!(
                    "[lbm-agent] multi-PC: {} asks for input {value:#04x} on {display}",
                    live.info.hello.name
                );
                let _ = self.notices.send(Notice::SelectInput {
                    from: machine_id.to_owned(),
                    display,
                    value,
                    request,
                });
            }
            // How a switch this machine asked of its remote went.
            Control::InputSwitched {
                display,
                value,
                request,
                error,
            } if live.info.hello.role == Role::Remote => {
                let _ = self.notices.send(Notice::InputSwitched {
                    from: machine_id.to_owned(),
                    display,
                    value,
                    request,
                    error,
                });
            }
            Control::SelectInput { display, .. } => {
                eprintln!(
                    "[lbm-agent] multi-PC: {} is not this machine's master: its input switch on {display} is ignored",
                    live.info.hello.name
                );
            }
            _ => {}
        }
    }

    /// A master's stream crossed onto this machine, or left it. Only the link of the
    /// master that drives says it left: a second master's `Leave` (or the end of its
    /// link) says nothing of the first one's pointer.
    fn driving(&mut self, id: u64, machine_id: String, driving: bool) {
        let link = (machine_id, id);
        if driving {
            if self.links.get(&link.0).is_some_and(|l| l.id == id) {
                if self.driven_by.as_ref() != Some(&link) {
                    eprintln!("[lbm-agent] multi-PC: driven by {}", link.0);
                }
                self.driven_by = Some(link);
            }
        } else if self.driven_by.as_ref() == Some(&link) {
            eprintln!("[lbm-agent] multi-PC: no longer driven by {}", link.0);
            self.driven_by = None;
        }
    }

    fn closed(&mut self, id: u64, machine_id: &str, reason: CloseReason) {
        // The end of the stream is its `Leave`, whatever came before it.
        self.driving(id, machine_id.to_owned(), false);
        if !self.links.get(machine_id).is_some_and(|l| l.id == id) {
            return;
        }
        let live = self.links.remove(machine_id).expect("checked above");
        eprintln!(
            "[lbm-agent] multi-PC: the link with {} ended: {reason:?}",
            live.info.hello.name
        );
        if let Some(problem) = LinkProblem::of_close(&reason) {
            self.problem(machine_id, problem);
        }
        self.select();
        self.schedule_redial(machine_id);
    }

    /// Chooses the one remote this machine drives: its screens are the only remote ones
    /// in the layout and the hook's stream goes to it alone, since a batch carries no
    /// destination — a zone of another remote would send that remote's `Enter` and keys
    /// to this one.
    ///
    /// The selected remote stays selected for as long as its link lives: another linking
    /// meanwhile must not pull the screens the user is working on from under the pointer.
    /// When there is none (the first to link, or the selected one gone), the linked
    /// remote with the lowest machine id is taken, the order the layout merges in, so
    /// that the same machines linked give the same choice whatever the timing.
    ///
    /// A change of remote — or of link to the same one, or to none — is a new choice,
    /// numbered anew: the connection to the hook ends at once (it comes home), the previous
    /// link is told `Leave`, and nothing is relayed to the new one before the hook has
    /// zones fit for it ([`Actor::retarget`]).
    fn select(&mut self) {
        let is_remote = |live: &Live| live.info.hello.role == Role::Remote;
        let kept = self.selected.as_ref().and_then(|(machine_id, _)| {
            self.links
                .get_key_value(machine_id)
                .filter(|(_, live)| is_remote(live))
        });
        let next = kept
            .or_else(|| {
                self.links
                    .iter()
                    .filter(|(_, live)| is_remote(live))
                    .min_by(|a, b| a.0.cmp(b.0))
            })
            .map(|(machine_id, live)| ((machine_id.clone(), live.id), live.sender.clone()));
        let chosen = next.as_ref().map(|(key, _)| key.clone());
        if chosen == self.selected {
            return;
        }
        match &chosen {
            Some((machine_id, _)) => {
                eprintln!("[lbm-agent] multi-PC: the input stream goes to {machine_id}")
            }
            None => eprintln!("[lbm-agent] multi-PC: no remote to send the input stream to"),
        }
        self.selected = chosen;
        self.selection = SELECTIONS.fetch_add(1, Ordering::Relaxed) + 1;
        self.retarget();
    }

    /// Relays the hook's stream to the selected remote only while the agent says the
    /// hook's zones are fit for this very choice of it ([`Command::HookReady`]).
    ///
    /// The hook makes the other machines' zones enterable while it has a client, and a
    /// batch carries no destination. The zones it holds when the choice changes are the
    /// previous remote's until the rebuild that follows reaches it: connected meanwhile,
    /// a crossing into the old remote's zone would send its `Enter`, clicks and keys to
    /// the new one. Without a connection that border is a wall. The word must be about
    /// this choice, not merely this machine: a choice gone and come back may have left a
    /// load for the other one on its way to the hook.
    fn retarget(&mut self) {
        let ready = self.hook_ready == Some(self.selection);
        let next = self.selected.clone().filter(|_| ready);
        if next == self.relaying {
            return;
        }
        let sender = next
            .as_ref()
            .and_then(|(machine_id, _)| self.links.get(machine_id))
            .map(|live| Target {
                link: live.sender.clone(),
                reads_locks: live
                    .info
                    .hello
                    .capabilities
                    .iter()
                    .any(|c| c == LOCKS_CAPABILITY),
            });
        self.relaying = next;
        self.hook.set_target(sender);
    }

    fn forget(&mut self, machine_id: &str) -> Result<Value, String> {
        let stored = self
            .store
            .remove(machine_id)
            .map_err(|error| format!("cannot forget {machine_id}: {error}"))?;
        let waiting = self.pending.remove(machine_id).is_some();
        self.backoff.remove(machine_id);
        self.problems.remove(machine_id);
        if let Some(live) = self.links.remove(machine_id) {
            tokio::spawn(async move { live.sender.close("forgotten").await });
            self.select();
        }
        if stored || waiting {
            Ok(Value::Null)
        } else {
            Err(format!("no machine {machine_id} is known"))
        }
    }

    /// Sends `SelectInput` to `machine_id`, if it is linked as a remote of this machine.
    /// Queued in a task of its own, as `Displays` is.
    fn select_input(
        &mut self,
        machine_id: &str,
        display: String,
        value: u16,
        request: Option<u64>,
    ) {
        let Some(live) = self
            .links
            .get(machine_id)
            .filter(|live| live.info.hello.role == Role::Remote)
        else {
            eprintln!(
                "[lbm-agent] multi-PC: {machine_id} is not linked as a remote: cannot ask it to switch {display}"
            );
            return;
        };
        let sender = live.sender.clone();
        let message = Control::SelectInput {
            display,
            value,
            request,
        };
        tokio::spawn(async move {
            let _ = sender.control(&message).await;
        });
    }

    /// Tells this machine's master `machine_id` how its `SelectInput` went, if it is
    /// still linked as this machine's master.
    fn input_switched(
        &mut self,
        machine_id: &str,
        display: String,
        value: u16,
        request: Option<u64>,
        error: Option<String>,
    ) {
        let Some(live) = self
            .links
            .get(machine_id)
            .filter(|live| live.info.hello.role == Role::Master)
        else {
            return;
        };
        let sender = live.sender.clone();
        let message = Control::InputSwitched {
            display,
            value,
            request,
            error,
        };
        tokio::spawn(async move {
            let _ = sender.control(&message).await;
        });
    }

    /// Records why linking with a paired machine went wrong, for the state to show. A
    /// machine this one does not keep has no row to show it on.
    fn problem(&mut self, machine_id: &str, problem: LinkProblem) {
        if self.store.get(machine_id).is_some() {
            self.problems.insert(machine_id.to_owned(), problem);
        }
    }

    /// This machine's screens changed: every master this machine is the remote of is
    /// told, under a new revision.
    fn local_displays(&mut self, displays: Vec<PeerDisplay>) {
        if displays == self.displays && self.revision > 0 {
            return;
        }
        self.displays = displays;
        self.revision += 1;
        let masters: Vec<LinkSender> = self
            .links
            .values()
            .filter(|live| live.info.hello.role == Role::Master)
            .map(|live| live.sender.clone())
            .collect();
        for sender in masters {
            self.send_displays(&sender);
        }
    }

    /// What this machine's screens show changed: every master this machine is the remote
    /// of is told, under a new revision. The same pictures again are no change.
    fn local_wallpapers(&mut self, wallpapers: Vec<PeerWallpaper>) {
        if wallpapers == self.wallpapers && self.wallpaper_revision > 0 {
            return;
        }
        self.wallpapers = wallpapers;
        self.wallpaper_revision += 1;
        let masters: Vec<LinkSender> = self
            .links
            .values()
            .filter(|live| live.info.hello.role == Role::Master)
            .map(|live| live.sender.clone())
            .collect();
        for sender in masters {
            self.send_wallpapers(&sender);
        }
    }

    /// As `send_displays`, and in a task of its own for the same reason. The pictures go
    /// from the remote to the master, the opposite way to the input stream, so they never
    /// queue ahead of a click.
    fn send_wallpapers(&self, sender: &LinkSender) {
        let message = Control::Wallpapers {
            revision: self.wallpaper_revision,
            wallpapers: self.wallpapers.clone(),
        };
        let sender = sender.clone();
        tokio::spawn(async move {
            let _ = sender.control(&message).await;
        });
    }

    /// `PeerWallpapers`: what a remote of this machine last sent, under the stamp the
    /// `Peers` state shows. Nothing yet (a remote not linked, or older than wallpapers)
    /// is an empty list under stamp 0, not an error: the map just draws no picture.
    fn remote_wallpapers(&self, machine_id: &str) -> Result<Value, String> {
        if self.store.get(machine_id).is_none() && !self.links.contains_key(machine_id) {
            return Err(format!("no machine {machine_id} is known"));
        }
        let (stamp, wallpapers) = self
            .links
            .get(machine_id)
            .and_then(|live| live.wallpapers.as_ref())
            .map_or((0, &[][..]), |(_, stamp, wallpapers)| {
                (*stamp, wallpapers.as_slice())
            });
        Ok(serde_json::json!({
            "MachineId": machine_id,
            "Revision": stamp,
            "Wallpapers": wallpapers,
        }))
    }

    /// Queued in a task of its own: a control message may wait for room on a busy
    /// link, and this loop may not. Two sends can overtake each other that way, which
    /// the revision makes harmless — the master keeps the highest.
    fn send_displays(&self, sender: &LinkSender) {
        let message = Control::Displays {
            revision: self.revision,
            displays: self.displays.clone(),
        };
        let sender = sender.clone();
        tokio::spawn(async move {
            let _ = sender.control(&message).await;
        });
    }

    /// Says `Bye` on every link and lets go of everything.
    async fn shutdown(&mut self) {
        self.selected = None;
        self.driven_by = None;
        self.relaying = None;
        self.hook.set_target(None);
        self.pending.clear();
        // Half-made connections are dropped, which closes them; timers simply stop.
        self.tasks.abort_all();
        let senders: Vec<LinkSender> = self.links.drain().map(|(_, live)| live.sender).collect();
        let _ = tokio::time::timeout(SHUTDOWN_PATIENCE, async {
            for sender in senders {
                sender.close("the multi-PC mode was turned off").await;
            }
        })
        .await;
        eprintln!("[lbm-agent] multi-PC: stopped");
    }
}

/// One link's events, in a task of their own. The input batches go straight to the
/// hook, never through the manager's loop: a thousand reports a second have nothing to
/// decide there. Everything else goes to the manager.
async fn forward(
    id: u64,
    machine_id: String,
    their_role: Role,
    mut events: mpsc::Receiver<Event>,
    link: LinkSender,
    internal: mpsc::UnboundedSender<Internal>,
    mut hook: Sink,
) {
    // Only a master's batches are injected: a remote has no business driving this
    // machine's pointer.
    let injecting = their_role == Role::Master;
    // The session of this master's under way in the hook: the number of its first
    // batch queued, until the master's `Leave` or the end of the session here.
    let mut session: Option<u64> = None;
    // `InputReset` was said: what the master sends before it hears of it opens no
    // session, only its next crossing (or its `Leave`) does.
    let mut reset_said = false;
    // A batch of this link's was queued for the hook since its stream was last ended
    // here: the end of the link ends it, whether a session is under way or the master
    // left cleanly, so the hook does not go on holding a client for a master gone.
    let mut used = false;
    // This master's pointer is on this machine: its `Enter` reached the hook, and
    // neither its `Leave` nor the end of its session here has since. Told to the
    // manager as it changes, for the state.
    let mut driving = false;
    let drive = |driving: &mut bool, now: bool| {
        if std::mem::replace(driving, now) != now {
            let _ = internal.send(Internal::Driving {
                id,
                machine_id: machine_id.clone(),
                driving: now,
            });
        }
    };
    let reason = loop {
        let event = tokio::select! {
            lost = hook.lost() => {
                // A batch of the session never reached the hook, or reached it on a
                // connection that ended since by no doing of this link's (the hook
                // went, a write failed or took too long): the session is over there,
                // and what follows is ignored until an `Enter`. The master is told, as
                // after a full queue below. Batches lost before the session began say
                // nothing of it.
                if session.is_some_and(|first| lost >= first) {
                    session = None;
                    reset_said = true;
                    eprintln!(
                        "[lbm-agent] multi-PC: the hook lost input of the session: \
                         the master is told"
                    );
                    drive(&mut driving, false);
                    reset(&link);
                }
                continue;
            }
            event = events.recv() => event,
        };
        match event {
            Some(Event::Input(batch)) if injecting => {
                let (crosses, leaves) = session_edges(&batch);
                if let Some(number) = hook.try_batch(batch, crosses) {
                    used = true;
                    if leaves {
                        session = None;
                        reset_said = false;
                        drive(&mut driving, false);
                    } else if crosses || !reset_said {
                        if crosses {
                            drive(&mut driving, true);
                        }
                        reset_said = false;
                        session.get_or_insert(number);
                    }
                } else if session.is_some() || crosses {
                    // The hook is not keeping up and this batch is lost: it may have been
                    // a release, or the crossing itself. The stream ends here instead, so
                    // the hook releases what it holds; what follows reaches it as a new
                    // stream, ignored until the master's next `Enter`. The master is
                    // told, or it would go on sending into that void, its pointer and
                    // keyboards held here, until the user crossed home and back.
                    if std::mem::take(&mut used) {
                        hook.release().await;
                    }
                    session = None;
                    reset_said = true;
                    drive(&mut driving, false);
                    reset(&link);
                }
            }
            Some(Event::Input(_)) => {}
            Some(Event::Control(message)) => {
                let _ = internal.send(Internal::Control {
                    id,
                    machine_id: machine_id.clone(),
                    message,
                });
            }
            Some(Event::Closed(reason)) => break reason,
            None => break CloseReason::Local("the link stopped".into()),
        }
    };
    // The stream is over: the hook releases whatever it left pressed, and has no client
    // for this master any more.
    if used {
        hook.release().await;
    }
    let _ = internal.send(Internal::Closed {
        id,
        machine_id,
        reason,
    });
}

/// Tells the master the session of its stream ended here: what it goes on sending is
/// ignored until its next `Enter`. Sent from a task of its own: the send may wait for
/// room, and the link's loop must go on reading.
fn reset(link: &LinkSender) {
    let link = link.clone();
    tokio::spawn(async move {
        let _ = link.control(&Control::InputReset).await;
    });
}

/// Whether `batch` crosses (carries an `Enter`), and whether it ends the master's
/// session: the pointer went home with it and the hook let go of everything (its last
/// `Enter` or `Leave` is a `Leave`).
fn session_edges(batch: &[u8]) -> (bool, bool) {
    let Ok(events) = lbm_ipc::input::decode(batch) else {
        return (false, false);
    };
    let edges = events
        .iter()
        .filter(|event| matches!(event, InputEvent::Enter { .. } | InputEvent::Leave));
    let crosses = edges.clone().any(|event| *event != InputEvent::Leave);
    let leaves = edges.clone().next_back() == Some(&InputEvent::Leave);
    (crosses, leaves)
}

/// `host`, `host:port`, `[v6]` or `[v6]:port`, with the protocol's port when none is
/// given.
pub(super) fn with_default_port(address: &str) -> String {
    let address = address.trim();
    if address.parse::<SocketAddr>().is_ok() {
        return address.to_owned();
    }
    if let Ok(ip) = address.parse::<IpAddr>() {
        return SocketAddr::new(ip, lbm_net::DEFAULT_PORT).to_string();
    }
    if (address.starts_with('[') && address.ends_with(']')) || !address.contains(':') {
        return format!("{address}:{}", lbm_net::DEFAULT_PORT);
    }
    address.to_owned()
}

#[cfg(test)]
mod tests {
    use super::with_default_port;

    #[test]
    fn an_address_without_a_port_gets_the_protocols() {
        assert_eq!(with_default_port("192.168.1.30"), "192.168.1.30:24817");
        assert_eq!(
            with_default_port(" 192.168.1.30:5000 "),
            "192.168.1.30:5000"
        );
        assert_eq!(with_default_port("tower"), "tower:24817");
        assert_eq!(with_default_port("tower.lan:7"), "tower.lan:7");
        assert_eq!(with_default_port("fe80::1"), "[fe80::1]:24817");
        assert_eq!(with_default_port("[fe80::1]"), "[fe80::1]:24817");
        assert_eq!(with_default_port("[fe80::1]:9"), "[fe80::1]:9");
    }
}
