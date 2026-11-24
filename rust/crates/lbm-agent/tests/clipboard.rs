//! The shared clipboard between two agents over loopback, each with a clipboard in
//! memory (docs/multi-pc-etat.md, "Presse-papiers partagé"). Never the system's: the
//! developer's own clipboard is in it.

use std::path::Path;
use std::time::Duration;

use lbm_agent::clipboard::memory::{Content, Memory};
use lbm_agent::peers::{
    Answer, ClipboardChange, Notice, PeerRequest, PeerState, PeersState, Service, Setup,
};
use lbm_net::identity::Identity;
use lbm_net::link;
use lbm_net::peers::{PairedPeer, PeerStore};
use lbm_net::wire::{ClipFormat, Role};
use serde_json::Value;
use tokio::sync::mpsc::UnboundedReceiver;

const PATIENCE: Duration = Duration::from_secs(10);

struct Node {
    service: Service,
    notices: UnboundedReceiver<Notice>,
    clipboard: Memory,
    machine_id: String,
    _dir: tempfile::TempDir,
}

impl Node {
    fn new() -> Node {
        let dir = tempfile::tempdir().unwrap();
        let machine_id = Identity::load_or_create(dir.path())
            .unwrap()
            .fingerprint()
            .to_owned();
        let clipboard = Memory::new();
        let mut service = Service::new(
            dir.path().to_owned(),
            Setup {
                bind: "127.0.0.1".parse().unwrap(),
                input_endpoint: None,
                options: link::Options::default(),
                clipboard: clipboard.factory(),
            },
        );
        let notices = service.take_notices().unwrap();
        Node {
            service,
            notices,
            clipboard,
            machine_id,
            _dir: dir,
        }
    }

    fn dir(&self) -> &Path {
        self._dir.path()
    }

    async fn ask(&mut self, request: PeerRequest) -> Result<Value, String> {
        match self.service.request(request) {
            Answer::Now(result) => result,
            Answer::Later(result) => tokio::time::timeout(PATIENCE, result)
                .await
                .expect("an answer in time"),
        }
    }

    async fn set(&mut self, port: Option<u16>, clipboard: ClipboardChange) -> PeersState {
        let state = self
            .ask(PeerRequest::Set {
                enabled: true,
                port,
                name: None,
                pointer_lock_shortcut: None,
                clipboard,
            })
            .await
            .unwrap();
        serde_json::from_value(state).unwrap()
    }

    async fn until(&mut self, want: impl Fn(&PeersState) -> bool) -> PeersState {
        tokio::time::timeout(PATIENCE, async {
            loop {
                if let Notice::Peers(state) = self.notices.recv().await.expect("notices") {
                    if want(&state) {
                        return state;
                    }
                }
            }
        })
        .await
        .expect("the state wanted, in time")
    }
}

