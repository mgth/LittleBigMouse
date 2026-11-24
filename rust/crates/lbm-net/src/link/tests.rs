//! Two nodes in one process, over loopback.

use super::*;
use crate::peers::{PairedPeer, PeerStore};
use crate::wire::Role;
use lbm_ipc::input::{self, InputEvent};
use tokio::io::AsyncWriteExt;

/// Fast enough for a test, slow enough that a busy machine does not see a healthy
/// link as dead.
fn fast() -> Options {
    Options {
        keepalive: Duration::from_millis(50),
        missed: 3,
        handshake_timeout: Duration::from_secs(5),
        pairing_timeout: Duration::from_secs(5),
        queue: 64,
    }
}

struct Node {
    _dir: tempfile::TempDir,
    identity: Identity,
}

fn node() -> Node {
    let dir = tempfile::tempdir().unwrap();
    let identity = Identity::load_or_create(dir.path()).unwrap();
    Node {
        _dir: dir,
        identity,
    }
}

/// A remote listening and a master connecting to it, both at `Pending`.
async fn meet(remote: &Node, master: &Node) -> (Listener, Pending, Pending) {
    let mut listener = listen(
        "127.0.0.1:0",
        &remote.identity,
        Hello::new("tower", "test", Role::Remote),
        fast(),
    )
    .await
    .unwrap();
    let address = listener.local_addr();
    let identity = master.identity.clone();
    let connecting = tokio::spawn(async move {
        connect(
            address,
            &identity,
            Hello::new("desk", "test", Role::Master),
            fast(),
        )
        .await
    });
    let at_remote = listener.next().await.unwrap().unwrap();
    let at_master = connecting.await.unwrap().unwrap();
    (listener, at_remote, at_master)
}

/// A live link, seen from the master and from the remote.
async fn linked() -> (Link, Link) {
    let (remote, master) = (node(), node());
    let (_listener, at_remote, at_master) = meet(&remote, &master).await;
    let (on_remote, on_master) = tokio::join!(at_remote.accept(), at_master.accept());
    (on_master.unwrap(), on_remote.unwrap())
}

/// A live link on the master's side whose remote end is a bare TLS stream the test
/// writes to by hand — or leaves silent.
async fn linked_to_raw() -> (Link, Stream) {
    let (remote, master) = (node(), node());
    let (_listener, at_remote, at_master) = meet(&remote, &master).await;
    let mut raw = at_remote.stream;
    send(&mut raw, &Control::Paired).await.unwrap();
    (at_master.accept().await.unwrap(), raw)
}

async fn next(link: &mut Link) -> Event {
    timeout(Duration::from_secs(5), link.next())
        .await
        .expect("the link said nothing")
        .expect("the link ended without Closed")
}

#[tokio::test]
async fn two_nodes_handshake_and_each_sees_the_others_fingerprint() {
    let (remote, master) = (node(), node());
    let (_listener, at_remote, at_master) = meet(&remote, &master).await;

    assert_eq!(at_remote.peer().fingerprint, master.identity.fingerprint());
    assert_eq!(at_master.peer().fingerprint, remote.identity.fingerprint());
    assert_eq!(at_remote.peer().hello.name, "desk");
    assert_eq!(at_remote.peer().hello.role, Role::Master);
    assert_eq!(at_master.peer().hello.name, "tower");
    assert_eq!(at_master.peer().hello.role, Role::Remote);
    assert_eq!(at_master.peer().hello.os, std::env::consts::OS);

    let (on_remote, on_master) = tokio::join!(at_remote.accept(), at_master.accept());
    let (on_remote, on_master) = (on_remote.unwrap(), on_master.unwrap());
    assert_eq!(on_remote.peer().fingerprint, master.identity.fingerprint());
    assert_eq!(on_master.peer().fingerprint, remote.identity.fingerprint());
}

