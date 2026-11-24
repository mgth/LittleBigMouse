//! The multi-PC manager over real sockets on loopback: pairing, the screens a remote
//! publishes, and the input stream from one hook's endpoint to the other's.

use std::path::Path;
use std::time::Duration;

use lbm_agent::peers::{
    Answer, LinkProblem, Notice, PeerRequest, PeerState, PeersState, ProblemKind, Service,
    Settings, Setup,
};
use lbm_ipc::peer::PeerDisplay;
use lbm_net::identity::Identity;
use lbm_net::link::{self, Event, Link, LinkError};
use lbm_net::peers::{PairedPeer, PeerStore};
use lbm_net::wire::{Control, Hello, Role};
use serde_json::Value;
use tokio::sync::mpsc::UnboundedReceiver;

const PATIENCE: Duration = Duration::from_secs(10);

fn setup(input_endpoint: Option<String>) -> Setup {
    setup_with(input_endpoint, link::Options::default())
}

fn setup_with(input_endpoint: Option<String>, options: link::Options) -> Setup {
    Setup {
        bind: "127.0.0.1".parse().unwrap(),
        input_endpoint,
        options,
        // Never the system's clipboard in a test: the developer's own is in it.
        clipboard: lbm_agent::clipboard::memory::Memory::new().factory(),
    }
}

/// A port nobody listens on right now.
fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

fn display(id: &str, x: i32) -> PeerDisplay {
    PeerDisplay {
        id: id.into(),
        connector: format!("{id}-connector"),
        name: id.into(),
        edid: Some("AP///////wA=".into()),
        x,
        y: 0,
        width: 1920,
        height: 1080,
        scale: 1.0,
        effective_dpi: 96.0,
        orientation: 0,
        frequency: 60.0,
        primary: x == 0,
        attached: true,
        width_mm: 527.0,
        height_mm: 296.0,
        input_source: None,
    }
}

/// One machine's manager, as the agent's loop would hold it.
struct Node {
    service: Service,
    notices: UnboundedReceiver<Notice>,
    /// The last state announced: what the agent's loop would know. Kept whole, unlike the
    /// answer to `Peers`, which carries no `selection`.
    last: Option<PeersState>,
}

impl Node {
    fn new(directory: &Path, input_endpoint: Option<String>) -> Node {
        Node::with_setup(directory, setup(input_endpoint))
    }

    fn with_setup(directory: &Path, setup: Setup) -> Node {
        let mut service = Service::new(directory.to_owned(), setup);
        let notices = service.take_notices().unwrap();
        Node {
            service,
            notices,
            last: None,
        }
    }

    async fn ask(&mut self, request: PeerRequest) -> Result<Value, String> {
        match self.service.request(request) {
            Answer::Now(result) => result,
            Answer::Later(result) => tokio::time::timeout(PATIENCE, result)
                .await
                .expect("an answer in time"),
        }
    }

    async fn state(&mut self) -> PeersState {
        serde_json::from_value(self.ask(PeerRequest::State).await.unwrap()).unwrap()
    }

    async fn turn_on(&mut self, port: u16, name: &str) -> PeersState {
        let state = self
            .ask(PeerRequest::Set {
                enabled: true,
                port: Some(port),
                name: Some(name.into()),
                pointer_lock_shortcut: None,
                clipboard: Default::default(),
            })
            .await
            .unwrap();
        serde_json::from_value(state).unwrap()
    }

    /// The first state, the latest announced or one announced later, that satisfies
    /// `want`.
    async fn until(&mut self, want: impl Fn(&PeersState) -> bool) -> PeersState {
        while let Ok(notice) = self.notices.try_recv() {
            if let Notice::Peers(state) = notice {
                self.last = Some(state);
            }
        }
        if let Some(last) = self.last.clone().filter(|state| want(state)) {
            return last;
        }
        tokio::time::timeout(PATIENCE, async {
            loop {
                let Notice::Peers(state) = self.notices.recv().await.expect("notices") else {
                    continue;
                };
                self.last = Some(state.clone());
                if want(&state) {
                    return state;
                }
            }
        })
        .await
        .expect("the state wanted, in time")
    }

    /// Says, as the agent does once its hook confirmed zones leading to `machine_id` and
    /// nowhere else, that the hook is fit for the choice of that remote; that choice's
    /// number, for a test to repeat it later.
    #[cfg(unix)]
    async fn hook_ready_for(&mut self, machine_id: &str) -> u64 {
        let state = self
            .until(|s| s.selected_remote.as_deref() == Some(machine_id))
            .await;
        self.service.hook_ready(Some(state.selection));
        state.selection
    }
}

fn peer<'a>(state: &'a PeersState, machine_id: &str) -> Option<&'a lbm_agent::peers::PeerEntry> {
    state.peers.iter().find(|p| p.peer.machine_id == machine_id)
}

fn linked(state: &PeersState, machine_id: &str) -> bool {
    peer(state, machine_id).is_some_and(|p| p.peer.connected && p.state == PeerState::Paired)
}

/// A test master: an identity of its own, nothing else of an agent.
async fn master_connects(identity: &Identity, address: &str) -> lbm_net::link::Pending {
    link::connect(
        address,
        identity,
        Hello::new("test master", "0", Role::Master),
        link::Options::default(),
    )
    .await
    .unwrap()
}

/// The next control message on `link`, skipping input.
async fn next_control(link: &mut Link) -> Control {
    tokio::time::timeout(PATIENCE, async {
        loop {
            match link.next().await {
                Some(Event::Control(message)) => return message,
                Some(Event::Input(_)) => {}
                other => panic!("the link ended: {other:?}"),
            }
        }
    })
    .await
    .expect("a control message in time")
}

/// Trusts `machine_id` in `directory`'s store, as a pairing the user made earlier.
fn pair(directory: &Path, machine_id: &str, address: &str, role: Role) {
    PeerStore::load(directory)
        .insert(PairedPeer {
            machine_id: machine_id.into(),
            name: "paired".into(),
            address: address.into(),
            role,
        })
        .unwrap();
}

#[tokio::test]
async fn with_the_mode_off_nothing_listens_and_no_identity_is_made() {
    let dir = tempfile::tempdir().unwrap();
    let port = free_port();
    Settings {
        enabled: false,
        port,
        name: None,
        pointer_lock_shortcut: None,
        ..Settings::default()
    }
    .save(dir.path())
    .unwrap();
    let mut node = Node::new(dir.path(), None);

    let state = node.state().await;
    assert!(!state.enabled);
    assert_eq!(state.port, port);
    assert_eq!(state.listen_address, None);
    assert_eq!(state.machine_id, None);
    assert!(state.peers.is_empty());
    // The port is free: nobody took it.
    std::net::TcpListener::bind(("127.0.0.1", port)).unwrap();
    let error = node
        .ask(PeerRequest::Connect("127.0.0.1:1".into()))
        .await
        .unwrap_err();
    assert!(error.contains("off"), "{error}");

    let names: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect();
    assert_eq!(names, ["multi-pc.json"], "no identity, no store");
}

#[tokio::test]
async fn two_machines_pair_once_both_users_accept_and_stay_paired_after_a_restart() {
    let (desk_dir, tower_dir) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let tower_port = free_port();
    let mut tower = Node::new(tower_dir.path(), None);
    let tower_state = tower.turn_on(tower_port, "tower").await;
    let tower_id = tower_state.machine_id.clone().unwrap();
    assert_eq!(
        tower_state.listen_address.as_deref(),
        Some(format!("127.0.0.1:{tower_port}").as_str())
    );
    let mut desk = Node::new(desk_dir.path(), None);
    let desk_id = desk.turn_on(0, "desk").await.machine_id.unwrap();
    assert_ne!(desk_id, tower_id);

    // The desk connects: each side sees the other waiting, with its fingerprint.
    let connected = desk
        .ask(PeerRequest::Connect(format!("127.0.0.1:{tower_port}")))
        .await
        .unwrap();
    assert_eq!(connected["MachineId"], tower_id);
    assert_eq!(connected["Name"], "tower");
    assert_eq!(connected["Role"], "Remote");
    assert_eq!(connected["State"], "Pending");
    let waiting = tower
        .until(|s| peer(s, &desk_id).is_some_and(|p| p.state == PeerState::Pending))
        .await;
    let desk_seen = peer(&waiting, &desk_id).unwrap();
    assert_eq!(desk_seen.peer.name, "desk");
    assert_eq!(desk_seen.role, Role::Master);
    assert!(!desk_seen.peer.connected);

    // One yes is not a pairing.
    desk.ask(PeerRequest::Accept(tower_id.clone()))
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(!linked(&desk.state().await, &tower_id));
    tower
        .ask(PeerRequest::Accept(desk_id.clone()))
        .await
        .unwrap();
    desk.until(|s| linked(s, &tower_id)).await;
    let tower_now = tower.until(|s| linked(s, &desk_id)).await;
    assert_eq!(peer(&tower_now, &desk_id).unwrap().role, Role::Master);

    // Both stores hold the other; the desk knows where to call the tower back.
    let desk_store = PeerStore::load(desk_dir.path());
    let stored = desk_store.get(&tower_id).unwrap();
    assert_eq!(stored.role, Role::Remote);
    assert_eq!(stored.address, format!("127.0.0.1:{tower_port}"));
    assert_eq!(
        PeerStore::load(tower_dir.path())
            .get(&desk_id)
            .unwrap()
            .role,
        Role::Master
    );

    // Both restart: the desk calls the tower back, and nobody is asked anything.
    desk.service.stop().await;
    tower.service.stop().await;
    drop((desk, tower));
    let mut tower = Node::new(tower_dir.path(), None);
    let mut desk = Node::new(desk_dir.path(), None);
    desk.until(|s| linked(s, &tower_id)).await;
    tower.until(|s| linked(s, &desk_id)).await;
}

#[tokio::test]
async fn an_unknown_machine_waits_for_the_user_and_is_told_nothing() {
    let tower_dir = tempfile::tempdir().unwrap();
    let mut tower = Node::new(tower_dir.path(), None);
    let address = tower.turn_on(0, "tower").await.listen_address.unwrap();
    tower.service.local_displays(vec![display("A", 0)]);

    let master_dir = tempfile::tempdir().unwrap();
    let master = Identity::load_or_create(master_dir.path()).unwrap();
    let pending = master_connects(&master, &address).await;
    tower
        .until(|s| peer(s, master.fingerprint()).is_some_and(|p| p.state == PeerState::Pending))
        .await;

    // The master says yes; the tower's user has not: no `Paired`, so no link and no
    // screens — a frame other than `Paired` would fail the accept as a protocol error.
    let accepting = tokio::spawn(pending.accept());
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert!(!accepting.is_finished(), "nothing but Hello until accepted");
    let state = tower.state().await;
    let waiting = peer(&state, master.fingerprint()).unwrap();
    assert_eq!(waiting.state, PeerState::Pending);
    assert!(!waiting.peer.connected);
    assert!(PeerStore::load(tower_dir.path()).peers().is_empty());

    tower
        .ask(PeerRequest::Refuse(master.fingerprint().to_owned()))
        .await
        .unwrap();
    let refused = tokio::time::timeout(PATIENCE, accepting)
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(refused, Err(LinkError::Refused(_))), "{refused:?}");
    tower.until(|s| s.peers.is_empty()).await;
    assert!(PeerStore::load(tower_dir.path()).peers().is_empty());
}

#[tokio::test]
async fn a_remote_publishes_its_screens_at_link_up_and_after_a_display_change() {
    let master_dir = tempfile::tempdir().unwrap();
    let master = Identity::load_or_create(master_dir.path()).unwrap();
    let tower_dir = tempfile::tempdir().unwrap();
    pair(tower_dir.path(), master.fingerprint(), "", Role::Master);

    let mut tower = Node::new(tower_dir.path(), None);
    tower.service.local_displays(vec![display("A", 0)]);
    let address = tower.turn_on(0, "tower").await.listen_address.unwrap();

    let mut link = master_connects(&master, &address)
        .await
        .accept()
        .await
        .expect("a paired master is accepted without asking");
    let Control::Displays { revision, displays } = next_control(&mut link).await else {
        panic!("Displays first");
    };
    assert_eq!(displays, [display("A", 0)]);

    tower
        .service
        .local_displays(vec![display("A", 0), display("B", 1920)]);
    let Control::Displays {
        revision: next,
        displays,
    } = next_control(&mut link).await
    else {
        panic!("Displays again");
    };
    assert!(next > revision, "{next} after {revision}");
    assert_eq!(displays, [display("A", 0), display("B", 1920)]);

    // The same screens again are no change.
    tower
        .service
        .local_displays(vec![display("A", 0), display("B", 1920)]);
    let quiet = tokio::time::timeout(Duration::from_millis(300), next_control(&mut link)).await;
    assert!(quiet.is_err(), "nothing new to say: {quiet:?}");
}

/// A stand-in for a hook's input endpoint: what it is sent, batch by batch, and `None`
/// when the client closes the connection.
#[cfg(unix)]
mod endpoint {
    use std::path::Path;
    use std::time::Duration;

    use lbm_ipc::input::{read_batch, write_batch};
    use tokio::sync::mpsc;

    pub struct Endpoint {
        pub path: String,
        pub received: mpsc::UnboundedReceiver<Option<Vec<u8>>>,
        /// Batches written to the client of the moment (a master hook's capture).
        pub to_client: mpsc::UnboundedSender<Vec<u8>>,
    }

