//! The peer manager's task while the mode is on: the listener, the pairings waiting for
//! the user, the live links, the reconnections. It owns all of them and nothing else
//! touches them; the agent's loop talks to it through [`Command`]s and hears from it
//! through [`Notice`]s, so that nothing the network does can hold that loop up.

use std::collections::{HashMap, HashSet};
use std::net::{IpAddr, SocketAddr};
use std::time::Duration;

use lbm_ipc::peer::{Peer, PeerDisplay};
use lbm_net::identity::Identity;
use lbm_net::link::{
    self, CloseReason, Event, Link, LinkError, LinkSender, Listener, PeerInfo, Pending,
};
use lbm_net::peers::{PairedPeer, PeerStore};
use lbm_net::wire::{Control, Hello, Role};
use serde_json::Value;
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinSet;

use super::hook_input::{HookInput, Sink};
use super::{Notice, PeerEntry, PeerState, PeersState};

/// The first wait before calling a remote back, doubled at every failure up to
/// [`REDIAL_MAX`].
const REDIAL_FIRST: Duration = Duration::from_secs(1);
const REDIAL_MAX: Duration = Duration::from_secs(30);

/// How long the links are given to say `Bye` when the mode is turned off.
const SHUTDOWN_PATIENCE: Duration = Duration::from_secs(1);

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
    /// `SelectInput` to a remote of this machine.
    SelectInput {
        machine_id: String,
        display: String,
        value: u16,
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
    /// This machine's screens, and how many times they changed.
    displays: Vec<PeerDisplay>,
    revision: u64,
    hook: HookInput,
    /// The remote the hook's stream goes to (this machine is its master).
    streaming_to: Option<String>,
    internal: mpsc::UnboundedSender<Internal>,
    notices: mpsc::UnboundedSender<Notice>,
    published: Option<PeersState>,
    next_id: u64,
    next_serial: u64,
    /// Connections being made or accepted, and timers (see [`Actor::spawn`]).
    tasks: JoinSet<()>,
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
    let hello = Hello::new(start.name.clone(), env!("CARGO_PKG_VERSION"), Role::Remote);
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
        // A list handed over at start is this machine's screens already: revision 1, so
        // the first change after it is told apart.
        revision: u64::from(!start.displays.is_empty()),
        displays: start.displays,
        hook: HookInput::spawn(start.input_endpoint),
        streaming_to: None,
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
        }
    }
}

impl Actor {
    fn hello(&self) -> Hello {
        Hello::new(self.name.clone(), env!("CARGO_PKG_VERSION"), Role::Master)
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
                    },
                    role: paired.role,
                    state: PeerState::Paired,
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
                },
                role: info.hello.role,
                state: PeerState::Pending,
            }
        }));
        PeersState {
            enabled: true,
            port: self.port,
            name: self.name.clone(),
            machine_id: Some(self.identity.fingerprint().to_owned()),
            listen_address: self.listen_address.map(|a| a.to_string()),
            error: self.listen_error.clone(),
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
            Command::SelectInput {
                machine_id,
                display,
                value,
            } => self.select_input(&machine_id, display, value),
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
            },
            role: info.hello.role,
            state: if self.store.get(&info.fingerprint).is_some() {
                PeerState::Paired
            } else {
                PeerState::Pending
            },
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
            self.internal.clone(),
            self.hook.sink(),
        ));
        match info.hello.role {
            // This machine is the remote: its screens, from the first moment.
            Role::Master => self.send_displays(&sender),
            // This machine is the master: the hook's stream goes to that remote. One at
            // a time for now: the newest link takes the stream (see `peers` docs).
            Role::Remote => {
                self.hook.set_target(Some(sender.clone()));
                self.streaming_to = Some(machine_id.clone());
            }
        }
        let live = Live {
            id,
            sender,
            info,
            displays: None,
        };
        if let Some(older) = self.links.insert(machine_id, live) {
            tokio::spawn(async move {
                older.sender.close("replaced by a newer connection").await;
            });
        }
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
            // Only this machine's master may switch its monitors: a remote of this
            // machine has no say over them, and an unpaired machine never gets a link.
            Control::SelectInput { display, value } if live.info.hello.role == Role::Master => {
                eprintln!(
                    "[lbm-agent] multi-PC: {} asks for input {value:#04x} on {display}",
                    live.info.hello.name
                );
                let _ = self.notices.send(Notice::SelectInput {
                    from: machine_id.to_owned(),
                    display,
                    value,
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

    fn closed(&mut self, id: u64, machine_id: &str, reason: CloseReason) {
        if !self.links.get(machine_id).is_some_and(|l| l.id == id) {
            return;
        }
        let live = self.links.remove(machine_id).expect("checked above");
        eprintln!(
            "[lbm-agent] multi-PC: the link with {} ended: {reason:?}",
            live.info.hello.name
        );
        if self.streaming_to.as_deref() == Some(machine_id) {
            self.retarget();
        }
        self.schedule_redial(machine_id);
    }

    /// The stream goes to another linked remote, if one is left, else nowhere.
    fn retarget(&mut self) {
        let next = self
            .links
            .iter()
            .filter(|(_, live)| live.info.hello.role == Role::Remote)
            .max_by_key(|(_, live)| live.id)
            .map(|(machine_id, live)| (machine_id.clone(), live.sender.clone()));
        self.streaming_to = next.as_ref().map(|(machine_id, _)| machine_id.clone());
        self.hook.set_target(next.map(|(_, sender)| sender));
    }

    fn forget(&mut self, machine_id: &str) -> Result<Value, String> {
        let stored = self
            .store
            .remove(machine_id)
            .map_err(|error| format!("cannot forget {machine_id}: {error}"))?;
        let waiting = self.pending.remove(machine_id).is_some();
        self.backoff.remove(machine_id);
        if let Some(live) = self.links.remove(machine_id) {
            tokio::spawn(async move { live.sender.close("forgotten").await });
            if self.streaming_to.as_deref() == Some(machine_id) {
                self.retarget();
            }
        }
        if stored || waiting {
            Ok(Value::Null)
        } else {
            Err(format!("no machine {machine_id} is known"))
        }
    }

    /// Sends `SelectInput` to `machine_id`, if it is linked as a remote of this machine.
    /// Queued in a task of its own, as `Displays` is.
    fn select_input(&mut self, machine_id: &str, display: String, value: u16) {
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
        let message = Control::SelectInput { display, value };
        tokio::spawn(async move {
            let _ = sender.control(&message).await;
        });
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
    internal: mpsc::UnboundedSender<Internal>,
    hook: Sink,
) {
    // Only a master's batches are injected: a remote has no business driving this
    // machine's pointer.
    let injecting = their_role == Role::Master;
    let mut injected = false;
    let reason = loop {
        match events.recv().await {
            Some(Event::Input(batch)) if injecting => injected |= hook.try_batch(batch),
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
    // The stream is over: the hook releases whatever it left pressed.
    if injected {
        hook.release().await;
    }
    let _ = internal.send(Internal::Closed {
        id,
        machine_id,
        reason,
    });
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