#[tokio::test]
async fn an_unpinned_peer_gets_hello_and_then_refused() {
    let (remote, master) = (node(), node());
    let (_listener, at_remote, at_master) = meet(&remote, &master).await;

    // The master trusts the remote already; the remote has never seen the master.
    let store_dir = tempfile::tempdir().unwrap();
    let store = PeerStore::load(store_dir.path());
    let waiting = tokio::spawn(at_master.accept());

    // While the remote has not decided, the master hears nothing past Hello.
    tokio::time::sleep(fast().keepalive * 4).await;
    assert!(!waiting.is_finished());

    assert!(store.get(&at_remote.peer().fingerprint).is_none());
    at_remote.refuse("not paired").await;
    match waiting.await.unwrap() {
        Err(LinkError::Refused(reason)) => assert_eq!(reason, "not paired"),
        other => panic!("expected a refusal, got {other:?}"),
    }
}

#[tokio::test]
async fn a_pinned_peer_is_accepted_from_the_store() {
    let (remote, master) = (node(), node());
    let dir = tempfile::tempdir().unwrap();
    let mut store = PeerStore::load(dir.path());
    store
        .insert(PairedPeer {
            machine_id: master.identity.fingerprint().to_owned(),
            name: "desk".into(),
            address: String::new(),
            role: Role::Master,
        })
        .unwrap();

    let (_listener, at_remote, at_master) = meet(&remote, &master).await;
    assert!(store.get(&at_remote.peer().fingerprint).is_some());
    let (on_remote, on_master) = tokio::join!(at_remote.accept(), at_master.accept());
    assert!(on_remote.is_ok() && on_master.is_ok());
}

#[tokio::test]
async fn control_and_input_frames_arrive_in_order_interleaved() {
    let (master, mut remote) = linked().await;
    let to_remote = master.sender();

    let batches: Vec<Vec<u8>> = (0..50)
        .map(|i| {
            input::encode(&[
                InputEvent::Motion { x: i, y: -i },
                InputEvent::Button {
                    code: 0x110,
                    pressed: i % 2 == 0,
                },
            ])
        })
        .collect();
    let mut expected = Vec::new();
    for (i, batch) in batches.iter().enumerate() {
        if i % 10 == 0 {
            let message = Control::SelectInput {
                display: format!("DP-{i}"),
                value: i as u16,
                request: Some(i as u64),
            };
            to_remote.control(&message).await.unwrap();
            expected.push(Event::Control(message));
        }
        to_remote.try_input(batch).unwrap();
        expected.push(Event::Input(batch.clone()));
    }
    for want in expected {
        assert_eq!(next(&mut remote).await, want);
    }

    // And the other way.
    let mut master = master;
    let displays = Control::Displays {
        revision: 1,
        displays: vec![],
    };
    remote.sender().control(&displays).await.unwrap();
    remote.sender().input(&batches[0]).await.unwrap();
    assert_eq!(next(&mut master).await, Event::Control(displays));
    assert_eq!(next(&mut master).await, Event::Input(batches[0].clone()));
}

#[tokio::test]
async fn an_idle_link_stays_up() {
    let (mut master, mut remote) = linked().await;
    // Ten keepalive intervals with nothing to say: the pings carry the link.
    let quiet = fast().keepalive * 10;
    assert!(timeout(quiet, master.next()).await.is_err());
    assert!(timeout(Duration::ZERO, remote.next()).await.is_err());
}

#[tokio::test]
async fn a_dead_peer_is_noticed() {
    let (mut master, _silent) = linked_to_raw().await;
    let started = Instant::now();
    assert_eq!(next(&mut master).await, Event::Closed(CloseReason::Lost));
    assert!(started.elapsed() >= fast().dead_after() - fast().keepalive);
    assert!(master.next().await.is_none());
    assert!(master.sender().is_closed());
}

#[tokio::test]
async fn a_vanished_peer_closes_the_link() {
    let (mut master, raw) = linked_to_raw().await;
    drop(raw);
    assert!(matches!(
        next(&mut master).await,
        Event::Closed(CloseReason::Failed(_))
    ));
}

#[tokio::test]
async fn a_peer_saying_bye_closes_the_link_with_its_reason() {
    let (mut master, remote) = linked().await;
    remote.sender().close("shutting down").await;
    assert_eq!(
        next(&mut master).await,
        Event::Closed(CloseReason::Bye("shutting down".into()))
    );
}