    pub fn serve(directory: &Path, name: &str) -> Endpoint {
        serve_after(directory, name, Duration::ZERO)
    }

    /// As [`serve`], with a hook that only starts reading `pause` after the client
    /// connected: one too busy to keep up for a while. A client that connects again is
    /// served again, as the hook serves its newest client; the pause is the first's.
    pub fn serve_after(directory: &Path, name: &str, pause: Duration) -> Endpoint {
        let path = directory.join(name);
        let listener = tokio::net::UnixListener::bind(&path).unwrap();
        let (received_tx, received) = mpsc::unbounded_channel();
        let (to_client, mut outgoing) = mpsc::unbounded_channel::<Vec<u8>>();
        let current = std::sync::Arc::new(tokio::sync::Mutex::new(None));
        let writer = current.clone();
        tokio::spawn(async move {
            // What is written before a client connects waits for it.
            while let Some(batch) = outgoing.recv().await {
                loop {
                    if let Some(writer) = writer.lock().await.as_mut() {
                        let _ = write_batch(writer, &batch).await;
                        break;
                    }
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            }
        });
        tokio::spawn(async move {
            let mut pause = pause;
            while let Ok((stream, _)) = listener.accept().await {
                tokio::time::sleep(std::mem::take(&mut pause)).await;
                let (mut reader, writer) = tokio::io::split(stream);
                *current.lock().await = Some(writer);
                let received_tx = received_tx.clone();
                tokio::spawn(async move {
                    while let Ok(batch) = read_batch(&mut reader).await {
                        let _ = received_tx.send(Some(batch));
                    }
                    let _ = received_tx.send(None);
                });
            }
        });
        Endpoint {
            path: path.to_string_lossy().into_owned(),
            received,
            to_client,
        }
    }

    impl Endpoint {
        pub async fn next(&mut self) -> Option<Vec<u8>> {
            tokio::time::timeout(super::PATIENCE, self.received.recv())
                .await
                .expect("something from the client in time")
                .expect("the endpoint task")
        }
    }
}

#[cfg(unix)]
fn batch(x: i32) -> Vec<u8> {
    use lbm_ipc::input::{encode, InputEvent};
    encode(&[InputEvent::Motion { x, y: 2 * x }])
}

#[cfg(unix)]
#[tokio::test]
async fn a_masters_batches_reach_the_remotes_hook_in_order_and_its_end_releases_the_hook() {
    let master_dir = tempfile::tempdir().unwrap();
    let master = Identity::load_or_create(master_dir.path()).unwrap();
    let tower_dir = tempfile::tempdir().unwrap();
    pair(tower_dir.path(), master.fingerprint(), "", Role::Master);
    let mut hook = endpoint::serve(tower_dir.path(), "input.sock");

    let mut tower = Node::new(tower_dir.path(), Some(hook.path.clone()));
    let address = tower.turn_on(0, "tower").await.listen_address.unwrap();
    let mut link = master_connects(&master, &address)
        .await
        .accept()
        .await
        .unwrap();
    next_control(&mut link).await;

    let sender = link.sender();
    for x in 1..=5 {
        sender.input(&batch(x)).await.unwrap();
    }
    for x in 1..=5 {
        assert_eq!(hook.next().await, Some(batch(x)));
    }

    // The link ends: the hook's connection with it, so the hook releases what is held.
    sender.close("done").await;
    assert_eq!(hook.next().await, None);
}

#[cfg(unix)]
#[tokio::test]
async fn the_remotes_state_says_which_master_drives_it() {
    use lbm_ipc::input::{encode, InputEvent};
    let master_dir = tempfile::tempdir().unwrap();
    let master = Identity::load_or_create(master_dir.path()).unwrap();
    let master_id = master.fingerprint().to_owned();
    let tower_dir = tempfile::tempdir().unwrap();
    pair(tower_dir.path(), &master_id, "", Role::Master);
    let mut hook = endpoint::serve(tower_dir.path(), "input.sock");

    let mut tower = Node::new(tower_dir.path(), Some(hook.path.clone()));
    let on = tower.turn_on(0, "tower").await;
    assert_eq!(on.driven_by, None, "nobody drives it yet");
    let address = on.listen_address.unwrap();
    let mut link = master_connects(&master, &address)
        .await
        .accept()
        .await
        .unwrap();
    next_control(&mut link).await;
    tower.until(|s| linked(s, &master_id)).await;
    let sender = link.sender();

    // Motion before any crossing drives nothing: the master's pointer is not here.
    sender.input(&batch(1)).await.unwrap();
    assert_eq!(hook.next().await, Some(batch(1)));
    assert_eq!(tower.state().await.driven_by, None);

    // Its Enter, as it reaches the hook: driven by that master.
    let crossing = encode(&[
        InputEvent::Enter { x: 0, y: 0 },
        InputEvent::Motion { x: 1, y: 0 },
    ]);
    sender.input(&crossing).await.unwrap();
    assert_eq!(hook.next().await, Some(crossing.clone()));
    tower
        .until(|s| s.driven_by.as_deref() == Some(master_id.as_str()))
        .await;
    // The master's own entry is not "driven": that is the other way round.
    assert_eq!(tower.state().await.driven, None);

    // Its Leave: nobody drives it any more.
    let home = encode(&[InputEvent::Motion { x: 2, y: 0 }, InputEvent::Leave]);
    sender.input(&home).await.unwrap();
    assert_eq!(hook.next().await, Some(home));
    tower.until(|s| s.driven_by.is_none()).await;

    // Over here again, then the link ends without a Leave: the end of the stream is one.
    sender.input(&crossing).await.unwrap();
    assert_eq!(hook.next().await, Some(crossing));
    tower.until(|s| s.driven_by.is_some()).await;
    sender.close("done").await;
    tower.until(|s| s.driven_by.is_none()).await;
}

/// Links a test master `identity` to the node listening at `address`, once it said
/// its screens.
#[cfg(unix)]
async fn master_linked(identity: &Identity, address: &str) -> Link {
    let mut link = master_connects(identity, address)
        .await
        .accept()
        .await
        .unwrap();
    next_control(&mut link).await;
    link
}

#[cfg(unix)]
#[tokio::test]
async fn a_master_that_goes_ends_the_session_of_the_one_driving_and_both_states_say_so() {
    // Two masters share the remote's one connection to its hook. The one that goes
    // closes it, so that the hook releases what it left pressed: the session of the
    // one driving ends with it, and the state must not go on saying it drives.
    use lbm_ipc::input::{encode, InputEvent};
    let alpha_dir = tempfile::tempdir().unwrap();
    let alpha = Identity::load_or_create(alpha_dir.path()).unwrap();
    let beta_dir = tempfile::tempdir().unwrap();
    let beta = Identity::load_or_create(beta_dir.path()).unwrap();
    let alpha_id = alpha.fingerprint().to_owned();
    let tower_dir = tempfile::tempdir().unwrap();
    pair(tower_dir.path(), &alpha_id, "", Role::Master);
    pair(tower_dir.path(), beta.fingerprint(), "", Role::Master);
    let mut hook = endpoint::serve(tower_dir.path(), "input.sock");
    let mut tower = Node::new(tower_dir.path(), Some(hook.path.clone()));
    let address = tower.turn_on(0, "tower").await.listen_address.unwrap();
    let mut alpha_link = master_linked(&alpha, &address).await;
    let beta_link = master_linked(&beta, &address).await;
    let enter = encode(&[InputEvent::Enter { x: 0, y: 0 }]);
    let leave = encode(&[InputEvent::Leave]);

    // Beta crosses in and goes home again; alpha crosses in and drives.
    let beta_sender = beta_link.sender();
    beta_sender.input(&enter).await.unwrap();
    beta_sender.input(&leave).await.unwrap();
    assert_eq!(hook.next().await, Some(enter.clone()));
    assert_eq!(hook.next().await, Some(leave));
    alpha_link.sender().input(&enter).await.unwrap();
    assert_eq!(hook.next().await, Some(enter));
    tower
        .until(|s| s.driven_by.as_deref() == Some(alpha_id.as_str()))
        .await;

    // Beta goes: the hook's connection ends, alpha's session with it.
    beta_sender.close("done").await;
    assert_eq!(hook.next().await, None);
    tower.until(|s| s.driven_by.is_none()).await;
    // Alpha is told, and brings its pointer home.
    assert_eq!(next_control(&mut alpha_link).await, Control::InputReset);
}

#[cfg(unix)]
#[tokio::test]
async fn a_master_that_left_cleanly_and_goes_releases_the_remotes_hook() {
    use lbm_ipc::input::{encode, InputEvent};
    let master_dir = tempfile::tempdir().unwrap();
    let master = Identity::load_or_create(master_dir.path()).unwrap();
    let tower_dir = tempfile::tempdir().unwrap();
    pair(tower_dir.path(), master.fingerprint(), "", Role::Master);
    let mut hook = endpoint::serve(tower_dir.path(), "input.sock");

    let mut tower = Node::new(tower_dir.path(), Some(hook.path.clone()));
    let address = tower.turn_on(0, "tower").await.listen_address.unwrap();
    let mut link = master_connects(&master, &address)
        .await
        .accept()
        .await
        .unwrap();
    next_control(&mut link).await;

    // A crossing, and the pointer back home: no session is under way any more.
    let sender = link.sender();
    let crossing = encode(&[InputEvent::Enter { x: 0, y: 0 }]);
    let home = encode(&[InputEvent::Leave]);
    sender.input(&crossing).await.unwrap();
    sender.input(&home).await.unwrap();
    assert_eq!(hook.next().await, Some(crossing));
    assert_eq!(hook.next().await, Some(home));

    // The master goes: the hook's connection ends with it, the hook has no client left.
    sender.close("done").await;
    assert_eq!(hook.next().await, None);
}

#[cfg(unix)]
#[tokio::test]
async fn the_master_hooks_stream_crosses_to_the_remote_hook_until_the_link_ends() {
    let (desk_dir, tower_dir) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let desk_id = Identity::load_or_create(desk_dir.path())
        .unwrap()
        .fingerprint()
        .to_owned();
    let tower_id = Identity::load_or_create(tower_dir.path())
        .unwrap()
        .fingerprint()
        .to_owned();
    let tower_port = free_port();
    pair(tower_dir.path(), &desk_id, "", Role::Master);
    pair(
        desk_dir.path(),
        &tower_id,
        &format!("127.0.0.1:{tower_port}"),
        Role::Remote,
    );
    let mut desk_hook = endpoint::serve(desk_dir.path(), "desk-input.sock");
    let mut tower_hook = endpoint::serve(tower_dir.path(), "tower-input.sock");

    let mut tower = Node::new(tower_dir.path(), Some(tower_hook.path.clone()));
    tower.turn_on(tower_port, "tower").await;
    let mut desk = Node::new(desk_dir.path(), Some(desk_hook.path.clone()));
    desk.turn_on(0, "desk").await;
    desk.until(|s| linked(s, &tower_id)).await;
    tower.until(|s| linked(s, &desk_id)).await;
    desk.hook_ready_for(&tower_id).await;

    // What the desk's hook captures goes to the tower's hook, in order. The desk's
    // agent connects to its hook's endpoint once a remote is linked; batches written
    // before that would have nobody to read them.
    tokio::time::sleep(Duration::from_millis(200)).await;
    for x in 1..=5 {
        desk_hook.to_client.send(batch(x)).unwrap();
    }
    for x in 1..=5 {
        assert_eq!(tower_hook.next().await, Some(batch(x)));
    }

    // The desk turns the mode off: the link ends, and with it both hooks' streams.
    desk.ask(PeerRequest::Set {
        enabled: false,
        port: None,
        name: None,
        pointer_lock_shortcut: None,
        clipboard: Default::default(),
    })
    .await
    .unwrap();
    assert_eq!(tower_hook.next().await, None);
    assert_eq!(desk_hook.next().await, None);
    tower.until(|s| !linked(s, &desk_id)).await;
}

/// The next `SelectInput` the manager passes on to its agent, if one comes `within`.
async fn select_input(node: &mut Node, within: Duration) -> Option<(String, String, u16)> {
    tokio::time::timeout(within, async {
        loop {
            if let Notice::SelectInput {
                from,
                display,
                value,
                ..
            } = node.notices.recv().await.expect("notices")
            {
                return (from, display, value);
            }
        }
    })
    .await
    .ok()
}

#[tokio::test]
async fn only_this_machines_master_may_switch_its_monitors() {
    // The tower's master asks for an input: the tower's agent hears of it.
    let master_dir = tempfile::tempdir().unwrap();
    let master = Identity::load_or_create(master_dir.path()).unwrap();
    let tower_dir = tempfile::tempdir().unwrap();
    pair(tower_dir.path(), master.fingerprint(), "", Role::Master);
    let mut tower = Node::new(tower_dir.path(), None);
    let address = tower.turn_on(0, "tower").await.listen_address.unwrap();
    let link = master_connects(&master, &address)
        .await
        .accept()
        .await
        .unwrap();
    link.sender()
        .control(&Control::SelectInput {
            display: "DEL4065_ABC123".into(),
            value: 0x11,
            request: Some(4),
        })
        .await
        .unwrap();
    assert_eq!(
        select_input(&mut tower, PATIENCE).await,
        Some((
            master.fingerprint().to_owned(),
            "DEL4065_ABC123".to_owned(),
            0x11
        ))
    );

    // The desk's remote asks the same of the desk: a remote has no say over its
    // master's monitors, and the desk's agent hears nothing.
    let laptop_dir = tempfile::tempdir().unwrap();
    let laptop = Identity::load_or_create(laptop_dir.path()).unwrap();
    let mut listener = link::listen(
        "127.0.0.1:0",
        &laptop,
        Hello::new("laptop", "0", Role::Remote),
        link::Options::default(),
    )
    .await
    .unwrap();
    let desk_dir = tempfile::tempdir().unwrap();
    pair(
        desk_dir.path(),
        laptop.fingerprint(),
        &listener.local_addr().to_string(),
        Role::Remote,
    );
    let mut desk = Node::new(desk_dir.path(), None);
    desk.turn_on(0, "desk").await;
    let mut laptop_link = tokio::time::timeout(PATIENCE, listener.next())
        .await
        .expect("the desk calls its remote back")
        .unwrap()
        .unwrap()
        .accept()
        .await
        .unwrap();
    desk.until(|s| linked(s, laptop.fingerprint())).await;
    laptop_link
        .sender()
        .control(&Control::SelectInput {
            display: "DP-1".into(),
            value: 0x0f,
            request: None,
        })
        .await
        .unwrap();
    assert_eq!(
        select_input(&mut desk, Duration::from_millis(500)).await,
        None
    );

    // The other way round is the desk's to ask: the laptop is told.
    assert!(desk
        .service
        .select_input(laptop.fingerprint(), "HDMI-1", 0x11, Some(9)));
    assert_eq!(
        next_control(&mut laptop_link).await,
        Control::SelectInput {
            display: "HDMI-1".into(),
            value: 0x11,
            request: Some(9),
        }
    );
    drop(link);
}

#[tokio::test]
async fn a_switch_asked_of_a_remote_is_answered_back_to_its_master() {
    // The tower is the remote: what its agent says of a switch reaches its master.
    let master_dir = tempfile::tempdir().unwrap();
    let master = Identity::load_or_create(master_dir.path()).unwrap();
    let tower_dir = tempfile::tempdir().unwrap();
    pair(tower_dir.path(), master.fingerprint(), "", Role::Master);
    let mut tower = Node::new(tower_dir.path(), None);
    let address = tower.turn_on(0, "tower").await.listen_address.unwrap();
    let mut link = master_connects(&master, &address)
        .await
        .accept()
        .await
        .unwrap();
    tower.until(|s| linked(s, master.fingerprint())).await;
    assert!(tower.service.input_switched(
        master.fingerprint(),
        "DP-1",
        0x11,
        Some(4),
        Some("DDC/CI write failed".into())
    ));
    loop {
        match next_control(&mut link).await {
            Control::InputSwitched {
                display,
                value,
                request,
                error,
            } => {
                assert_eq!((display.as_str(), value, request), ("DP-1", 0x11, Some(4)));
                assert_eq!(error.as_deref(), Some("DDC/CI write failed"));
                break;
            }
            // The screens it sends at link-up.
            Control::Displays { .. } | Control::Wallpapers { .. } => {}
            other => panic!("unexpected {other:?}"),
        }
    }

    // The desk is the master: its remote's answer reaches the desk's agent.
    let laptop_dir = tempfile::tempdir().unwrap();
    let laptop = Identity::load_or_create(laptop_dir.path()).unwrap();
    let mut listener = link::listen(
        "127.0.0.1:0",
        &laptop,
        Hello::new("laptop", "0", Role::Remote),
        link::Options::default(),
    )
    .await
    .unwrap();
    let desk_dir = tempfile::tempdir().unwrap();
    pair(
        desk_dir.path(),
        laptop.fingerprint(),
        &listener.local_addr().to_string(),
        Role::Remote,
    );
    let mut desk = Node::new(desk_dir.path(), None);
    desk.turn_on(0, "desk").await;
    let laptop_link = tokio::time::timeout(PATIENCE, listener.next())
        .await
        .expect("the desk calls its remote back")
        .unwrap()
        .unwrap()
        .accept()
        .await
        .unwrap();
    desk.until(|s| linked(s, laptop.fingerprint())).await;
    laptop_link
        .sender()
        .control(&Control::InputSwitched {
            display: "HDMI-1".into(),
            value: 0x0f,
            request: Some(2),
            error: None,
        })
        .await
        .unwrap();
    let heard = tokio::time::timeout(PATIENCE, async {
        loop {
            if let Notice::InputSwitched {
                from,
                display,
                value,
                request,
                error,
            } = desk.notices.recv().await.expect("notices")
            {
                return (from, display, value, request, error);
            }
        }
    })
    .await
    .expect("the answer, in time");
    assert_eq!(
        heard,
        (
            laptop.fingerprint().to_owned(),
            "HDMI-1".to_owned(),
            0x0f,
            Some(2),
            None
        )
    );
    drop(link);
}

//==================//
// Link problems    //
//==================//

fn problem(state: &PeersState, machine_id: &str) -> Option<LinkProblem> {
    peer(state, machine_id).and_then(|p| p.problem.clone())
}

/// A remote that is paired with the desk refuses it, then trusts it again: the refusal
/// and its reason are in the desk's state until the link is up.
#[tokio::test]
async fn a_remote_that_refuses_the_link_is_said_until_it_links() {
    let laptop_dir = tempfile::tempdir().unwrap();
    let laptop = Identity::load_or_create(laptop_dir.path()).unwrap();
    let mut listener = link::listen(
        "127.0.0.1:0",
        &laptop,
        Hello::new("laptop", "0", Role::Remote),
        link::Options::default(),
    )
    .await
    .unwrap();
    let desk_dir = tempfile::tempdir().unwrap();
    pair(
        desk_dir.path(),
        laptop.fingerprint(),
        &listener.local_addr().to_string(),
        Role::Remote,
    );
    let mut desk = Node::new(desk_dir.path(), None);
    desk.turn_on(0, "desk").await;
    let first = tokio::time::timeout(PATIENCE, listener.next())
        .await
        .expect("the desk calls its remote back")
        .unwrap()
        .unwrap();
    tokio::spawn(first.refuse("this machine no longer trusts the desk"));
    let state = desk
        .until(|s| problem(s, laptop.fingerprint()).is_some())
        .await;
    assert_eq!(
        problem(&state, laptop.fingerprint()),
        Some(LinkProblem::new(
            ProblemKind::Refused,
            "this machine no longer trusts the desk"
        ))
    );
    assert!(!linked(&state, laptop.fingerprint()));

    // The desk calls back; this time the laptop trusts it.
    let _link = tokio::time::timeout(PATIENCE, listener.next())
        .await
        .expect("the desk calls again")
        .unwrap()
        .unwrap()
        .accept()
        .await
        .unwrap();
    let state = desk.until(|s| linked(s, laptop.fingerprint())).await;
    assert_eq!(problem(&state, laptop.fingerprint()), None);
}

/// A paired remote nobody answers for: a network problem, gone once it answers.
#[tokio::test]
async fn a_remote_out_of_reach_is_said_until_it_answers() {
    let laptop_dir = tempfile::tempdir().unwrap();
    let laptop = Identity::load_or_create(laptop_dir.path()).unwrap();
    let port = free_port();
    let desk_dir = tempfile::tempdir().unwrap();
    pair(
        desk_dir.path(),
        laptop.fingerprint(),
        &format!("127.0.0.1:{port}"),
        Role::Remote,
    );
    let mut desk = Node::new(desk_dir.path(), None);
    desk.turn_on(0, "desk").await;
    let state = desk
        .until(|s| problem(s, laptop.fingerprint()).is_some())
        .await;
    let said = problem(&state, laptop.fingerprint()).unwrap();
    assert_eq!(said.kind, ProblemKind::Network);
    assert!(!said.detail.is_empty());

    // The laptop comes up on that port: the desk's next call links, and the problem
    // goes.
    let mut listener = link::listen(
        ("127.0.0.1", port),
        &laptop,
        Hello::new("laptop", "0", Role::Remote),
        link::Options::default(),
    )
    .await
    .unwrap();
    let _link = tokio::time::timeout(PATIENCE, listener.next())
        .await
        .expect("the desk calls again")
        .unwrap()
        .unwrap()
        .accept()
        .await
        .unwrap();
    let state = desk.until(|s| linked(s, laptop.fingerprint())).await;
    assert_eq!(problem(&state, laptop.fingerprint()), None);

    // Forgotten, it is said nothing of, whatever happens to the link.
    desk.ask(PeerRequest::Forget(laptop.fingerprint().to_owned()))
        .await
        .unwrap();
    let state = desk
        .until(|s| peer(s, laptop.fingerprint()).is_none())
        .await;
    assert!(state.peers.iter().all(|p| p.problem.is_none()));
}

/// Another machine answers at a paired remote's address: its fingerprint is said.
#[tokio::test]
async fn another_machine_at_a_paired_address_is_said_with_its_fingerprint() {
    let laptop_dir = tempfile::tempdir().unwrap();
    let laptop = Identity::load_or_create(laptop_dir.path()).unwrap();
    let stranger_dir = tempfile::tempdir().unwrap();
    let stranger = Identity::load_or_create(stranger_dir.path()).unwrap();
    let mut listener = link::listen(
        "127.0.0.1:0",
        &stranger,
        Hello::new("stranger", "0", Role::Remote),
        link::Options::default(),
    )
    .await
    .unwrap();
    let desk_dir = tempfile::tempdir().unwrap();
    pair(
        desk_dir.path(),
        laptop.fingerprint(),
        &listener.local_addr().to_string(),
        Role::Remote,
    );
    let mut desk = Node::new(desk_dir.path(), None);
    desk.turn_on(0, "desk").await;
    // Held, so that the desk's refusal is what ends it.
    let _held = tokio::time::timeout(PATIENCE, listener.next()).await;
    let state = desk
        .until(|s| problem(s, laptop.fingerprint()).is_some())
        .await;
    assert_eq!(
        problem(&state, laptop.fingerprint()),
        Some(LinkProblem::new(
            ProblemKind::UnknownFingerprint,
            stranger.fingerprint()
        ))
    );
}

//==================//
// One session      //
//==================//

#[cfg(unix)]
const CTRL: u16 = 29;
#[cfg(unix)]
const BTN_LEFT: u16 = 0x110;
#[cfg(unix)]
const KEY_A: u16 = 30;

/// What a remote's hook holds pressed after the stream it was sent, counted the way its
/// session counts (`lbm-hook/src/remote/mod.rs`): nothing before an `Enter`, and
/// everything released on a `Leave` or at the end of the connection.
#[cfg(unix)]
#[derive(Default)]
struct Held {
    driven: bool,
    keys: std::collections::BTreeSet<u16>,
    buttons: std::collections::BTreeSet<u16>,
}

#[cfg(unix)]
impl Held {
    fn apply(&mut self, batch: &[u8]) {
        use lbm_ipc::input::{decode, InputEvent};
        for event in decode(batch).unwrap() {
            match event {
                InputEvent::Enter { .. } => self.driven = true,
                InputEvent::Leave => self.end(),
                _ if !self.driven => {}
                InputEvent::Key { code, pressed } => {
                    if pressed {
                        self.keys.insert(code);
                    } else {
                        self.keys.remove(&code);
                    }
                }
                InputEvent::Button { code, pressed } => {
                    if pressed {
                        self.buttons.insert(code);
                    } else {
                        self.buttons.remove(&code);
                    }
                }
                _ => {}
            }
        }
    }