fn both_ways(max_bytes: Option<u64>) -> ClipboardChange {
    ClipboardChange {
        send: Some(true),
        receive: Some(true),
        max_bytes,
    }
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

fn linked(state: &PeersState, machine_id: &str) -> bool {
    state.peers.iter().any(|p| {
        p.peer.machine_id == machine_id && p.peer.connected && p.state == PeerState::Paired
    })
}

/// A desk (master) and a tower (remote) paired and linked, each with the clipboard as
/// `desk` and `tower` say.
async fn linked_pair(desk: ClipboardChange, tower: ClipboardChange) -> (Node, Node) {
    let (mut d, mut t) = (Node::new(), Node::new());
    pair(t.dir(), &d.machine_id, "", Role::Master);
    let address = t.set(Some(0), tower).await.listen_address.unwrap();
    pair(d.dir(), &t.machine_id, &address, Role::Remote);
    d.set(Some(0), desk).await;
    let (dm, tm) = (d.machine_id.clone(), t.machine_id.clone());
    d.until(|s| linked(s, &tm)).await;
    t.until(|s| linked(s, &dm)).await;
    (d, t)
}

/// Waits for `memory` to hold an offer.
async fn offered(memory: &Memory) -> Vec<ClipFormat> {
    tokio::time::timeout(PATIENCE, async {
        loop {
            if let Some(Content::Offer { formats, .. }) = memory.content() {
                return formats;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("an offer in time")
}

fn png() -> Vec<u8> {
    let mut bytes = b"\x89PNG\r\n\x1a\n".to_vec();
    bytes.extend((0..200_000u32).map(|i| (i % 251) as u8));
    bytes
}

#[tokio::test]
async fn text_and_a_picture_copied_on_one_machine_are_pasted_on_the_other_both_ways() {
    let (desk, tower) = linked_pair(both_ways(None), both_ways(None)).await;
    assert!(desk.clipboard.is_open() && tower.clipboard.is_open());

    tower.clipboard.copy(vec![
        (ClipFormat::Text, "from the tower\n".as_bytes().to_vec()),
        (ClipFormat::Png, png()),
    ]);
    assert_eq!(
        offered(&desk.clipboard).await,
        [ClipFormat::Text, ClipFormat::Png]
    );
    assert_eq!(
        desk.clipboard.paste(ClipFormat::Text).await.unwrap(),
        b"from the tower\n"
    );
    // Larger than a piece: it crosses in several.
    assert_eq!(desk.clipboard.paste(ClipFormat::Png).await.unwrap(), png());
    // What was received is not sent back: the tower still holds its own copy.
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert!(matches!(tower.clipboard.content(), Some(Content::Local(_))));

    desk.clipboard
        .copy(vec![(ClipFormat::Text, b"from the desk".to_vec())]);
    assert_eq!(offered(&tower.clipboard).await, [ClipFormat::Text]);
    assert_eq!(
        tower.clipboard.paste(ClipFormat::Text).await.unwrap(),
        b"from the desk"
    );
    assert!(matches!(desk.clipboard.content(), Some(Content::Local(_))));
}

#[tokio::test]
async fn the_clipboard_is_not_opened_unless_the_user_lets_it_go_one_way() {
    let (desk, tower) = linked_pair(ClipboardChange::default(), ClipboardChange::default()).await;
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(desk.clipboard.opened(), 0);
    assert_eq!(tower.clipboard.opened(), 0);
}

#[tokio::test]
async fn a_machine_that_only_sends_takes_nothing_from_the_other() {
    let send_only = ClipboardChange {
        send: Some(true),
        receive: Some(false),
        max_bytes: None,
    };
    let (desk, tower) = linked_pair(both_ways(None), send_only).await;
    desk.clipboard
        .copy(vec![(ClipFormat::Text, b"desk".to_vec())]);
    tower
        .clipboard
        .copy(vec![(ClipFormat::Text, b"tower".to_vec())]);
    assert_eq!(offered(&desk.clipboard).await, [ClipFormat::Text]);
    assert_eq!(
        desk.clipboard.paste(ClipFormat::Text).await.unwrap(),
        b"tower"
    );
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert!(matches!(tower.clipboard.content(), Some(Content::Local(_))));
}

#[tokio::test]
async fn content_over_the_bound_is_refused_and_the_state_says_why() {
    let (mut desk, tower) = linked_pair(both_ways(None), both_ways(Some(1000))).await;
    tower.clipboard.copy(vec![(ClipFormat::Png, png())]);
    offered(&desk.clipboard).await;
    let error = desk.clipboard.paste(ClipFormat::Png).await.unwrap_err();
    assert!(error.contains("bound"), "{error}");
    let state = desk.until(|s| s.clipboard.problem.is_some()).await;
    assert!(state.clipboard.problem.unwrap().contains("bound"));

    // The receiver's bound holds too, whatever the sender allows.
    let (desk, tower) = linked_pair(both_ways(Some(1000)), both_ways(None)).await;
    tower.clipboard.copy(vec![(ClipFormat::Png, png())]);
    offered(&desk.clipboard).await;
    let error = desk.clipboard.paste(ClipFormat::Png).await.unwrap_err();
    assert!(error.contains("bound"), "{error}");
}

#[tokio::test]
async fn an_offer_of_a_machine_that_left_is_let_go_of() {
    let (mut desk, tower) = linked_pair(both_ways(None), both_ways(None)).await;
    tower
        .clipboard
        .copy(vec![(ClipFormat::Text, b"soon gone".to_vec())]);
    offered(&desk.clipboard).await;
    drop(tower);
    tokio::time::timeout(PATIENCE, async {
        while desk.clipboard.content().is_some() {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("the offer let go of");
    let state = desk.until(|s| !s.clipboard.active).await;
    assert!(state.clipboard.send && state.clipboard.receive);
    assert!(
        !desk.clipboard.is_open(),
        "closed with no machine to share with"
    );
}

#[tokio::test]
async fn turning_sending_off_takes_the_offer_back_from_the_other_machine() {
    let (desk, mut tower) = linked_pair(both_ways(None), both_ways(None)).await;
    tower
        .clipboard
        .copy(vec![(ClipFormat::Text, b"private".to_vec())]);
    offered(&desk.clipboard).await;
    tower
        .set(
            None,
            ClipboardChange {
                send: Some(false),
                ..ClipboardChange::default()
            },
        )
        .await;
    tokio::time::timeout(PATIENCE, async {
        while desk.clipboard.content().is_some() {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("the offer taken back");
}

#[tokio::test]
async fn two_copies_made_at_once_settle_on_one_that_both_sides_can_paste() {
    for _ in 0..5 {
        let (desk, tower) = linked_pair(both_ways(None), both_ways(None)).await;
        // Both copied before either heard of the other's.
        desk.clipboard
            .copy(vec![(ClipFormat::Text, b"from the desk".to_vec())]);
        tower
            .clipboard
            .copy(vec![(ClipFormat::Text, b"from the tower".to_vec())]);
        let settled = tokio::time::timeout(PATIENCE, async {
            loop {
                tokio::time::sleep(Duration::from_millis(50)).await;
                match (desk.clipboard.content(), tower.clipboard.content()) {
                    (Some(Content::Local(_)), Some(Content::Offer { .. })) => {
                        return (&desk, &tower)
                    }
                    (Some(Content::Offer { .. }), Some(Content::Local(_))) => {
                        return (&tower, &desk)
                    }
                    _ => {}
                }
            }
        })
        .await
        .expect("one copy kept, the other side holding it");
        let (kept, holder) = settled;
        let theirs = kept.clipboard.paste(ClipFormat::Text).await.unwrap();
        assert_eq!(
            holder.clipboard.paste(ClipFormat::Text).await.unwrap(),
            theirs
        );
        // Settled for good: neither moves again.
        tokio::time::sleep(Duration::from_millis(200)).await;
        assert!(matches!(kept.clipboard.content(), Some(Content::Local(_))));
        assert!(matches!(
            holder.clipboard.content(),
            Some(Content::Offer { .. })
        ));
    }
}

#[tokio::test]
async fn a_newer_copy_than_the_one_announced_is_never_sent() {
    let (desk, tower) = linked_pair(both_ways(None), both_ways(None)).await;
    tower
        .clipboard
        .copy(vec![(ClipFormat::Text, b"announced".to_vec())]);
    offered(&desk.clipboard).await;
    // Copied over on the tower, its event not yet handled there: what is read must not be
    // that, a password perhaps.
    tower
        .clipboard
        .replace(vec![(ClipFormat::Text, b"not announced".to_vec())]);
    let error = desk.clipboard.paste(ClipFormat::Text).await.unwrap_err();
    assert!(error.contains("changed since"), "{error}");
}