#[tokio::test]
async fn an_oversized_frame_closes_the_link() {
    let (mut master, mut raw) = linked_to_raw().await;
    let mut frame = (wire::MAX_INPUT_SIZE as u32 + 1).to_le_bytes().to_vec();
    frame.push(wire::INPUT);
    raw.write_all(&frame).await.unwrap();
    raw.flush().await.unwrap();
    assert!(matches!(
        next(&mut master).await,
        Event::Closed(CloseReason::Failed(_))
    ));
}

#[tokio::test]
async fn garbage_closes_the_link() {
    for garbage in [&b"\x03\x00\x00\x00\x00abc"[..], b"\x00\x00\x00\x00\x09"] {
        let (mut master, mut raw) = linked_to_raw().await;
        raw.write_all(garbage).await.unwrap();
        raw.flush().await.unwrap();
        assert!(matches!(
            next(&mut master).await,
            Event::Closed(CloseReason::Failed(_))
        ));
    }
}

#[tokio::test]
async fn an_oversized_batch_is_refused_before_it_is_queued() {
    let (master, _remote) = linked().await;
    let too_big = vec![0; wire::MAX_INPUT_SIZE + 1];
    assert!(matches!(
        master.sender().try_input(&too_big),
        Err(SendError::Invalid(_))
    ));
}

/// Reads `link` until the bulk transfer `transfer` ends, and returns its data with
/// everything else that came meanwhile, in order.
async fn bulk_until_last(link: &mut Link, transfer: u64) -> (Vec<u8>, Vec<Event>) {
    let mut data = Vec::new();
    let mut others = Vec::new();
    loop {
        match next(link).await {
            Event::Bulk(piece) if piece.transfer == transfer => {
                assert!(piece.data.len() <= BULK_PIECE);
                data.extend_from_slice(&piece.data);
                if piece.last {
                    return (data, others);
                }
            }
            other => others.push(other),
        }
    }
}

#[tokio::test]
async fn a_bulk_transfer_arrives_whole_in_pieces() {
    let (master, mut remote) = linked().await;
    let data: Vec<u8> = (0..BULK_PIECE * 3 + 5).map(|i| i as u8).collect();
    master.sender().bulk(7, &data).await.unwrap();
    master.sender().bulk(8, &[]).await.unwrap();
    let (received, others) = bulk_until_last(&mut remote, 7).await;
    assert_eq!(received, data);
    assert!(others.is_empty());
    // Empty data is one last piece, still.
    let (received, _) = bulk_until_last(&mut remote, 8).await;
    assert!(received.is_empty());
}

#[tokio::test]
async fn input_overtakes_a_bulk_transfer_under_way() {
    let (master, mut remote) = linked().await;
    let sender = master.sender();
    let data = vec![0x5a; 8 * 1024 * 1024];
    let transfer = tokio::spawn({
        let sender = sender.clone();
        async move { sender.bulk(1, &data).await }
    });
    // The transfer is under way once its first piece is here.
    match next(&mut remote).await {
        Event::Bulk(piece) => assert_eq!(piece.transfer, 1),
        other => panic!("expected bulk, got {other:?}"),
    }
    let batch = input::encode(&[InputEvent::Motion { x: 1, y: 1 }]);
    sender.try_input(&batch).unwrap();
    let (_, others) = bulk_until_last(&mut remote, 1).await;
    assert_eq!(
        others,
        vec![Event::Input(batch)],
        "the input arrived before the transfer's end"
    );
    transfer.await.unwrap().unwrap();
}

#[tokio::test]
async fn two_sides_sending_bulk_at_once_both_finish() {
    let (mut master, mut remote) = linked().await;
    let data = vec![1u8; 2 * 1024 * 1024];
    let (to_remote, to_master) = (master.sender(), remote.sender());
    let sending = tokio::spawn({
        let data = data.clone();
        async move {
            let (a, b) = tokio::join!(to_remote.bulk(1, &data), to_master.bulk(2, &data));
            a.and(b)
        }
    });
    let ((at_remote, _), (at_master, _)) = tokio::join!(
        bulk_until_last(&mut remote, 1),
        bulk_until_last(&mut master, 2)
    );
    assert_eq!(at_remote.len(), data.len());
    assert_eq!(at_master.len(), data.len());
    sending.await.unwrap().unwrap();
}