    fn end(&mut self) {
        self.driven = false;
        self.keys.clear();
        self.buttons.clear();
    }
}

/// A batch as large as a frame carries: few of them fill any buffer on the way.
#[cfg(unix)]
fn flood() -> Vec<u8> {
    use lbm_ipc::input::{encode, InputEvent};
    let events: Vec<InputEvent> = (0..7000)
        .map(|i| InputEvent::Motion { x: i, y: i })
        .collect();
    encode(&events)
}

/// The events of the next input batch on `link`, skipping control messages.
#[cfg(unix)]
async fn next_input(link: &mut Link) -> Vec<lbm_ipc::input::InputEvent> {
    tokio::time::timeout(PATIENCE, async {
        loop {
            match link.next().await {
                Some(Event::Input(batch)) => return lbm_ipc::input::decode(&batch).unwrap(),
                Some(Event::Control(_)) => {}
                other => panic!("the link ended: {other:?}"),
            }
        }
    })
    .await
    .expect("an input batch in time")
}

/// Nothing comes on `link` for a while.
#[cfg(unix)]
async fn silent(link: &mut Link) -> bool {
    tokio::time::timeout(Duration::from_millis(300), next_input(link))
        .await
        .is_err()
}

/// A remote of a test's own: a listener under a fresh identity.
#[cfg(unix)]
async fn a_remote(
    name: &str,
    options: link::Options,
) -> (tempfile::TempDir, Identity, link::Listener) {
    a_remote_able(name, options, &[]).await
}

/// [`a_remote`], saying it can do `capabilities`.
#[cfg(unix)]
async fn a_remote_able(
    name: &str,
    options: link::Options,
    capabilities: &[&str],
) -> (tempfile::TempDir, Identity, link::Listener) {
    let directory = tempfile::tempdir().unwrap();
    let identity = Identity::load_or_create(directory.path()).unwrap();
    let mut hello = Hello::new(name, "0", Role::Remote);
    hello.capabilities = capabilities.iter().map(|c| (*c).to_owned()).collect();
    let listener = link::listen("127.0.0.1:0", &identity, hello, options)
        .await
        .unwrap();
    (directory, identity, listener)
}

/// The link a master made to `listener`, accepted.
#[cfg(unix)]
async fn linked_from(listener: &mut link::Listener) -> Link {
    tokio::time::timeout(PATIENCE, listener.next())
        .await
        .expect("the master connects")
        .unwrap()
        .unwrap()
        .accept()
        .await
        .unwrap()
}

#[cfg(unix)]
#[tokio::test]
async fn a_batch_the_link_has_no_room_for_ends_the_session_and_the_remote_is_told_leave() {
    use lbm_ipc::input::{encode, InputEvent};
    // A link that stalls on purpose here, and must not be taken for lost meanwhile.
    let stalling = link::Options {
        keepalive: Duration::from_secs(60),
        queue: 1,
        ..link::Options::default()
    };
    let (_laptop_dir, laptop, mut listener) = a_remote("laptop", stalling.clone()).await;
    let desk_dir = tempfile::tempdir().unwrap();
    pair(
        desk_dir.path(),
        laptop.fingerprint(),
        &listener.local_addr().to_string(),
        Role::Remote,
    );
    let mut desk_hook = endpoint::serve(desk_dir.path(), "desk-input.sock");
    let mut desk = Node::with_setup(
        desk_dir.path(),
        setup_with(Some(desk_hook.path.clone()), stalling),
    );
    desk.turn_on(0, "desk").await;
    let mut laptop_link = linked_from(&mut listener).await;
    desk.until(|s| linked(s, laptop.fingerprint())).await;
    desk.hook_ready_for(laptop.fingerprint()).await;
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Ctrl goes down on the laptop, which then reads nothing while the hook's stream
    // outgrows every buffer on the way — Ctrl's release last.
    desk_hook
        .to_client
        .send(encode(&[
            InputEvent::Enter { x: 0, y: 0 },
            InputEvent::Key {
                code: CTRL,
                pressed: true,
            },
        ]))
        .unwrap();
    for _ in 0..400 {
        desk_hook.to_client.send(flood()).unwrap();
    }
    desk_hook
        .to_client
        .send(encode(&[InputEvent::Key {
            code: CTRL,
            pressed: false,
        }]))
        .unwrap();

    // A batch found no room: the agent lets go of the hook, which comes home.
    assert_eq!(desk_hook.next().await, None);

    // The laptop reads again: whatever was lost on the way, the session ends with a
    // `Leave`, and its hook holds nothing.
    let mut held = Held::default();
    let mut pressed = false;
    tokio::time::timeout(PATIENCE, async {
        loop {
            match laptop_link.next().await {
                Some(Event::Input(batch)) => {
                    held.apply(&batch);
                    pressed |= held.keys.contains(&CTRL);
                    if lbm_ipc::input::decode(&batch).unwrap() == [InputEvent::Leave] {
                        return;
                    }
                }
                Some(Event::Control(_)) => {}
                other => panic!("the link ended: {other:?}"),
            }
        }
    })
    .await
    .expect("a Leave in time");
    assert!(pressed, "Ctrl reached the laptop");
    assert!(held.keys.is_empty(), "{:?} left pressed", held.keys);
}

#[cfg(unix)]
#[tokio::test]
async fn a_batch_the_remotes_hook_has_no_room_for_ends_its_stream_so_the_hook_releases_ctrl() {
    use lbm_ipc::input::{encode, InputEvent};
    let master_dir = tempfile::tempdir().unwrap();
    let master = Identity::load_or_create(master_dir.path()).unwrap();
    let tower_dir = tempfile::tempdir().unwrap();
    pair(tower_dir.path(), master.fingerprint(), "", Role::Master);
    // A hook busy for a while (less than the agent's patience with one write): the
    // agent's queue to it fills.
    let mut hook =
        endpoint::serve_after(tower_dir.path(), "input.sock", Duration::from_millis(700));
    let mut tower = Node::new(tower_dir.path(), Some(hook.path.clone()));
    let address = tower.turn_on(0, "tower").await.listen_address.unwrap();
    let mut link = master_connects(&master, &address)
        .await
        .accept()
        .await
        .unwrap();
    next_control(&mut link).await;

    let sender = link.sender();
    sender
        .input(&encode(&[
            InputEvent::Enter { x: 0, y: 0 },
            InputEvent::Key {
                code: CTRL,
                pressed: true,
            },
        ]))
        .await
        .unwrap();
    for _ in 0..120 {
        sender.input(&flood()).await.unwrap();
    }
    sender
        .input(&encode(&[InputEvent::Key {
            code: CTRL,
            pressed: false,
        }]))
        .await
        .unwrap();

    // Ctrl is released on the tower: by its release if that got through, else by the end
    // of the connection.
    let mut held = Held::default();
    let mut pressed = false;
    loop {
        match hook.next().await {
            Some(batch) => {
                held.apply(&batch);
                pressed |= held.keys.contains(&CTRL);
            }
            None => {
                held.end();
                break;
            }
        }
        if pressed && !held.keys.contains(&CTRL) {
            break;
        }
    }
    assert!(pressed, "Ctrl reached the tower's hook");
    assert!(held.keys.is_empty(), "{:?} left pressed", held.keys);

    // The master is told the session ended: it would otherwise go on sending into a
    // session the tower no longer has, its pointer away and its keyboards held.
    tokio::time::timeout(PATIENCE, async {
        while next_control(&mut link).await != Control::InputReset {}
    })
    .await
    .expect("the master is told the session ended");

    // Once what was queued has drained, the master's next crossing works: a fresh
    // `Enter`, then a key, reach the tower's hook and are taken.
    while tokio::time::timeout(Duration::from_millis(300), hook.received.recv())
        .await
        .is_ok()
    {}
    sender
        .input(&encode(&[
            InputEvent::Enter { x: 5, y: 5 },
            InputEvent::Key {
                code: KEY_A,
                pressed: true,
            },
        ]))
        .await
        .unwrap();
    let mut after = Held::default();
    while !after.keys.contains(&KEY_A) {
        match hook.next().await {
            Some(batch) => after.apply(&batch),
            None => after.end(),
        }
    }
}

#[cfg(unix)]
#[tokio::test]
async fn a_remote_that_ended_the_session_brings_the_masters_pointer_home_for_a_fresh_crossing() {
    use lbm_ipc::input::{encode, read_batch, write_batch, InputEvent};
    let (_laptop_dir, laptop, mut listener) = a_remote("laptop", link::Options::default()).await;
    let desk_dir = tempfile::tempdir().unwrap();
    pair(
        desk_dir.path(),
        laptop.fingerprint(),
        &listener.local_addr().to_string(),
        Role::Remote,
    );
    let hook_path = desk_dir.path().join("desk-input.sock");
    let hook_listener = tokio::net::UnixListener::bind(&hook_path).unwrap();
    let mut desk = Node::new(
        desk_dir.path(),
        Some(hook_path.to_string_lossy().into_owned()),
    );
    desk.turn_on(0, "desk").await;
    let mut laptop_link = linked_from(&mut listener).await;
    desk.hook_ready_for(laptop.fingerprint()).await;
    let accept = || async {
        tokio::time::timeout(PATIENCE, hook_listener.accept())
            .await
            .expect("the agent connects to the hook")
            .unwrap()
            .0
    };

    let mut hook = accept().await;
    let crossing = [
        InputEvent::Enter { x: 10, y: 10 },
        InputEvent::Key {
            code: CTRL,
            pressed: true,
        },
    ];
    write_batch(&mut hook, &encode(&crossing)).await.unwrap();
    assert_eq!(next_input(&mut laptop_link).await, crossing);

    // The laptop's hook could not keep up: the laptop ended the session and says so.
    laptop_link
        .sender()
        .control(&Control::InputReset)
        .await
        .unwrap();

    // The desk's hook loses its client (it comes home and lets go of the keyboards), the
    // laptop is told `Leave`, and a new connection carries the next crossing.
    assert!(
        tokio::time::timeout(PATIENCE, read_batch(&mut hook))
            .await
            .expect("the connection ends")
            .is_err(),
        "the hook's connection is closed"
    );
    assert_eq!(next_input(&mut laptop_link).await, [InputEvent::Leave]);
    let mut successor = accept().await;
    let again = [
        InputEvent::Enter { x: 20, y: 20 },
        InputEvent::Key {
            code: KEY_A,
            pressed: true,
        },
    ];
    write_batch(&mut successor, &encode(&again)).await.unwrap();
    assert_eq!(next_input(&mut laptop_link).await, again);
}

#[cfg(unix)]
#[tokio::test]
async fn when_the_master_hook_goes_the_remote_is_told_leave_before_anything_from_its_successor() {
    use lbm_ipc::input::{encode, write_batch, InputEvent};
    let (_laptop_dir, laptop, mut listener) = a_remote("laptop", link::Options::default()).await;
    let desk_dir = tempfile::tempdir().unwrap();
    pair(
        desk_dir.path(),
        laptop.fingerprint(),
        &listener.local_addr().to_string(),
        Role::Remote,
    );
    let hook_path = desk_dir.path().join("desk-input.sock");
    let hook_listener = tokio::net::UnixListener::bind(&hook_path).unwrap();
    let mut desk = Node::new(
        desk_dir.path(),
        Some(hook_path.to_string_lossy().into_owned()),
    );
    desk.turn_on(0, "desk").await;
    let mut laptop_link = linked_from(&mut listener).await;
    desk.hook_ready_for(laptop.fingerprint()).await;
    let accept = || async {
        tokio::time::timeout(PATIENCE, hook_listener.accept())
            .await
            .expect("the agent connects to the hook")
            .unwrap()
            .0
    };

    // The pointer crosses to the laptop and a drag starts there.
    let mut hook = accept().await;
    let press = [
        InputEvent::Enter { x: 10, y: 10 },
        InputEvent::Button {
            code: BTN_LEFT,
            pressed: true,
        },
    ];
    write_batch(&mut hook, &encode(&press)).await.unwrap();
    assert_eq!(next_input(&mut laptop_link).await, press);

    // The hook exits with the button down; the next one starts and crosses at once.
    drop(hook);
    let mut successor = accept().await;
    write_batch(
        &mut successor,
        &encode(&[InputEvent::Enter { x: 20, y: 20 }]),
    )
    .await
    .unwrap();

    // The laptop releases the drag before it hears anything of the new hook.
    assert_eq!(next_input(&mut laptop_link).await, [InputEvent::Leave]);
    assert_eq!(
        next_input(&mut laptop_link).await,
        [InputEvent::Enter { x: 20, y: 20 }]
    );
}

#[cfg(unix)]
#[tokio::test]
async fn the_lock_keys_go_only_to_a_remote_that_reads_them() {
    use lbm_ipc::input::{encode, write_batch, InputEvent, Locks, LOCKS_CAPABILITY};
    let enter = InputEvent::Enter { x: 10, y: 10 };
    let locks = InputEvent::Locks(Locks {
        caps: Some(true),
        num: Some(true),
        scroll: None,
    });
    let older: &[InputEvent] = &[enter];
    let newer: &[InputEvent] = &[enter, locks];
    for (capabilities, expected) in [(&[][..], older), (&[LOCKS_CAPABILITY][..], newer)] {
        let (_laptop_dir, laptop, mut listener) =
            a_remote_able("laptop", link::Options::default(), capabilities).await;
        let desk_dir = tempfile::tempdir().unwrap();
        pair(
            desk_dir.path(),
            laptop.fingerprint(),
            &listener.local_addr().to_string(),
            Role::Remote,
        );
        let hook_path = desk_dir.path().join("desk-input.sock");
        let hook_listener = tokio::net::UnixListener::bind(&hook_path).unwrap();
        let mut desk = Node::new(
            desk_dir.path(),
            Some(hook_path.to_string_lossy().into_owned()),
        );
        desk.turn_on(0, "desk").await;
        let mut laptop_link = linked_from(&mut listener).await;
        assert!(
            laptop_link
                .peer()
                .hello
                .capabilities
                .iter()
                .any(|c| c == LOCKS_CAPABILITY),
            "this agent says its hook reads them"
        );
        desk.hook_ready_for(laptop.fingerprint()).await;
        let (mut hook, _) = tokio::time::timeout(PATIENCE, hook_listener.accept())
            .await
            .expect("the agent connects to the hook")
            .unwrap();
        write_batch(&mut hook, &encode(&[enter, locks]))
            .await
            .unwrap();
        assert_eq!(next_input(&mut laptop_link).await, expected);
    }
}

#[cfg(unix)]
#[tokio::test]
async fn the_state_says_which_machine_has_the_mouse_and_keyboard() {
    use lbm_ipc::input::{encode, write_batch, InputEvent};
    let (_laptop_dir, laptop, mut listener) = a_remote("laptop", link::Options::default()).await;
    let desk_dir = tempfile::tempdir().unwrap();
    pair(
        desk_dir.path(),
        laptop.fingerprint(),
        &listener.local_addr().to_string(),
        Role::Remote,
    );
    let hook_path = desk_dir.path().join("desk-input.sock");
    let hook_listener = tokio::net::UnixListener::bind(&hook_path).unwrap();
    let mut desk = Node::new(
        desk_dir.path(),
        Some(hook_path.to_string_lossy().into_owned()),
    );
    let on = desk.turn_on(0, "desk").await;
    assert_eq!(on.driven, None, "the pointer starts here");
    let mut laptop_link = linked_from(&mut listener).await;
    desk.hook_ready_for(laptop.fingerprint()).await;
    let accept = || async {
        tokio::time::timeout(PATIENCE, hook_listener.accept())
            .await
            .expect("the agent connects to the hook")
            .unwrap()
            .0
    };
    let laptop_id = laptop.fingerprint().to_owned();

    // Motion alone is not a crossing; the Enter is.
    let mut hook = accept().await;
    let crossing = [
        InputEvent::Enter { x: 10, y: 10 },
        InputEvent::Motion { x: 11, y: 10 },
    ];
    write_batch(&mut hook, &encode(&crossing)).await.unwrap();
    assert_eq!(next_input(&mut laptop_link).await, crossing);
    desk.until(|s| s.driven.as_deref() == Some(laptop_id.as_str()))
        .await;

    // Home again by the stream's own Leave.
    write_batch(&mut hook, &encode(&[InputEvent::Leave]))
        .await
        .unwrap();
    assert_eq!(next_input(&mut laptop_link).await, [InputEvent::Leave]);
    desk.until(|s| s.driven.is_none()).await;

    // Over there again, then the hook goes: its end brings the pointer home too.
    write_batch(&mut hook, &encode(&[InputEvent::Enter { x: 5, y: 5 }]))
        .await
        .unwrap();
    assert_eq!(
        next_input(&mut laptop_link).await,
        [InputEvent::Enter { x: 5, y: 5 }]
    );
    desk.until(|s| s.driven.is_some()).await;
    drop(hook);
    desk.until(|s| s.driven.is_none()).await;
}

#[cfg(unix)]
#[tokio::test]
async fn with_two_remotes_linked_only_the_selected_ones_screens_are_merged_and_it_alone_gets_the_input(
) {
    use lbm_ipc::input::{encode, read_batch, write_batch, InputEvent};
    let (_a_dir, alpha, mut alpha_listener) = a_remote("alpha", link::Options::default()).await;
    let (_b_dir, beta, mut beta_listener) = a_remote("beta", link::Options::default()).await;
    let desk_dir = tempfile::tempdir().unwrap();
    pair(
        desk_dir.path(),
        alpha.fingerprint(),
        &alpha_listener.local_addr().to_string(),
        Role::Remote,
    );
    // Paired, but called back by nobody: the desk connects to it by hand, last.
    pair(desk_dir.path(), beta.fingerprint(), "", Role::Remote);
    let hook_path = desk_dir.path().join("desk-input.sock");
    let hook_listener = tokio::net::UnixListener::bind(&hook_path).unwrap();
    let mut desk = Node::new(
        desk_dir.path(),
        Some(hook_path.to_string_lossy().into_owned()),
    );
    desk.turn_on(0, "desk").await;
    let accept = || async {
        tokio::time::timeout(PATIENCE, hook_listener.accept())
            .await
            .expect("the agent connects to the hook")
            .unwrap()
            .0
    };

    let mut alpha_link = linked_from(&mut alpha_listener).await;
    alpha_link
        .sender()
        .control(&Control::Displays {
            revision: 1,
            displays: vec![display("A1", 0)],
        })
        .await
        .unwrap();
    desk.until(|s| linked(s, alpha.fingerprint())).await;
    let (connected, beta_link) = tokio::join!(
        desk.ask(PeerRequest::Connect(beta_listener.local_addr().to_string())),
        linked_from(&mut beta_listener)
    );
    connected.unwrap();
    let mut beta_link = beta_link;
    beta_link
        .sender()
        .control(&Control::Displays {
            revision: 1,
            displays: vec![display("B1", 0)],
        })
        .await
        .unwrap();
    let both = desk
        .until(|s| {
            [alpha.fingerprint(), beta.fingerprint()].iter().all(|id| {
                peer(s, id).is_some_and(|p| p.peer.connected && !p.peer.displays.is_empty())
            })
        })
        .await;

    // Alpha linked first and stays the one driven although beta linked after it.
    assert_eq!(both.selected_remote.as_deref(), Some(alpha.fingerprint()));
    let merged: Vec<String> = both
        .remote_screens()
        .into_iter()
        .map(|r| r.machine_id)
        .collect();
    assert_eq!(merged, [alpha.fingerprint().to_owned()]);
    // The hook holds zones leading to alpha alone: until it does, nothing is relayed.
    let alpha_choice = desk.hook_ready_for(alpha.fingerprint()).await;
    let mut hook = accept().await;
    let crossing = [
        InputEvent::Enter { x: 5, y: 5 },
        InputEvent::Key {
            code: CTRL,
            pressed: true,
        },
    ];
    write_batch(&mut hook, &encode(&crossing)).await.unwrap();
    assert_eq!(next_input(&mut alpha_link).await, crossing);
    assert!(silent(&mut beta_link).await, "beta hears nothing");

    // Alpha goes: beta is selected now. The hook's connection ends at once (its pointer
    // comes home)...
    drop(alpha_link);
    let now = desk
        .until(|s| s.selected_remote.as_deref() == Some(beta.fingerprint()))
        .await;
    let merged: Vec<String> = now
        .remote_screens()
        .into_iter()
        .map(|r| r.machine_id)
        .collect();
    assert_eq!(merged, [beta.fingerprint().to_owned()]);
    assert!(
        tokio::time::timeout(PATIENCE, read_batch(&mut hook))
            .await
            .expect("the old connection ends")
            .is_err(),
        "the first connection is closed"
    );

    // ...but the hook still holds alpha's zones: the rebuild for beta has not reached it.
    // No connection is made meanwhile, so the hook keeps the border to alpha's old zone a
    // wall; were one made, a crossing there would be an `Enter` for beta. A word about
    // alpha's choice, late or repeated, says nothing about this one.
    desk.service.hook_ready(Some(alpha_choice));
    let early = tokio::time::timeout(Duration::from_millis(500), hook_listener.accept()).await;
    if let Ok(Ok((mut early, _))) = early {
        let crossing_into_alpha = [
            InputEvent::Enter { x: 5, y: 5 },
            InputEvent::Button {
                code: BTN_LEFT,
                pressed: true,
            },
        ];
        let _ = write_batch(&mut early, &encode(&crossing_into_alpha)).await;
        assert!(silent(&mut beta_link).await, "beta hears nothing");
        panic!("the agent connected to the hook while it held alpha's zones");
    }
    assert!(silent(&mut beta_link).await, "beta hears nothing");

    // The hook confirmed zones leading to beta: the next connection relays to beta from
    // its first batch.
    desk.hook_ready_for(beta.fingerprint()).await;
    let mut successor = accept().await;
    let entering = [InputEvent::Enter { x: 7, y: 7 }];
    write_batch(&mut successor, &encode(&entering))
        .await
        .unwrap();
    assert_eq!(next_input(&mut beta_link).await, entering);
}

//========================//
// The remote's hook goes  //
//========================//

/// Whether an `InputReset` comes on `link` `within` this long; input and other control
/// messages are let go.
#[cfg(unix)]
async fn reset_within(link: &mut Link, within: Duration) -> bool {
    tokio::time::timeout(within, async {
        loop {
            match link.next().await {
                Some(Event::Control(Control::InputReset)) => return,
                Some(_) => {}
                None => std::future::pending().await,
            }
        }
    })
    .await
    .is_ok()
}

/// The next connection the agent makes to the hook listening at `listener`.
#[cfg(unix)]
async fn hook_client(listener: &tokio::net::UnixListener) -> tokio::net::UnixStream {
    tokio::time::timeout(PATIENCE, listener.accept())
        .await
        .expect("the agent connects to the hook")
        .unwrap()
        .0
}

#[cfg(unix)]
#[tokio::test]
async fn a_remote_hook_that_goes_in_a_session_makes_the_remote_say_input_reset_once() {
    use lbm_ipc::input::{encode, read_batch, InputEvent};
    let master_dir = tempfile::tempdir().unwrap();
    let master = Identity::load_or_create(master_dir.path()).unwrap();
    let tower_dir = tempfile::tempdir().unwrap();
    pair(tower_dir.path(), master.fingerprint(), "", Role::Master);
    let hook_path = tower_dir.path().join("input.sock");
    let hook_listener = tokio::net::UnixListener::bind(&hook_path).unwrap();
    let mut tower = Node::new(
        tower_dir.path(),
        Some(hook_path.to_string_lossy().into_owned()),
    );
    let address = tower.turn_on(0, "tower").await.listen_address.unwrap();
    let mut link = master_connects(&master, &address)
        .await
        .accept()
        .await
        .unwrap();
    next_control(&mut link).await;
    let sender = link.sender();

    // The pointer crosses and Ctrl goes down on the tower.
    let crossing = encode(&[
        InputEvent::Enter { x: 0, y: 0 },
        InputEvent::Key {
            code: CTRL,
            pressed: true,
        },
    ]);
    sender.input(&crossing).await.unwrap();
    let mut hook = hook_client(&hook_listener).await;
    assert_eq!(read_batch(&mut hook).await.unwrap(), crossing);

    // The tower's hook goes (restarted, crashed) while the link lives on: its successor
    // would ignore everything until an `Enter`, so the master is told.
    drop(hook);
    assert!(
        reset_within(&mut link, PATIENCE).await,
        "the master is told the session ended"
    );

    // Once for that session: what the master sends before it hears of it, and the hook
    // being away meanwhile, say nothing more.
    sender
        .input(&encode(&[InputEvent::Key {
            code: KEY_A,
            pressed: true,
        }]))
        .await
        .unwrap();
    assert!(
        !reset_within(&mut link, Duration::from_millis(700)).await,
        "a second InputReset for the same session"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn a_remote_hook_that_goes_out_of_a_session_makes_no_input_reset() {
    use lbm_ipc::input::{encode, read_batch, InputEvent};
    let master_dir = tempfile::tempdir().unwrap();
    let master = Identity::load_or_create(master_dir.path()).unwrap();
    let tower_dir = tempfile::tempdir().unwrap();
    pair(tower_dir.path(), master.fingerprint(), "", Role::Master);
    let hook_path = tower_dir.path().join("input.sock");
    let hook_listener = tokio::net::UnixListener::bind(&hook_path).unwrap();
    let mut tower = Node::new(
        tower_dir.path(),
        Some(hook_path.to_string_lossy().into_owned()),
    );
    let address = tower.turn_on(0, "tower").await.listen_address.unwrap();
    let mut link = master_connects(&master, &address)
        .await
        .accept()
        .await
        .unwrap();
    next_control(&mut link).await;
    let sender = link.sender();

    // A crossing, and the pointer back home: the session is over.
    let crossing = encode(&[InputEvent::Enter { x: 0, y: 0 }]);
    let home = encode(&[InputEvent::Leave]);
    sender.input(&crossing).await.unwrap();
    sender.input(&home).await.unwrap();
    let mut hook = hook_client(&hook_listener).await;
    assert_eq!(read_batch(&mut hook).await.unwrap(), crossing);
    assert_eq!(read_batch(&mut hook).await.unwrap(), home);

    // The hook goes now: nothing was under way, nothing is said.
    drop(hook);
    assert!(
        !reset_within(&mut link, Duration::from_millis(700)).await,
        "an InputReset out of a session"
    );

    // The next crossing reaches the hook's successor at once, although the agent lost
    // its hook less than a second ago and leaves anything else for that long.
    let again = encode(&[InputEvent::Enter { x: 5, y: 5 }]);
    sender.input(&again).await.unwrap();
    let mut successor = hook_client(&hook_listener).await;
    assert_eq!(read_batch(&mut successor).await.unwrap(), again);
    assert!(
        !reset_within(&mut link, Duration::from_millis(300)).await,
        "an InputReset out of a session"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn when_the_remotes_hook_goes_in_a_session_the_masters_pointer_comes_home_for_a_fresh_crossing(
) {
    use lbm_ipc::input::{decode, encode, read_batch, write_batch, InputEvent};
    let (desk_dir, tower_dir) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let desk_id = Identity::load_or_create(desk_dir.path())
        .unwrap()
        .fingerprint()
        .to_owned();
    let tower_id = Identity::load_or_create(tower_dir.path())
        .unwrap()
        .fingerprint()
        .to_owned();
    let tower_port = free_port();
    pair(tower_dir.path(), &desk_id, "", Role::Master);
    pair(
        desk_dir.path(),
        &tower_id,
        &format!("127.0.0.1:{tower_port}"),
        Role::Remote,
    );
    let desk_path = desk_dir.path().join("desk-input.sock");
    let desk_listener = tokio::net::UnixListener::bind(&desk_path).unwrap();
    let tower_path = tower_dir.path().join("tower-input.sock");
    let tower_listener = tokio::net::UnixListener::bind(&tower_path).unwrap();

    let mut tower = Node::new(
        tower_dir.path(),
        Some(tower_path.to_string_lossy().into_owned()),
    );
    tower.turn_on(tower_port, "tower").await;
    let mut desk = Node::new(
        desk_dir.path(),
        Some(desk_path.to_string_lossy().into_owned()),
    );
    desk.turn_on(0, "desk").await;
    desk.until(|s| linked(s, &tower_id)).await;
    tower.until(|s| linked(s, &desk_id)).await;
    desk.hook_ready_for(&tower_id).await;

    // The desk's pointer crosses to the tower and Ctrl goes down there.
    let mut desk_hook = hook_client(&desk_listener).await;
    let crossing = encode(&[
        InputEvent::Enter { x: 10, y: 10 },
        InputEvent::Key {
            code: CTRL,
            pressed: true,
        },
    ]);
    write_batch(&mut desk_hook, &crossing).await.unwrap();
    let mut tower_hook = hook_client(&tower_listener).await;
    assert_eq!(read_batch(&mut tower_hook).await.unwrap(), crossing);

    // The tower's hook goes, alone: the link lives on. The desk hears `InputReset` and
    // ends the hop: its hook loses its client (the pointer comes home, the keyboards are
    // let go), and a new connection is made for the next crossing.
    drop(tower_hook);
    assert!(
        tokio::time::timeout(PATIENCE, read_batch(&mut desk_hook))
            .await
            .expect("the desk's hook loses its client")
            .is_err(),
        "the desk's hook connection is closed"
    );
    let mut desk_successor = hook_client(&desk_listener).await;

    // The next crossing, made at once, reaches the tower's new hook: an `Enter` it
    // takes, then a key. The desk's `Leave` that ended the hop may come first, or have
    // been dropped while no hook was connected.
    let again = encode(&[
        InputEvent::Enter { x: 20, y: 20 },
        InputEvent::Key {
            code: KEY_A,
            pressed: true,
        },
    ]);
    write_batch(&mut desk_successor, &again).await.unwrap();
    let mut tower_successor = hook_client(&tower_listener).await;
    let mut held = Held::default();
    tokio::time::timeout(PATIENCE, async {
        loop {
            let batch = read_batch(&mut tower_successor).await.unwrap();
            if batch == again {
                held.apply(&batch);
                return;
            }
            assert_eq!(decode(&batch).unwrap(), [InputEvent::Leave]);
        }
    })
    .await
    .expect("the next crossing reaches the tower's new hook");
    assert!(held.driven && held.keys.contains(&KEY_A));
    tower.until(|s| linked(s, &desk_id)).await;
}

#[cfg(unix)]
#[tokio::test]
async fn a_crossing_the_remotes_hook_is_not_there_for_makes_the_remote_say_input_reset() {
    use lbm_ipc::input::{encode, read_batch, InputEvent};
    let master_dir = tempfile::tempdir().unwrap();
    let master = Identity::load_or_create(master_dir.path()).unwrap();
    let tower_dir = tempfile::tempdir().unwrap();
    pair(tower_dir.path(), master.fingerprint(), "", Role::Master);
    // The tower's hook is not running yet.
    let hook_path = tower_dir.path().join("input.sock");
    let mut tower = Node::new(
        tower_dir.path(),
        Some(hook_path.to_string_lossy().into_owned()),
    );
    let address = tower.turn_on(0, "tower").await.listen_address.unwrap();
    let mut link = master_connects(&master, &address)
        .await
        .accept()
        .await
        .unwrap();
    next_control(&mut link).await;
    let sender = link.sender();

    // The pointer crosses: nothing takes it on the tower, and the master is told, once.
    let crossing = encode(&[
        InputEvent::Enter { x: 0, y: 0 },
        InputEvent::Key {
            code: CTRL,
            pressed: true,
        },
    ]);
    sender.input(&crossing).await.unwrap();
    assert!(
        reset_within(&mut link, PATIENCE).await,
        "the master is told the crossing went nowhere"
    );
    let stray = encode(&[InputEvent::Key {
        code: KEY_A,
        pressed: true,
    }]);
    sender.input(&stray).await.unwrap();
    assert!(
        !reset_within(&mut link, Duration::from_millis(700)).await,
        "a second InputReset for the same crossing"
    );

    // The master's hop ends, and the user crosses again, at once: still nobody there,
    // and the master is told again.
    let home = encode(&[InputEvent::Leave]);
    sender.input(&home).await.unwrap();
    sender.input(&crossing).await.unwrap();
    assert!(
        reset_within(&mut link, PATIENCE).await,
        "the master is told the second crossing went nowhere"
    );

    // The hook starts: the next crossing, at once, reaches it, and nothing is said.
    let hook_listener = tokio::net::UnixListener::bind(&hook_path).unwrap();
    sender.input(&home).await.unwrap();
    sender.input(&crossing).await.unwrap();
    let mut hook = hook_client(&hook_listener).await;
    assert_eq!(read_batch(&mut hook).await.unwrap(), crossing);
    assert!(
        !reset_within(&mut link, Duration::from_millis(300)).await,
        "an InputReset for a crossing the hook took"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn the_pointer_lock_shortcut_changes_without_dropping_a_link_and_the_lock_is_in_the_state() {
    let (_laptop_dir, laptop, mut listener) = a_remote("laptop", link::Options::default()).await;
    let desk_dir = tempfile::tempdir().unwrap();
    pair(
        desk_dir.path(),
        laptop.fingerprint(),
        &listener.local_addr().to_string(),
        Role::Remote,
    );
    let mut desk = Node::new(desk_dir.path(), None);
    let on = desk.turn_on(0, "desk").await;
    assert_eq!(on.pointer_lock_shortcut, "", "none unless set");
    let _link = linked_from(&mut listener).await;
    let laptop_id = laptop.fingerprint().to_owned();
    desk.until(|s| linked(s, &laptop_id)).await;

    let set = |shortcut: &str| PeerRequest::Set {
        enabled: true,
        port: None,
        name: None,
        pointer_lock_shortcut: Some(shortcut.to_owned()),
        clipboard: Default::default(),
    };
    let state: PeersState =
        serde_json::from_value(desk.ask(set(" Ctrl+Alt+L ")).await.unwrap()).unwrap();
    assert_eq!(state.pointer_lock_shortcut, "Ctrl+Alt+L");
    assert!(linked(&state, &laptop_id), "the link stayed up");
    assert!(
        tokio::time::timeout(Duration::from_millis(300), listener.next())
            .await
            .is_err(),
        "the master did not connect again"
    );
    assert_eq!(
        Settings::load(desk_dir.path())
            .pointer_lock_shortcut
            .as_deref(),
        Some("Ctrl+Alt+L")
    );

    // The hook says it locked the pointer, then let it go.
    desk.service.hook_locked(true);
    desk.until(|s| s.pointer_locked).await;
    desk.service.hook_locked(false);
    desk.until(|s| !s.pointer_locked).await;

    // Empty takes the shortcut away.
    let state: PeersState = serde_json::from_value(desk.ask(set("")).await.unwrap()).unwrap();
    assert_eq!(state.pointer_lock_shortcut, "");
    assert_eq!(Settings::load(desk_dir.path()).pointer_lock_shortcut, None);
}

#[cfg(unix)]
#[tokio::test]
async fn a_remote_strips_the_lock_keys_for_a_hook_that_has_not_said_it_reads_them() {
    use lbm_ipc::input::{encode, read_batch, InputEvent, Locks};
    let master_dir = tempfile::tempdir().unwrap();
    let master = Identity::load_or_create(master_dir.path()).unwrap();
    let tower_dir = tempfile::tempdir().unwrap();
    pair(tower_dir.path(), master.fingerprint(), "", Role::Master);
    let hook_path = tower_dir.path().join("input.sock");
    let hook_listener = tokio::net::UnixListener::bind(&hook_path).unwrap();
    let mut tower = Node::new(
        tower_dir.path(),
        Some(hook_path.to_string_lossy().into_owned()),
    );
    let address = tower.turn_on(0, "tower").await.listen_address.unwrap();
    let mut link = master_connects(&master, &address)
        .await
        .accept()
        .await
        .unwrap();
    next_control(&mut link).await;
    let sender = link.sender();
    let enter = InputEvent::Enter { x: 0, y: 0 };
    let locks = InputEvent::Locks(Locks {
        caps: Some(true),
        num: None,
        scroll: None,
    });

    // An older hook, or one that has not greeted yet: the Enter alone.
    sender.input(&encode(&[enter, locks])).await.unwrap();
    let mut hook = hook_client(&hook_listener).await;
    assert_eq!(read_batch(&mut hook).await.unwrap(), encode(&[enter]));

    // A hook that said it reads them gets them.
    tower.service.hook_reads_locks(true);
    sender
        .input(&encode(&[InputEvent::Leave, enter, locks]))
        .await
        .unwrap();
    assert_eq!(
        read_batch(&mut hook).await.unwrap(),
        encode(&[InputEvent::Leave, enter, locks])
    );
}

/// The next `InjectionBlocked` on `link` within `within`, skipping everything else.
async fn next_block(
    link: &mut Link,
    within: Duration,
) -> Option<Option<lbm_ipc::peer::InjectionBlock>> {
    tokio::time::timeout(within, async {
        loop {
            match link.next().await {
                Some(Event::Control(Control::InjectionBlocked { block })) => return block,
                Some(Event::Control(_) | Event::Input(_)) => {}
                other => panic!("the link ended: {other:?}"),
            }
        }
    })
    .await
    .ok()
}

#[tokio::test]
async fn a_remote_says_its_injection_is_blocked_only_to_a_master_that_hears_it() {
    use lbm_ipc::peer::{InjectionBlock, INJECTION_BLOCKED_CAPABILITY};
    let osk = || Some(InjectionBlock::elevated("osk.exe"));
    for hears in [false, true] {
        let master_dir = tempfile::tempdir().unwrap();
        let master = Identity::load_or_create(master_dir.path()).unwrap();
        let tower_dir = tempfile::tempdir().unwrap();
        pair(tower_dir.path(), master.fingerprint(), "", Role::Master);
        let mut tower = Node::new(tower_dir.path(), None);
        let address = tower.turn_on(0, "tower").await.listen_address.unwrap();
        let mut hello = Hello::new("test master", "0", Role::Master);
        if hears {
            hello.capabilities = vec![INJECTION_BLOCKED_CAPABILITY.to_owned()];
        }
        let mut link = link::connect(&address, &master, hello, link::Options::default())
            .await
            .unwrap()
            .accept()
            .await
            .unwrap();
        assert!(
            link.peer()
                .hello
                .capabilities
                .iter()
                .any(|c| c == INJECTION_BLOCKED_CAPABILITY),
            "the remote says it tells"
        );
        tower.until(|s| linked(s, master.fingerprint())).await;

        // The tower's hook says the on-screen keyboard took the focus, then let it go.
        tower.service.injection_blocked(osk());
        tower.service.injection_blocked(None);
        if hears {
            assert_eq!(next_block(&mut link, PATIENCE).await, Some(osk()));
            assert_eq!(
                next_block(&mut link, PATIENCE).await,
                Some(None),
                "in the order said"
            );
        } else {
            assert_eq!(
                next_block(&mut link, Duration::from_millis(500)).await,
                None,
                "an older master is told nothing"
            );
        }
    }
}

#[cfg(unix)]
#[tokio::test]
async fn a_remote_whose_injection_is_blocked_brings_the_pointer_home_and_the_state_says_why() {
    use lbm_ipc::input::{encode, read_batch, write_batch, InputEvent};
    use lbm_ipc::peer::InjectionBlock;
    let osk = || Some(InjectionBlock::elevated("osk.exe"));
    let (_laptop_dir, laptop, mut listener) = a_remote("laptop", link::Options::default()).await;
    let desk_dir = tempfile::tempdir().unwrap();
    pair(
        desk_dir.path(),
        laptop.fingerprint(),
        &listener.local_addr().to_string(),
        Role::Remote,
    );
    let hook_path = desk_dir.path().join("desk-input.sock");
    let hook_listener = tokio::net::UnixListener::bind(&hook_path).unwrap();
    let mut desk = Node::new(
        desk_dir.path(),
        Some(hook_path.to_string_lossy().into_owned()),
    );
    desk.turn_on(0, "desk").await;
    let mut laptop_link = linked_from(&mut listener).await;
    assert!(
        laptop_link
            .peer()
            .hello
            .capabilities
            .iter()
            .any(|c| c == lbm_ipc::peer::INJECTION_BLOCKED_CAPABILITY),
        "the master says it hears of blocks"
    );
    desk.hook_ready_for(laptop.fingerprint()).await;
    let accept = || async {
        tokio::time::timeout(PATIENCE, hook_listener.accept())
            .await
            .expect("the agent connects to the hook")
            .unwrap()
            .0
    };
    let blocked =
        |s: &PeersState| peer(s, laptop.fingerprint()).and_then(|p| p.injection_blocked.clone());

    // A block said while the pointer is home is shown, and brings nothing home.
    let mut hook = accept().await;
    laptop_link
        .sender()
        .control(&Control::InjectionBlocked { block: osk() })
        .await
        .unwrap();
    desk.until(|s| blocked(s) == osk()).await;
    laptop_link
        .sender()
        .control(&Control::InjectionBlocked { block: None })
        .await
        .unwrap();
    desk.until(|s| blocked(s).is_none()).await;

    // The pointer crosses; the laptop's hook says Windows drops what it injects.
    let crossing = [InputEvent::Enter { x: 10, y: 10 }];
    write_batch(&mut hook, &encode(&crossing)).await.unwrap();
    assert_eq!(next_input(&mut laptop_link).await, crossing);
    laptop_link
        .sender()
        .control(&Control::InjectionBlocked { block: osk() })
        .await
        .unwrap();

    // The pointer comes home rather than stay frozen there: the hook's connection ends
    // and the laptop is told `Leave`. The state says why, for the page and the tray.
    assert!(
        tokio::time::timeout(PATIENCE, read_batch(&mut hook))
            .await
            .expect("the connection ends")
            .is_err(),
        "the hook's connection is closed"
    );
    assert_eq!(next_input(&mut laptop_link).await, [InputEvent::Leave]);
    let state = desk.until(|s| blocked(s) == osk()).await;
    assert_eq!(
        lbm_agent::peers::blocked_remotes(&state),
        [lbm_agent::peers::BlockedRemote {
            name: "laptop".into(),
            process: "osk.exe".into(),
            cause: lbm_ipc::peer::BlockCause::Integrity,
        }]
    );

    // The link ends: nothing is left to say about it.
    drop(laptop_link);
    desk.until(|s| !linked(s, laptop.fingerprint()) && blocked(s).is_none())
        .await;
}

/// Rulers on a remote's screens: what this machine's painter was asked to do.
mod rulers {
    use super::*;
    use lbm_agent::peers::overlay::{Painter, Shown};
    use lbm_overlay::scene::{Band, Content, Pixels, Side};
    use lbm_overlay::standalone::OnError;
    use tokio::sync::mpsc;

    #[derive(Debug, PartialEq)]
    pub enum Drawn {
        Show(Vec<Band>),
        Update(Vec<Band>),
        Hide,
    }

    struct Recorded(mpsc::UnboundedSender<Drawn>);
    impl Shown for Recorded {
        fn update(&self, bands: Vec<Band>) {
            let _ = self.0.send(Drawn::Update(bands));
        }
    }
    impl Drop for Recorded {
        fn drop(&mut self) {
            let _ = self.0.send(Drawn::Hide);
        }
    }

    /// A painter that draws nothing and says what it was asked, with the way to report
    /// a failure of what it shows, as a display change would.
    pub fn recorder() -> (
        Painter,
        mpsc::UnboundedReceiver<Drawn>,
        mpsc::UnboundedReceiver<OnError>,
    ) {
        let (drawn, drawn_rx) = mpsc::unbounded_channel();
        let (errors, errors_rx) = mpsc::unbounded_channel();
        let painter = Painter::new(move |bands, on_error: OnError| {
            let _ = drawn.send(Drawn::Show(bands));
            let _ = errors.send(on_error);
            Ok(Box::new(Recorded(drawn.clone())) as Box<dyn Shown>)
        });
        (painter, drawn_rx, errors_rx)
    }

    pub async fn next(drawn: &mut mpsc::UnboundedReceiver<Drawn>) -> Drawn {
        tokio::time::timeout(PATIENCE, drawn.recv())
            .await
            .expect("drawn in time")
            .expect("the painter")
    }

    pub fn ruler(display: &PeerDisplay, zero_mm: f64) -> Value {
        let screen = Pixels {
            x: display.x,
            y: display.y,
            width: display.width,
            height: display.height,
        };
        serde_json::to_value(Band {
            monitor: display.id.clone(),
            connector: display.connector.clone(),
            screen,
            rect: Pixels {
                height: 100,
                ..screen
            },
            side: Side::Top,
            scale: 1.0,
            axis_mm: display.width_mm,
            cross_pixels_per_mm: 3.6,
            content: Content::Ruler {
                zero_mm,
                measured_mm: 600.0,
            },
            selected: None,
            passive: false,
        })
        .unwrap()
    }

    /// A tower linked to a test master that announces nothing special, the tower's
    /// rulers recorded.
    pub async fn tower_and_master(
        tower_dir: &Path,
    ) -> (
        Node,
        Link,
        mpsc::UnboundedReceiver<Drawn>,
        mpsc::UnboundedReceiver<OnError>,
    ) {
        let master_dir = tempfile::tempdir().unwrap();
        let master = Identity::load_or_create(master_dir.path()).unwrap();
        pair(tower_dir, master.fingerprint(), "", Role::Master);
        let mut tower = Node::new(tower_dir, None);
        let (painter, drawn, errors) = recorder();
        tower.service.set_overlay_painter(painter);
        tower
            .service
            .local_displays(vec![display("A", 0), display("B", 1920)]);
        let state = tower.turn_on(0, "tower").await;
        let link = master_connects(&master, &state.listen_address.unwrap())
            .await
            .accept()
            .await
            .unwrap();
        assert!(
            link.peer()
                .hello
                .capabilities
                .iter()
                .any(|c| c == lbm_net::wire::OVERLAY_CAPABILITY),
            "the tower says it draws rulers"
        );
        tower.until(|s| linked(s, master.fingerprint())).await;
        (tower, link, drawn, errors)
    }

    pub async fn overlay(link: &Link, revision: u64, bands: Vec<Value>) {
        link.sender()
            .control(&Control::Overlay { revision, bands })
            .await
            .unwrap();
    }

    /// The next `Overlay` a remote is sent.
    pub async fn next_overlay(link: &mut Link) -> (u64, Vec<Value>) {
        loop {
            if let Control::Overlay { revision, bands } = next_control(link).await {
                return (revision, bands);
            }
        }
    }

    /// The next `OverlayFailed` the master is sent.
    pub async fn failure(link: &mut Link) -> (u64, String) {
        loop {
            match next_control(link).await {
                Control::OverlayFailed { revision, error } => return (revision, error),
                Control::Displays { .. } | Control::Wallpapers { .. } => {}
                other => panic!("unexpected {other:?}"),
            }
        }
    }
}

#[tokio::test]
async fn a_masters_rulers_are_drawn_checked_and_taken_down_with_its_link() {
    use rulers::*;
    let tower_dir = tempfile::tempdir().unwrap();
    let (_tower, mut link, mut drawn, _errors) = tower_and_master(tower_dir.path()).await;

    overlay(&link, 1, vec![ruler(&display("A", 0), 0.0)]).await;
    let Drawn::Show(bands) = next(&mut drawn).await else {
        panic!("shown");
    };
    assert_eq!(bands.len(), 1);
    assert_eq!(bands[0].connector, "A-connector");

    // A drag moves the zero: the same surfaces, redrawn.
    overlay(
        &link,
        3,
        vec![
            ruler(&display("A", 0), 12.0),
            ruler(&display("B", 1920), 12.0),
        ],
    )
    .await;
    assert!(matches!(next(&mut drawn).await, Drawn::Update(b) if b.len() == 2));
    // An older one that overtook nothing is ignored.
    overlay(&link, 2, vec![ruler(&display("A", 0), 5.0)]).await;
    // A ruler for a screen as the master wrongly believes it is: refused, taken down,
    // and the master told.
    overlay(&link, 4, vec![ruler(&display("B", 2560), 0.0)]).await;
    assert_eq!(next(&mut drawn).await, Drawn::Hide);
    let (revision, error) = failure(&mut link).await;
    assert_eq!(revision, 4);
    assert!(error.contains("screens changed"), "{error}");

    // Shown again, then the link drops: nothing is left on the screens.
    overlay(&link, 5, vec![ruler(&display("A", 0), 0.0)]).await;
    assert!(matches!(next(&mut drawn).await, Drawn::Show(_)));
    drop(link);
    assert_eq!(next(&mut drawn).await, Drawn::Hide);
}

#[tokio::test]
async fn a_remotes_rulers_go_when_its_screens_change_or_the_mode_is_turned_off() {
    use rulers::*;
    let tower_dir = tempfile::tempdir().unwrap();
    let (mut tower, mut link, mut drawn, mut errors) = tower_and_master(tower_dir.path()).await;

    overlay(&link, 1, vec![ruler(&display("B", 1920), 0.0)]).await;
    assert!(matches!(next(&mut drawn).await, Drawn::Show(_)));
    // B moves: its ruler no longer falls where it should.
    tower
        .service
        .local_displays(vec![display("A", 0), display("B", 2560)]);
    assert_eq!(next(&mut drawn).await, Drawn::Hide);
    let (_, error) = failure(&mut link).await;
    assert!(error.contains("B-connector"), "{error}");

    // The surfaces themselves fail (a display change they saw first): the master is told.
    overlay(&link, 2, vec![ruler(&display("B", 2560), 0.0)]).await;
    assert!(matches!(next(&mut drawn).await, Drawn::Show(_)));
    let _first = errors.recv().await.unwrap();
    let on_error = errors.recv().await.unwrap();
    on_error("Displays changed while showing overlays.".into());
    assert_eq!(next(&mut drawn).await, Drawn::Hide);
    let (revision, error) = failure(&mut link).await;
    assert_eq!(
        (revision, error.as_str()),
        (2, "Displays changed while showing overlays.")
    );

    // Shown again, then the mode is turned off.
    overlay(&link, 3, vec![ruler(&display("A", 0), 0.0)]).await;
    assert!(matches!(next(&mut drawn).await, Drawn::Show(_)));
    tower
        .ask(PeerRequest::Set {
            enabled: false,
            port: None,
            name: None,
            pointer_lock_shortcut: None,
            clipboard: Default::default(),
        })
        .await
        .unwrap();
    assert_eq!(next(&mut drawn).await, Drawn::Hide);
}

#[tokio::test]
async fn only_a_master_draws_on_another_machine() {
    use rulers::*;
    // The desk is the master of the laptop: the laptop's word is no order to the desk.
    let laptop_dir = tempfile::tempdir().unwrap();
    let laptop = Identity::load_or_create(laptop_dir.path()).unwrap();
    let mut listener = link::listen(
        "127.0.0.1:0",
        &laptop,
        Hello::new("laptop", "0", Role::Remote),
        link::Options::default(),
    )
    .await
    .unwrap();
    let desk_dir = tempfile::tempdir().unwrap();
    pair(
        desk_dir.path(),
        laptop.fingerprint(),
        &listener.local_addr().to_string(),
        Role::Remote,
    );
    let mut desk = Node::new(desk_dir.path(), None);
    let (painter, mut drawn, _errors) = recorder();
    desk.service.set_overlay_painter(painter);
    desk.service.local_displays(vec![display("A", 0)]);
    desk.turn_on(0, "desk").await;
    let laptop_link = tokio::time::timeout(PATIENCE, listener.next())
        .await
        .expect("the desk calls its remote back")
        .unwrap()
        .unwrap()
        .accept()
        .await
        .unwrap();
    desk.until(|s| linked(s, laptop.fingerprint())).await;
    overlay(&laptop_link, 1, vec![ruler(&display("A", 0), 0.0)]).await;
    let quiet = tokio::time::timeout(Duration::from_millis(300), drawn.recv()).await;
    assert!(quiet.is_err(), "nothing drawn: {quiet:?}");
}

#[tokio::test]
async fn a_master_shows_a_remotes_failure_of_its_latest_rulers_only() {
    use rulers::*;
    let laptop_dir = tempfile::tempdir().unwrap();
    let laptop = Identity::load_or_create(laptop_dir.path()).unwrap();
    let mut listener = link::listen(
        "127.0.0.1:0",
        &laptop,
        Hello {
            capabilities: vec![lbm_net::wire::OVERLAY_CAPABILITY.into()],
            ..Hello::new("laptop", "0", Role::Remote)
        },
        link::Options::default(),
    )
    .await
    .unwrap();
    let desk_dir = tempfile::tempdir().unwrap();
    pair(
        desk_dir.path(),
        laptop.fingerprint(),
        &listener.local_addr().to_string(),
        Role::Remote,
    );
    let mut desk = Node::new(desk_dir.path(), None);
    desk.turn_on(0, "desk").await;
    let mut laptop_link = tokio::time::timeout(PATIENCE, listener.next())
        .await
        .expect("the desk calls its remote back")
        .unwrap()
        .unwrap()
        .accept()
        .await
        .unwrap();
    desk.until(|s| linked(s, laptop.fingerprint())).await;
    assert!(desk
        .service
        .overlay(laptop.fingerprint(), vec![ruler(&display("A", 0), 0.0)]));
    let (first, bands) = next_overlay(&mut laptop_link).await;
    assert_eq!(bands.len(), 1);
    desk.service
        .overlay(laptop.fingerprint(), vec![ruler(&display("A", 0), 9.0)]);
    let (latest, _) = next_overlay(&mut laptop_link).await;
    assert!(latest > first);

    // The first one failed, but the latest is what is asked for now: nothing to say.
    let failed = |revision| Control::OverlayFailed {
        revision,
        error: "Screen bounds changed.".into(),
    };
    laptop_link.sender().control(&failed(first)).await.unwrap();
    tokio::time::sleep(Duration::from_millis(300)).await;
    let state = desk.state().await;
    assert_eq!(
        peer(&state, laptop.fingerprint()).unwrap().overlay_error,
        None
    );
    // The latest failed: the user is told, until the rulers are asked for again.
    laptop_link.sender().control(&failed(latest)).await.unwrap();
    let told = desk
        .until(|s| peer(s, laptop.fingerprint()).is_some_and(|p| p.overlay_error.is_some()))
        .await;
    assert_eq!(
        peer(&told, laptop.fingerprint())
            .unwrap()
            .overlay_error
            .as_deref(),
        Some("Screen bounds changed.")
    );
    desk.service.overlay(laptop.fingerprint(), Vec::new());
    let (_, bands) = next_overlay(&mut laptop_link).await;
    assert!(bands.is_empty());
    desk.until(|s| peer(s, laptop.fingerprint()).is_some_and(|p| p.overlay_error.is_none()))
        .await;
}

// ---- A remote's Windows service (docs/multi-pc-service-windows.md) ----

/// A remote of a test's own whose `Hello` announces a service at `service`, answering
/// with the certificate whose fingerprint is `machine_id`.
#[cfg(unix)]
async fn a_remote_with_service(
    name: &str,
    service: &link::Listener,
    machine_id: &str,
) -> (tempfile::TempDir, Identity, link::Listener) {
    let directory = tempfile::tempdir().unwrap();
    let identity = Identity::load_or_create(directory.path()).unwrap();
    let mut hello = Hello::new(name, "0", Role::Remote);
    hello.secure_input = Some(lbm_net::wire::SecureInput {
        port: service.local_addr().port(),
        machine_id: machine_id.to_owned(),
    });
    let listener = link::listen("127.0.0.1:0", &identity, hello, link::Options::default())
        .await
        .unwrap();
    (directory, identity, listener)
}

/// The desk, master of `remote`, with a hook endpoint of the test's own.
#[cfg(unix)]
fn a_master_of(
    remote: &Identity,
    listener: &link::Listener,
) -> (tempfile::TempDir, tokio::net::UnixListener, String) {
    let desk_dir = tempfile::tempdir().unwrap();
    pair(
        desk_dir.path(),
        remote.fingerprint(),
        &listener.local_addr().to_string(),
        Role::Remote,
    );
    let hook_path = desk_dir.path().join("desk-input.sock");
    let hook_listener = tokio::net::UnixListener::bind(&hook_path).unwrap();
    let endpoint = hook_path.to_string_lossy().into_owned();
    (desk_dir, hook_listener, endpoint)
}

#[cfg(unix)]
#[tokio::test]
async fn a_remotes_service_takes_the_stream_and_the_agent_gets_it_back_when_the_service_goes() {
    use lbm_ipc::input::{encode, write_batch, InputEvent};
    let (_service_dir, service, mut service_listener) =
        a_remote("tower-service", link::Options::default()).await;
    let (_tower_dir, tower, mut tower_listener) =
        a_remote_with_service("tower", &service_listener, service.fingerprint()).await;
    let (desk_dir, hook_listener, endpoint) = a_master_of(&tower, &tower_listener);
    let mut desk = Node::new(desk_dir.path(), Some(endpoint));
    desk.turn_on(0, "desk").await;
    let mut tower_link = linked_from(&mut tower_listener).await;
    // The master dials the service the tower announced, and checks who answers.
    let mut service_link = linked_from(&mut service_listener).await;
    assert_eq!(service_link.peer().hello.role, Role::Master);
    tokio::time::sleep(Duration::from_millis(200)).await;
    desk.hook_ready_for(tower.fingerprint()).await;

    let (mut hook, _) = tokio::time::timeout(PATIENCE, hook_listener.accept())
        .await
        .expect("the agent connects to the hook")
        .unwrap();
    let enter = [InputEvent::Enter { x: 5, y: 5 }];
    write_batch(&mut hook, &encode(&enter)).await.unwrap();
    assert_eq!(next_input(&mut service_link).await, enter);
    assert!(
        silent(&mut tower_link).await,
        "the agent got the stream too"
    );

    // The service goes: the hop ends there (the pointer comes home) and the stream goes
    // to the agent again.
    service_link.sender().close("stopped").await;
    drop(service_link);
    let (mut hook, _) = tokio::time::timeout(PATIENCE, hook_listener.accept())
        .await
        .expect("the agent connects to the hook again")
        .unwrap();
    let enter = [InputEvent::Enter { x: 7, y: 7 }];
    write_batch(&mut hook, &encode(&enter)).await.unwrap();
    assert_eq!(next_input(&mut tower_link).await, enter);
}

#[cfg(unix)]
#[tokio::test]
async fn a_service_that_answers_with_another_certificate_is_refused_and_the_agent_keeps_the_stream()
{
    use lbm_ipc::input::{encode, write_batch, InputEvent};
    let (_service_dir, _service, mut service_listener) =
        a_remote("impostor", link::Options::default()).await;
    let announced = "ab".repeat(32);
    let (_tower_dir, tower, mut tower_listener) =
        a_remote_with_service("tower", &service_listener, &announced).await;
    let (desk_dir, hook_listener, endpoint) = a_master_of(&tower, &tower_listener);
    let mut desk = Node::new(desk_dir.path(), Some(endpoint));
    desk.turn_on(0, "desk").await;
    let mut tower_link = linked_from(&mut tower_listener).await;

    // The master reaches the impostor, sees another fingerprint and refuses it.
    let pending = tokio::time::timeout(PATIENCE, service_listener.next())
        .await
        .expect("the master dials the announced service")
        .unwrap()
        .unwrap();
    assert!(
        matches!(pending.accept().await, Err(LinkError::Refused(_))),
        "the master refused the impostor"
    );
    desk.hook_ready_for(tower.fingerprint()).await;
    let (mut hook, _) = tokio::time::timeout(PATIENCE, hook_listener.accept())
        .await
        .expect("the agent connects to the hook")
        .unwrap();
    let enter = [InputEvent::Enter { x: 5, y: 5 }];
    write_batch(&mut hook, &encode(&enter)).await.unwrap();
    assert_eq!(next_input(&mut tower_link).await, enter);
}

#[cfg(unix)]
#[tokio::test]
async fn a_service_that_ended_the_session_brings_the_masters_pointer_home_for_a_fresh_crossing() {
    use lbm_ipc::input::{encode, write_batch, InputEvent};
    use tokio::io::AsyncReadExt;
    let (_service_dir, service, mut service_listener) =
        a_remote("tower-service", link::Options::default()).await;
    let (_tower_dir, tower, mut tower_listener) =
        a_remote_with_service("tower", &service_listener, service.fingerprint()).await;
    let (desk_dir, hook_listener, endpoint) = a_master_of(&tower, &tower_listener);
    let mut desk = Node::new(desk_dir.path(), Some(endpoint));
    desk.turn_on(0, "desk").await;
    let _tower_link = linked_from(&mut tower_listener).await;
    let mut service_link = linked_from(&mut service_listener).await;
    tokio::time::sleep(Duration::from_millis(200)).await;
    desk.hook_ready_for(tower.fingerprint()).await;
    let (mut hook, _) = tokio::time::timeout(PATIENCE, hook_listener.accept())
        .await
        .expect("the agent connects to the hook")
        .unwrap();
    write_batch(&mut hook, &encode(&[InputEvent::Enter { x: 5, y: 5 }]))
        .await
        .unwrap();
    next_input(&mut service_link).await;

    // The service lost its injector, say: the master's hook is let go of, which brings
    // its pointer home, and taken again for the next crossing.
    service_link
        .sender()
        .control(&Control::InputReset)
        .await
        .unwrap();
    let mut rest = Vec::new();
    tokio::time::timeout(PATIENCE, hook.read_to_end(&mut rest))
        .await
        .expect("the agent lets go of the hook")
        .unwrap();
    let (mut hook, _) = tokio::time::timeout(PATIENCE, hook_listener.accept())
        .await
        .expect("the agent connects to the hook again")
        .unwrap();
    let enter = [InputEvent::Enter { x: 9, y: 9 }];
    write_batch(&mut hook, &encode(&enter)).await.unwrap();
    // The `Leave` of the hop that ended comes first.
    let mut got = next_input(&mut service_link).await;
    if got == [InputEvent::Leave] {
        got = next_input(&mut service_link).await;
    }
    assert_eq!(got, enter);
}

/// The desk linked with the tower and its service, the hook ready and connected:
/// what the tests below start from.
#[cfg(unix)]
struct WithService {
    _dirs: Vec<tempfile::TempDir>,
    desk: Node,
    tower: Identity,
    tower_link: Link,
    service_link: Link,
    hook: tokio::net::UnixStream,
}

#[cfg(unix)]
async fn driven_through_the_service() -> WithService {
    use lbm_ipc::input::{encode, write_batch, InputEvent};
    let (service_dir, service, mut service_listener) =
        a_remote("tower-service", link::Options::default()).await;
    let (tower_dir, tower, mut tower_listener) =
        a_remote_with_service("tower", &service_listener, service.fingerprint()).await;
    let (desk_dir, hook_listener, endpoint) = a_master_of(&tower, &tower_listener);
    let mut desk = Node::new(desk_dir.path(), Some(endpoint));
    desk.turn_on(0, "desk").await;
    let tower_link = linked_from(&mut tower_listener).await;
    let mut service_link = linked_from(&mut service_listener).await;
    tokio::time::sleep(Duration::from_millis(200)).await;
    desk.hook_ready_for(tower.fingerprint()).await;
    let (mut hook, _) = tokio::time::timeout(PATIENCE, hook_listener.accept())
        .await
        .expect("the agent connects to the hook")
        .unwrap();
    let enter = [InputEvent::Enter { x: 5, y: 5 }];
    write_batch(&mut hook, &encode(&enter)).await.unwrap();
    assert_eq!(next_input(&mut service_link).await, enter);
    WithService {
        _dirs: vec![service_dir, tower_dir, desk_dir],
        desk,
        tower,
        tower_link,
        service_link,
        hook,
    }
}

#[cfg(unix)]
#[tokio::test]
async fn a_reset_from_the_agent_leaves_the_stream_through_the_service_alone() {
    use lbm_ipc::input::{encode, write_batch, InputEvent};
    let mut with = driven_through_the_service().await;
    // Late word from the path the stream no longer takes.
    with.tower_link
        .sender()
        .control(&Control::InputReset)
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(300)).await;
    let motion = [InputEvent::Motion { x: 6, y: 6 }];
    write_batch(&mut with.hook, &encode(&motion)).await.unwrap();
    // On the same connection, with no `Leave` before it: the hop goes on.
    assert_eq!(next_input(&mut with.service_link).await, motion);
}

#[cfg(unix)]
#[tokio::test]
async fn forgetting_a_remote_closes_the_link_with_its_service() {
    let mut with = driven_through_the_service().await;
    with.desk
        .ask(PeerRequest::Forget(with.tower.fingerprint().to_owned()))
        .await
        .unwrap();
    let ended = tokio::time::timeout(PATIENCE, async {
        loop {
            match with.service_link.next().await {
                Some(Event::Closed(_)) | None => return,
                Some(_) => {}
            }
        }
    })
    .await;
    assert!(ended.is_ok(), "the service's link is still open");
}

#[cfg(unix)]
#[tokio::test]
async fn a_service_reached_for_a_replaced_link_is_dialled_again_for_the_current_one() {
    let (_service_dir, service, mut service_listener) =
        a_remote("tower-service", link::Options::default()).await;
    let (_tower_dir, tower, mut tower_listener) =
        a_remote_with_service("tower", &service_listener, service.fingerprint()).await;
    let (desk_dir, _hook_listener, endpoint) = a_master_of(&tower, &tower_listener);
    let mut desk = Node::new(desk_dir.path(), Some(endpoint));
    desk.turn_on(0, "desk").await;
    let tower_link = linked_from(&mut tower_listener).await;
    // The master's first attempt at the service waits here, unanswered...
    let first = tokio::time::timeout(PATIENCE, service_listener.next())
        .await
        .expect("the master dials the service")
        .unwrap()
        .unwrap();
    // ...while the link with the tower's agent ends and is made again.
    tower_link.sender().close("again").await;
    drop(tower_link);
    let _tower_link = linked_from(&mut tower_listener).await;
    tokio::time::sleep(Duration::from_millis(200)).await;
    // The first attempt succeeds now, for a link that is gone: the master drops it and
    // dials again, for the link of the moment.
    let _ = first.accept().await;
    let mut service_link = linked_from(&mut service_listener).await;
    desk.until(|s| linked(s, tower.fingerprint())).await;
    assert_eq!(service_link.peer().hello.role, Role::Master);
    assert!(
        tokio::time::timeout(Duration::from_millis(300), service_link.next())
            .await
            .map_or(true, |event| !matches!(
                event,
                Some(Event::Closed(_)) | None
            )),
        "the second link with the service was closed too"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn a_block_said_by_the_agent_is_not_shown_while_the_service_carries_the_stream() {
    use lbm_ipc::input::{encode, write_batch, InputEvent};
    use lbm_ipc::peer::InjectionBlock;
    let mut with = driven_through_the_service().await;
    let tower_id = with.tower.fingerprint().to_owned();
    with.tower_link
        .sender()
        .control(&Control::InjectionBlocked {
            block: Some(InjectionBlock::elevated("osk.exe")),
        })
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(300)).await;
    let state = with.desk.state().await;
    assert_eq!(
        peer(&state, &tower_id).and_then(|p| p.injection_blocked.clone()),
        None,
        "shown blocked while the service injects"
    );
    let motion = [InputEvent::Motion { x: 6, y: 6 }];
    write_batch(&mut with.hook, &encode(&motion)).await.unwrap();
    assert_eq!(next_input(&mut with.service_link).await, motion);

    // The service goes: the stream is the agent's again, and so is what it said.
    with.service_link.sender().close("stopped").await;
    let state = with
        .desk
        .until(|s| peer(s, &tower_id).is_some_and(|p| p.injection_blocked.is_some()))
        .await;
    assert_eq!(
        peer(&state, &tower_id)
            .and_then(|p| p.injection_blocked.clone())
            .map(|b| b.process),
        Some("osk.exe".to_owned())
    );
}
