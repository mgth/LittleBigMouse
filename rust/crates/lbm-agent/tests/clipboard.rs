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
                discovery: None,
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

    fn reception(&self) -> std::path::PathBuf {
        self.dir().join("reception")
    }

    async fn ask(&mut self, request: PeerRequest) -> Result<Value, String> {
        match self.service.request(request) {
            Answer::Now(result) => result,
            Answer::Later(result) => tokio::time::timeout(PATIENCE, result)
                .await
                .expect("an answer in time"),
        }
    }

    async fn set(&mut self, port: Option<u16>, mut clipboard: ClipboardChange) -> PeersState {
        // Never the developer's own downloads.
        clipboard
            .folder
            .get_or_insert_with(|| self.reception().to_string_lossy().into_owned());
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
        ..ClipboardChange::default()
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
        ..ClipboardChange::default()
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

/// A folder with a file larger than one range, and a lone file, under `dir`.
fn some_files(dir: &Path) -> (Vec<std::path::PathBuf>, Vec<u8>) {
    let photos = dir.join("photos");
    std::fs::create_dir_all(photos.join("2026")).unwrap();
    let large: Vec<u8> = (0..(lbm_agent::peers::clipboard_files::RANGE as usize * 2 + 77))
        .map(|i| (i % 253) as u8)
        .collect();
    std::fs::write(photos.join("2026/été.raw"), &large).unwrap();
    std::fs::write(photos.join("empty"), b"").unwrap();
    std::fs::write(dir.join("note.txt"), b"hello").unwrap();
    (vec![photos, dir.join("note.txt")], large)
}

fn pack(paths: &[std::path::PathBuf]) -> Vec<u8> {
    lbm_agent::peers::clipboard_files::pack_paths(paths)
}

#[tokio::test]
async fn files_copied_on_one_machine_are_fetched_into_the_reception_folder_of_the_other() {
    let (desk, tower) = linked_pair(both_ways(None), both_ways(None)).await;
    let source = tempfile::tempdir().unwrap();
    let (roots, large) = some_files(source.path());
    tower
        .clipboard
        .copy(vec![(ClipFormat::Files, pack(&roots))]);
    assert_eq!(offered(&desk.clipboard).await, [ClipFormat::Files]);

    let pasted = desk.clipboard.paste(ClipFormat::Files).await.unwrap();
    let pasted = lbm_agent::peers::clipboard_files::unpack_paths(&pasted);
    assert_eq!(pasted.len(), 2);
    assert!(pasted.iter().all(|p| p.starts_with(desk.reception())));
    assert_eq!(pasted[0].file_name().unwrap(), "photos");
    assert_eq!(
        std::fs::read(pasted[0].join("2026/été.raw")).unwrap(),
        large
    );
    assert_eq!(std::fs::read(pasted[0].join("empty")).unwrap(), b"");
    assert_eq!(std::fs::read(&pasted[1]).unwrap(), b"hello");

    // A second paste is handed the same files, not fetched again.
    let again = desk.clipboard.paste(ClipFormat::Files).await.unwrap();
    assert_eq!(
        lbm_agent::peers::clipboard_files::unpack_paths(&again),
        pasted
    );
    assert_eq!(std::fs::read_dir(desk.reception()).unwrap().count(), 1);
}

#[tokio::test]
async fn files_over_the_bound_are_not_offered_and_the_copier_says_why() {
    let bounded = ClipboardChange {
        files_max_bytes: Some(1000),
        ..both_ways(None)
    };
    let (desk, mut tower) = linked_pair(both_ways(None), bounded).await;
    let source = tempfile::tempdir().unwrap();
    let (roots, _) = some_files(source.path());
    tower.clipboard.copy(vec![
        (ClipFormat::Text, b"a caption".to_vec()),
        (ClipFormat::Files, pack(&roots)),
    ]);
    // The text still crosses; the files do not.
    assert_eq!(offered(&desk.clipboard).await, [ClipFormat::Text]);
    let state = tower.until(|s| s.clipboard.problem.is_some()).await;
    assert!(state.clipboard.problem.unwrap().contains("bound"));
}

#[tokio::test]
async fn an_offer_of_files_that_would_land_outside_the_reception_folder_is_refused() {
    use lbm_net::wire::{ClipFile, Control, Hello, CLIPBOARD_CAPABILITY};
    let mut tower = Node::new();
    let master = Identity::load_or_create(tempfile::tempdir().unwrap().path()).unwrap();
    pair(tower.dir(), master.fingerprint(), "", Role::Master);
    let address = tower
        .set(Some(0), both_ways(None))
        .await
        .listen_address
        .unwrap();
    let mut hello = Hello::new("test master", "0", Role::Master);
    hello.capabilities.push(CLIPBOARD_CAPABILITY.to_owned());
    let link = link::connect(address.as_str(), &master, hello, link::Options::default())
        .await
        .unwrap()
        .accept()
        .await
        .unwrap();
    tower.until(|s| s.clipboard.active).await;
    link.sender()
        .control(&Control::ClipboardOffer {
            serial: 1,
            formats: vec![ClipFormat::Files],
            files: vec![ClipFile {
                path: "../../.bashrc".into(),
                size: 4,
                folder: false,
            }],
            seen: 0,
            receiving: false,
        })
        .await
        .unwrap();
    let state = tower.until(|s| s.clipboard.problem.is_some()).await;
    assert!(
        state
            .clipboard
            .problem
            .unwrap()
            .contains("cannot be pasted"),
        "refused for its name"
    );
    assert_eq!(tower.clipboard.content(), None, "nothing offered here");
}

#[tokio::test]
async fn a_fetch_whose_sender_leaves_fails_and_leaves_nothing_behind() {
    use lbm_net::link::Event as LinkEvent;
    use lbm_net::wire::{ClipFile, Control, Hello, CLIPBOARD_CAPABILITY};
    let mut tower = Node::new();
    let master = Identity::load_or_create(tempfile::tempdir().unwrap().path()).unwrap();
    pair(tower.dir(), master.fingerprint(), "", Role::Master);
    let address = tower
        .set(Some(0), both_ways(None))
        .await
        .listen_address
        .unwrap();
    let mut hello = Hello::new("test master", "0", Role::Master);
    hello.capabilities.push(CLIPBOARD_CAPABILITY.to_owned());
    let mut link = link::connect(address.as_str(), &master, hello, link::Options::default())
        .await
        .unwrap()
        .accept()
        .await
        .unwrap();
    tower.until(|s| s.clipboard.active).await;
    link.sender()
        .control(&Control::ClipboardOffer {
            serial: 1,
            formats: vec![ClipFormat::Files],
            files: vec![ClipFile {
                path: "big.bin".into(),
                size: 1 << 20,
                folder: false,
            }],
            seen: 0,
            receiving: false,
        })
        .await
        .unwrap();
    offered(&tower.clipboard).await;
    let clipboard = tower.clipboard.clone();
    let paste = tokio::spawn(async move { clipboard.paste(ClipFormat::Files).await });
    // The first range is asked for; the master leaves instead of sending it.
    tokio::time::timeout(PATIENCE, async {
        loop {
            match link.next().await {
                Some(LinkEvent::Control(Control::ClipboardRequest { item, .. })) => {
                    assert_eq!(item, Some(0));
                    return;
                }
                Some(_) => {}
                None => panic!("the link ended"),
            }
        }
    })
    .await
    .expect("a request in time");
    assert!(
        tower.reception().read_dir().unwrap().count() == 1,
        "fetching"
    );
    drop(link);
    let error = tokio::time::timeout(PATIENCE, paste)
        .await
        .expect("the paste answered")
        .unwrap()
        .unwrap_err();
    assert!(!error.is_empty());
    tokio::time::timeout(PATIENCE, async {
        while tower.reception().read_dir().unwrap().count() > 0 {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("what was fetched is removed");
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

#[tokio::test]
async fn files_whose_copy_was_cleared_while_they_were_listed_are_never_offered() {
    let (desk, tower) = linked_pair(both_ways(None), both_ways(None)).await;
    let source = tempfile::tempdir().unwrap();
    let (roots, _) = some_files(source.path());
    // The listing reads the paths first: slow, so that the copy is emptied before it ends.
    tower.clipboard.slow_reads(Duration::from_millis(200));
    tower
        .clipboard
        .copy(vec![(ClipFormat::Files, pack(&roots))]);
    // Emptied once the paths were read, before the listing of the folder ends.
    tokio::time::sleep(Duration::from_millis(50)).await;
    tower.clipboard.copy(vec![]);
    for _ in 0..10 {
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert!(
            !matches!(desk.clipboard.content(), Some(Content::Offer { formats, .. }) if formats.contains(&ClipFormat::Files)),
            "the files of a copy gone are not offered"
        );
    }
}

#[tokio::test]
async fn a_listing_cut_short_by_the_last_machine_leaving_is_not_offered_when_it_comes_back() {
    let (mut desk, tower) = linked_pair(both_ways(None), both_ways(None)).await;
    let source = tempfile::tempdir().unwrap();
    let (roots, _) = some_files(source.path());
    tower.clipboard.slow_reads(Duration::from_millis(300));
    tower
        .clipboard
        .copy(vec![(ClipFormat::Files, pack(&roots))]);
    tokio::time::sleep(Duration::from_millis(50)).await;
    // The only machine the tower shares with goes while the files are being listed: the
    // tower's clipboard closes.
    let off = |enabled| PeerRequest::Set {
        enabled,
        port: None,
        name: None,
        pointer_lock_shortcut: None,
        clipboard: ClipboardChange::default(),
    };
    desk.ask(off(false)).await.unwrap();
    tokio::time::sleep(Duration::from_millis(500)).await;
    // And comes back: what was being listed then is not this machine's copy now.
    desk.ask(off(true)).await.unwrap();
    let tm = tower.machine_id.clone();
    desk.until(|s| linked(s, &tm)).await;
    for _ in 0..10 {
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert!(
            !matches!(desk.clipboard.content(), Some(Content::Offer { .. })),
            "nothing is offered from a listing of a clipboard closed since"
        );
    }
}

/// A desk (master) linked to two towers (its remotes), each with the clipboard as the
/// options say: the master relays between the remotes, linked to it only.
async fn linked_three(
    desk: ClipboardChange,
    a: ClipboardChange,
    b: ClipboardChange,
) -> (Node, Node, Node) {
    let (mut d, mut ta, mut tb) = (Node::new(), Node::new(), Node::new());
    pair(ta.dir(), &d.machine_id, "", Role::Master);
    pair(tb.dir(), &d.machine_id, "", Role::Master);
    let address_a = ta.set(Some(0), a).await.listen_address.unwrap();
    let address_b = tb.set(Some(0), b).await.listen_address.unwrap();
    pair(d.dir(), &ta.machine_id, &address_a, Role::Remote);
    pair(d.dir(), &tb.machine_id, &address_b, Role::Remote);
    d.set(Some(0), desk).await;
    let (dm, am, bm) = (
        d.machine_id.clone(),
        ta.machine_id.clone(),
        tb.machine_id.clone(),
    );
    d.until(|s| linked(s, &am) && linked(s, &bm)).await;
    ta.until(|s| linked(s, &dm)).await;
    tb.until(|s| linked(s, &dm)).await;
    (d, ta, tb)
}

/// The number of the offer `memory` holds, if it holds one.
fn offer_serial(memory: &Memory) -> Option<u64> {
    match memory.content() {
        Some(Content::Offer { serial, .. }) => Some(serial),
        _ => None,
    }
}

/// Waits for `memory` to hold an offer other than `before`.
async fn offered_anew(memory: &Memory, before: Option<u64>) -> Vec<ClipFormat> {
    tokio::time::timeout(PATIENCE, async {
        loop {
            if let Some(Content::Offer { serial, formats }) = memory.content() {
                if Some(serial) != before {
                    return formats;
                }
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("a new offer in time")
}

/// Waits for `memory` to hold nothing.
async fn emptied(memory: &Memory) {
    tokio::time::timeout(PATIENCE, async {
        while memory.content().is_some() {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("the offer let go of");
}

#[tokio::test]
async fn text_and_a_picture_copied_on_one_remote_are_pasted_on_another_through_the_master() {
    let (desk, a, b) = linked_three(both_ways(None), both_ways(None), both_ways(None)).await;

    a.clipboard.copy(vec![
        (ClipFormat::Text, "from tower A\n".as_bytes().to_vec()),
        (ClipFormat::Png, png()),
    ]);
    assert_eq!(
        offered(&b.clipboard).await,
        [ClipFormat::Text, ClipFormat::Png]
    );
    assert_eq!(
        b.clipboard.paste(ClipFormat::Text).await.unwrap(),
        b"from tower A\n"
    );
    // Larger than a piece: relayed in several, as they come.
    assert_eq!(b.clipboard.paste(ClipFormat::Png).await.unwrap(), png());
    // The master holds it too, and the copier its own: never announced back to it.
    assert_eq!(
        desk.clipboard.paste(ClipFormat::Text).await.unwrap(),
        b"from tower A\n"
    );
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert!(matches!(a.clipboard.content(), Some(Content::Local(_))));

    // And the other way.
    let before = offer_serial(&a.clipboard);
    b.clipboard
        .copy(vec![(ClipFormat::Text, b"from tower B".to_vec())]);
    assert_eq!(offered_anew(&a.clipboard, before).await, [ClipFormat::Text]);
    assert_eq!(
        a.clipboard.paste(ClipFormat::Text).await.unwrap(),
        b"from tower B"
    );
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert!(matches!(b.clipboard.content(), Some(Content::Local(_))));
}

#[tokio::test]
async fn files_copied_on_one_remote_are_fetched_on_another_through_the_master_both_ways() {
    let (_desk, a, b) = linked_three(both_ways(None), both_ways(None), both_ways(None)).await;
    for (from, to) in [(&a, &b), (&b, &a)] {
        let source = tempfile::tempdir().unwrap();
        let (roots, large) = some_files(source.path());
        let before = offer_serial(&to.clipboard);
        from.clipboard.copy(vec![(ClipFormat::Files, pack(&roots))]);
        assert_eq!(
            offered_anew(&to.clipboard, before).await,
            [ClipFormat::Files]
        );
        let pasted = to.clipboard.paste(ClipFormat::Files).await.unwrap();
        let pasted = lbm_agent::peers::clipboard_files::unpack_paths(&pasted);
        assert_eq!(pasted.len(), 2);
        assert!(pasted.iter().all(|p| p.starts_with(to.reception())));
        // Several ranges, each relayed.
        assert_eq!(
            std::fs::read(pasted[0].join("2026/été.raw")).unwrap(),
            large
        );
        assert_eq!(std::fs::read(pasted[0].join("empty")).unwrap(), b"");
        assert_eq!(std::fs::read(&pasted[1]).unwrap(), b"hello");
    }
}

#[tokio::test]
async fn a_copy_on_the_master_reaches_every_remote() {
    let (desk, a, b) = linked_three(both_ways(None), both_ways(None), both_ways(None)).await;
    desk.clipboard
        .copy(vec![(ClipFormat::Text, b"from the desk".to_vec())]);
    for remote in [&a, &b] {
        assert_eq!(offered(&remote.clipboard).await, [ClipFormat::Text]);
        assert_eq!(
            remote.clipboard.paste(ClipFormat::Text).await.unwrap(),
            b"from the desk"
        );
    }
}

#[tokio::test]
async fn a_relayed_offer_is_withdrawn_when_its_remote_leaves_or_stops_sending() {
    let (_desk, a, b) = linked_three(both_ways(None), both_ways(None), both_ways(None)).await;
    a.clipboard
        .copy(vec![(ClipFormat::Text, b"soon gone".to_vec())]);
    offered(&b.clipboard).await;
    drop(a);
    emptied(&b.clipboard).await;

    // Stops sending: what of it the master relayed goes too.
    let (_desk, mut a, c) = linked_three(both_ways(None), both_ways(None), both_ways(None)).await;
    a.clipboard
        .copy(vec![(ClipFormat::Text, b"private".to_vec())]);
    offered(&c.clipboard).await;
    a.set(
        None,
        ClipboardChange {
            send: Some(false),
            ..ClipboardChange::default()
        },
    )
    .await;
    emptied(&c.clipboard).await;
    drop(b);
}

#[tokio::test]
async fn the_master_relays_only_while_it_sends_and_within_its_own_bound() {
    // A master that does not send keeps what it receives to itself.
    let receive_only = ClipboardChange {
        send: Some(false),
        receive: Some(true),
        ..ClipboardChange::default()
    };
    let (desk, a, b) = linked_three(receive_only, both_ways(None), both_ways(None)).await;
    a.clipboard
        .copy(vec![(ClipFormat::Text, b"for the desk".to_vec())]);
    offered(&desk.clipboard).await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(b.clipboard.content().is_none());

    // Its bound holds for what it relays, whatever the remotes allow.
    let (_desk, a, b) = linked_three(both_ways(Some(1000)), both_ways(None), both_ways(None)).await;
    a.clipboard.copy(vec![(ClipFormat::Png, png())]);
    offered(&b.clipboard).await;
    let error = b.clipboard.paste(ClipFormat::Png).await.unwrap_err();
    assert!(error.contains("bound"), "{error}");
    // The copier is still asked for the next one.
    a.clipboard
        .copy(vec![(ClipFormat::Text, b"small".to_vec())]);
    let before = offer_serial(&b.clipboard);
    offered_anew(&b.clipboard, before).await;
    assert_eq!(b.clipboard.paste(ClipFormat::Text).await.unwrap(), b"small");
}

#[tokio::test]
async fn two_remotes_copying_at_once_settle_on_one_copy_all_three_can_paste() {
    for _ in 0..5 {
        let (desk, a, b) = linked_three(both_ways(None), both_ways(None), both_ways(None)).await;
        a.clipboard
            .copy(vec![(ClipFormat::Text, b"from tower A".to_vec())]);
        b.clipboard
            .copy(vec![(ClipFormat::Text, b"from tower B".to_vec())]);
        let kept = tokio::time::timeout(PATIENCE, async {
            loop {
                tokio::time::sleep(Duration::from_millis(50)).await;
                let local = |n: &Node| matches!(n.clipboard.content(), Some(Content::Local(_)));
                let offer = |n: &Node| matches!(n.clipboard.content(), Some(Content::Offer { .. }));
                if local(&a) && offer(&b) && offer(&desk) {
                    return &a;
                }
                if local(&b) && offer(&a) && offer(&desk) {
                    return &b;
                }
            }
        })
        .await
        .expect("one copy kept, the others holding it");
        // Settled for good, and every paste finds the same.
        tokio::time::sleep(Duration::from_millis(200)).await;
        let theirs = kept.clipboard.paste(ClipFormat::Text).await.unwrap();
        for node in [&desk, &a, &b] {
            assert_eq!(
                node.clipboard.paste(ClipFormat::Text).await.unwrap(),
                theirs
            );
        }
    }
}
