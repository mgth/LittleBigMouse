//! Discovery between multi-PC managers over real sockets on loopback: each node's
//! datagrams go to the other's discovery port instead of the network's broadcast
//! addresses, everything else is as on a LAN.

use std::net::{SocketAddr, UdpSocket};
use std::path::Path;
use std::time::Duration;

use lbm_agent::peers::{
    Answer, DiscoveredPeer, Notice, PeerRequest, PeerState, PeersState, ProblemKind, Service,
    Settings, Setup,
};
use lbm_net::discovery::{Beacon, Config, Kind, Targets};
use lbm_net::identity::Identity;
use lbm_net::link;
use lbm_net::peers::{PairedPeer, PeerStore};
use lbm_net::wire::Role;
use serde_json::Value;
use tokio::sync::mpsc::UnboundedReceiver;

const PATIENCE: Duration = Duration::from_secs(10);

fn loopback(port: u16) -> SocketAddr {
    SocketAddr::from(([127, 0, 0, 1], port))
}

/// A UDP port nobody listens on right now.
fn free_udp_port() -> u16 {
    UdpSocket::bind(loopback(0))
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

/// A TCP port nobody listens on right now.
fn free_port() -> u16 {
    std::net::TcpListener::bind(loopback(0))
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

/// A node listening for datagrams on `own` and sending its own to `others`.
fn setup(own: u16, others: &[u16]) -> Setup {
    Setup {
        bind: "127.0.0.1".parse().unwrap(),
        input_endpoint: None,
        options: link::Options::default(),
        clipboard: lbm_agent::clipboard::memory::Memory::new().factory(),
        discovery: Some(Config {
            bind: loopback(own),
            targets: Targets::Fixed(others.iter().map(|&port| loopback(port)).collect()),
        }),
    }
}

struct Node {
    service: Service,
    notices: UnboundedReceiver<Notice>,
    last: Option<PeersState>,
}

impl Node {
    fn new(directory: &Path, setup: Setup) -> Node {
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

    async fn set(&mut self, enabled: bool, port: u16, name: &str) -> PeersState {
        let state = self
            .ask(PeerRequest::Set {
                enabled,
                port: Some(port),
                name: Some(name.into()),
                pointer_lock_shortcut: None,
                clipboard: Default::default(),
            })
            .await
            .unwrap();
        serde_json::from_value(state).unwrap()
    }

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
}

fn seen<'a>(state: &'a PeersState, machine_id: &str) -> Option<&'a DiscoveredPeer> {
    state
        .discovered
        .iter()
        .flatten()
        .find(|seen| seen.machine_id == machine_id)
}

fn linked(state: &PeersState, machine_id: &str) -> bool {
    state.peers.iter().any(|p| {
        p.peer.machine_id == machine_id && p.peer.connected && p.state == PeerState::Paired
    })
}

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
async fn two_machines_see_each_other_without_an_address_and_pair_from_what_they_saw() {
    let (desk_dir, tower_dir) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let (desk_udp, tower_udp) = (free_udp_port(), free_udp_port());
    let tower_port = free_port();

    let mut tower = Node::new(tower_dir.path(), setup(tower_udp, &[desk_udp]));
    let tower_id = tower
        .set(true, tower_port, "tower")
        .await
        .machine_id
        .unwrap();
    // Alone on the network: discovery runs, nobody is there.
    let alone = tower.state().await;
    assert_eq!(alone.discovered, Some(Vec::new()));
    assert_eq!(alone.discovery_error, None);

    // The desk starts later: its query reaches the tower, whose answer reaches the desk.
    let mut desk = Node::new(desk_dir.path(), setup(desk_udp, &[tower_udp]));
    let desk_state = desk.set(true, 0, "desk").await;
    let desk_id = desk_state.machine_id.clone().unwrap();
    let desk_port: u16 = desk_state
        .listen_address
        .unwrap()
        .rsplit(':')
        .next()
        .unwrap()
        .parse()
        .unwrap();
    let on_desk = desk.until(|s| seen(s, &tower_id).is_some()).await;
    assert_eq!(
        seen(&on_desk, &tower_id),
        Some(&DiscoveredPeer {
            machine_id: tower_id.clone(),
            name: "tower".into(),
            address: format!("127.0.0.1:{tower_port}"),
            paired: false,
        })
    );
    let on_tower = tower.until(|s| seen(s, &desk_id).is_some()).await;
    assert_eq!(
        seen(&on_tower, &desk_id).unwrap().address,
        format!("127.0.0.1:{desk_port}")
    );
    // Only proposed: nobody is paired, nobody is waiting.
    assert!(on_desk.peers.is_empty() && on_tower.peers.is_empty());

    // The desk pairs with what it saw: the usual pairing, confirmed on both sides.
    let address = seen(&on_desk, &tower_id).unwrap().address.clone();
    desk.ask(PeerRequest::Connect(address)).await.unwrap();
    tower
        .until(|s| {
            s.peers
                .iter()
                .any(|p| p.peer.machine_id == desk_id && p.state == PeerState::Pending)
        })
        .await;
    tower
        .ask(PeerRequest::Accept(desk_id.clone()))
        .await
        .unwrap();
    desk.ask(PeerRequest::Accept(tower_id.clone()))
        .await
        .unwrap();
    let on_desk = desk.until(|s| linked(s, &tower_id)).await;
    assert!(seen(&on_desk, &tower_id).unwrap().paired);
    tower.until(|s| linked(s, &desk_id)).await;

    // The tower stops: it says so, and leaves the desk's list.
    tower.set(false, tower_port, "tower").await;
    let on_desk = desk.until(|s| seen(s, &tower_id).is_none()).await;
    assert_eq!(on_desk.discovered, Some(Vec::new()));
    // And its socket is gone: nothing runs with the mode off.
    assert_eq!(tower.state().await.discovered, None);
    UdpSocket::bind(loopback(tower_udp)).expect("the tower's discovery port is free");
}

#[tokio::test]
async fn a_paired_remote_heard_at_a_new_address_is_called_there_and_the_address_kept() {
    let (desk_dir, tower_dir) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let (desk_udp, tower_udp) = (free_udp_port(), free_udp_port());
    let desk_id = Identity::load_or_create(desk_dir.path())
        .unwrap()
        .fingerprint()
        .to_owned();
    let tower_id = Identity::load_or_create(tower_dir.path())
        .unwrap()
        .fingerprint()
        .to_owned();
    // Paired earlier, when the tower was at another address (nobody listens there now).
    let old_address = format!("127.0.0.1:{}", free_port());
    pair(desk_dir.path(), &tower_id, &old_address, Role::Remote);
    pair(tower_dir.path(), &desk_id, "", Role::Master);

    let mut desk = Node::new(desk_dir.path(), setup(desk_udp, &[tower_udp]));
    desk.set(true, 0, "desk").await;
    desk.until(|s| {
        s.peers.iter().any(|p| {
            p.problem
                .as_ref()
                .is_some_and(|p| p.kind == ProblemKind::Network)
        })
    })
    .await;

    // The tower comes up elsewhere and says so: the desk calls it there at once, as
    // the fingerprint it paired, and keeps the address once the link proves it.
    let tower_port = free_port();
    let mut tower = Node::new(tower_dir.path(), setup(tower_udp, &[desk_udp]));
    tower.set(true, tower_port, "tower").await;
    let on_desk = desk.until(|s| linked(s, &tower_id)).await;
    let new_address = format!("127.0.0.1:{tower_port}");
    let entry = on_desk
        .peers
        .iter()
        .find(|p| p.peer.machine_id == tower_id)
        .unwrap();
    assert_eq!(entry.peer.address, new_address);
    assert_eq!(
        PeerStore::load(desk_dir.path())
            .get(&tower_id)
            .unwrap()
            .address,
        new_address
    );
    assert!(seen(&on_desk, &tower_id).unwrap().paired);
}

#[tokio::test]
async fn a_datagram_that_lies_about_a_paired_fingerprint_changes_nothing_stored() {
    let (desk_dir, stranger_dir) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let desk_udp = free_udp_port();
    let tower_id = Identity::load_or_create(tempfile::tempdir().unwrap().path())
        .unwrap()
        .fingerprint()
        .to_owned();
    let old_address = format!("127.0.0.1:{}", free_port());
    pair(desk_dir.path(), &tower_id, &old_address, Role::Remote);

    let mut desk = Node::new(desk_dir.path(), setup(desk_udp, &[]));
    desk.set(true, 0, "desk").await;
    // The call to the stored address failed, and the next one waits: the datagram
    // below finds nothing under way.
    desk.until(|s| {
        s.peers.iter().any(|p| {
            p.problem
                .as_ref()
                .is_some_and(|p| p.kind == ProblemKind::Network)
        })
    })
    .await;
    // Another machine listens, and someone says the paired tower is there.
    let stranger_port = free_port();
    let mut stranger = Node::new(stranger_dir.path(), setup(free_udp_port(), &[]));
    stranger.set(true, stranger_port, "stranger").await;
    let liar = UdpSocket::bind(loopback(0)).unwrap();
    let lie = Beacon {
        kind: Kind::Announce,
        machine_id: tower_id.clone(),
        name: "tower".into(),
        port: stranger_port,
    };
    liar.send_to(&lie.encode(), loopback(desk_udp)).unwrap();

    // The desk tried, met another fingerprint, and keeps what the user paired.
    let on_desk = desk
        .until(|s| {
            s.peers.iter().any(|p| {
                p.problem
                    .as_ref()
                    .is_some_and(|p| p.kind == ProblemKind::UnknownFingerprint)
            })
        })
        .await;
    assert!(!linked(&on_desk, &tower_id));
    assert_eq!(
        PeerStore::load(desk_dir.path())
            .get(&tower_id)
            .unwrap()
            .address,
        old_address
    );
}

#[tokio::test]
async fn looking_again_lists_anew_the_machines_that_answer() {
    let (desk_dir, tower_dir) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let (desk_udp, tower_udp) = (free_udp_port(), free_udp_port());
    let mut tower = Node::new(tower_dir.path(), setup(tower_udp, &[desk_udp]));
    let tower_id = tower
        .set(true, free_port(), "tower")
        .await
        .machine_id
        .unwrap();
    let mut desk = Node::new(desk_dir.path(), setup(desk_udp, &[tower_udp]));
    desk.set(true, 0, "desk").await;
    desk.until(|s| seen(s, &tower_id).is_some()).await;

    // A machine gone without a word (made up here, as a datagram) stays listed until
    // the user looks again.
    let ghost = "ab".repeat(32);
    let datagram = Beacon {
        kind: Kind::Announce,
        machine_id: ghost.clone(),
        name: "ghost".into(),
        port: 1,
    };
    UdpSocket::bind(loopback(0))
        .unwrap()
        .send_to(&datagram.encode(), loopback(desk_udp))
        .unwrap();
    desk.until(|s| seen(s, &ghost).is_some()).await;

    assert_eq!(desk.ask(PeerRequest::Discover).await, Ok(Value::Null));
    let after = desk
        .until(|s| seen(s, &ghost).is_none() && seen(s, &tower_id).is_some())
        .await;
    assert_eq!(after.discovered.unwrap().len(), 1);
}

#[tokio::test]
async fn with_the_mode_off_or_no_discovery_nothing_is_listed_or_bound() {
    let dir = tempfile::tempdir().unwrap();
    let udp = free_udp_port();
    Settings {
        enabled: false,
        port: free_port(),
        ..Settings::default()
    }
    .save(dir.path())
    .unwrap();
    let mut node = Node::new(dir.path(), setup(udp, &[]));
    assert_eq!(node.state().await.discovered, None);
    UdpSocket::bind(loopback(udp)).expect("nothing bound with the mode off");
    let refused = node.ask(PeerRequest::Discover).await.unwrap_err();
    assert!(refused.contains("off"), "{refused}");

    // A setup without discovery runs the mode without it.
    let other = tempfile::tempdir().unwrap();
    let mut quiet = Node::new(
        other.path(),
        Setup {
            discovery: None,
            ..setup(udp, &[])
        },
    );
    let state = quiet.set(true, 0, "quiet").await;
    assert_eq!(state.discovered, None);
    let refused = quiet.ask(PeerRequest::Discover).await.unwrap_err();
    assert!(refused.contains("discovery"), "{refused}");
}
