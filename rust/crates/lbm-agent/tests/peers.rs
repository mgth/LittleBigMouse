//! The multi-PC manager over real sockets on loopback: pairing, the screens a remote
//! publishes, and the input stream from one hook's endpoint to the other's.

use std::path::Path;
use std::time::Duration;

use lbm_agent::peers::{
    Answer, Notice, PeerRequest, PeerState, PeersState, Service, Settings, Setup,
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
    Setup {
        bind: "127.0.0.1".parse().unwrap(),
        input_endpoint,
        options: link::Options::default(),
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
}

impl Node {
    fn new(directory: &Path, input_endpoint: Option<String>) -> Node {
        let mut service = Service::new(directory.to_owned(), setup(input_endpoint));
        let notices = service.take_notices().unwrap();
        Node { service, notices }
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
            })
            .await
            .unwrap();
        serde_json::from_value(state).unwrap()
    }

    /// The first state, now or announced later, that satisfies `want`.
    async fn until(&mut self, want: impl Fn(&PeersState) -> bool) -> PeersState {
        let now = self.state().await;
        if want(&now) {
            return now;
        }
        tokio::time::timeout(PATIENCE, async {
            loop {
                let Notice::Peers(state) = self.notices.recv().await.expect("notices");
                if want(&state) {
                    return state;
                }
            }
        })
        .await
        .expect("the state wanted, in time")
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

    use lbm_ipc::input::{read_batch, write_batch};
    use tokio::sync::mpsc;

    pub struct Endpoint {
        pub path: String,
        pub received: mpsc::UnboundedReceiver<Option<Vec<u8>>>,
        /// Batches written to the client of the moment (a master hook's capture).
        pub to_client: mpsc::UnboundedSender<Vec<u8>>,
    }

    pub fn serve(directory: &Path, name: &str) -> Endpoint {
        let path = directory.join(name);
        let listener = tokio::net::UnixListener::bind(&path).unwrap();
        let (received_tx, received) = mpsc::unbounded_channel();
        let (to_client, mut outgoing) = mpsc::unbounded_channel::<Vec<u8>>();
        tokio::spawn(async move {
            // One client, as the hook serves: the first is the one under test.
            let (stream, _) = listener.accept().await.unwrap();
            let (mut reader, mut writer) = tokio::io::split(stream);
            tokio::spawn(async move {
                while let Some(batch) = outgoing.recv().await {
                    if write_batch(&mut writer, &batch).await.is_err() {
                        break;
                    }
                }
            });
            while let Ok(batch) = read_batch(&mut reader).await {
                let _ = received_tx.send(Some(batch));
            }
            let _ = received_tx.send(None);
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
    })
    .await
    .unwrap();
    assert_eq!(tower_hook.next().await, None);
    assert_eq!(desk_hook.next().await, None);
    tower.until(|s| !linked(s, &desk_id)).await;
}
