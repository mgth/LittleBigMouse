use std::sync::mpsc::Receiver;
use std::time::Duration;

use lbm_net::identity::Identity;
use lbm_net::link::{self, Event, Link, LinkError};
use lbm_net::wire::{Control, Hello, Role};

use super::*;

const PATIENCE: Duration = Duration::from_secs(10);

fn identity() -> (tempfile::TempDir, Identity) {
    let directory = tempfile::tempdir().unwrap();
    let identity = Identity::load_or_create(directory.path()).unwrap();
    (directory, identity)
}

fn approving(masters: &[&Identity]) -> TrustStore {
    let mut store = TrustStore::default();
    for master in masters {
        store.trust(master.fingerprint(), "master").unwrap();
    }
    store
}

/// The service's network on loopback, approving `masters`.
fn service(masters: &[&Identity]) -> (tempfile::TempDir, Network, Receiver<Stream>) {
    let (directory, identity) = identity();
    let (out, heard) = std::sync::mpsc::channel();
    let (network, serving) = start(
        "127.0.0.1:0".parse().unwrap(),
        identity,
        hello("tower"),
        link::Options::default(),
        approving(masters),
        out,
    );
    tokio::spawn(serving);
    (directory, network, heard)
}

/// The next thing the network told, waiting at most [`PATIENCE`].
async fn next(heard: &Receiver<Stream>) -> Stream {
    let deadline = tokio::time::Instant::now() + PATIENCE;
    loop {
        if let Ok(stream) = heard.try_recv() {
            return stream;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "nothing heard in time"
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

async fn port(heard: &Receiver<Stream>) -> u16 {
    match next(heard).await {
        Stream::Listening(Some(port)) => port,
        other => panic!("not listening: {other:?}"),
    }
}

async fn connect(master: &Identity, port: u16) -> Result<Link, LinkError> {
    let pending = link::connect(
        ("127.0.0.1", port),
        master,
        Hello::new("desk", "0", Role::Master),
        link::Options::default(),
    )
    .await?;
    pending.accept().await
}

async fn closed(link: &mut Link) -> bool {
    tokio::time::timeout(PATIENCE, async {
        loop {
            match link.next().await {
                Some(Event::Closed(_)) | None => return,
                Some(_) => {}
            }
        }
    })
    .await
    .is_ok()
}

#[tokio::test]
async fn an_approved_masters_stream_is_handed_over_and_its_end_said() {
    let (_master_dir, master) = identity();
    let (_dir, _network, heard) = service(&[&master]);
    let port = port(&heard).await;
    let link = connect(&master, port).await.unwrap();
    link.sender().input(&[1, 2, 3]).await.unwrap();
    assert_eq!(
        next(&heard).await,
        Stream::Batch {
            link: 1,
            batch: vec![1, 2, 3]
        }
    );
    link.sender().close("done").await;
    assert_eq!(next(&heard).await, Stream::Ended { link: 1 });
}

#[tokio::test]
async fn a_master_that_is_not_approved_is_refused() {
    let (_approved_dir, approved) = identity();
    let (_stranger_dir, stranger) = identity();
    let (_dir, _network, heard) = service(&[&approved]);
    let port = port(&heard).await;
    assert!(matches!(
        connect(&stranger, port).await,
        Err(LinkError::Refused(_))
    ));
}

#[tokio::test]
async fn an_empty_store_is_a_closed_port_and_listening_follows_the_store() {
    let (_master_dir, master) = identity();
    let (_dir, network, heard) = service(&[]);
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert!(heard.try_recv().is_err(), "listening with nobody approved");
    network.trust(approving(&[&master]));
    let port = port(&heard).await;
    assert!(connect(&master, port).await.is_ok());
    let _ended = next(&heard).await;
    network.trust(TrustStore::default());
    let mut told = Vec::new();
    while !told.contains(&Stream::Listening(None)) {
        told.push(next(&heard).await);
    }
    assert!(connect(&master, port).await.is_err(), "still listening");
}

#[tokio::test]
async fn a_withdrawal_closes_the_link_of_the_master_driving_at_once() {
    let (_master_dir, master) = identity();
    let (_dir, network, heard) = service(&[&master]);
    let port = port(&heard).await;
    let mut link = connect(&master, port).await.unwrap();
    link.sender().input(&[1]).await.unwrap();
    next(&heard).await;
    // Withdrawn, while another stays approved: the port stays open.
    let (_other_dir, other) = identity();
    network.trust(approving(&[&other]));
    assert!(
        closed(&mut link).await,
        "the withdrawn master's link stays open"
    );
    assert_eq!(next(&heard).await, Stream::Ended { link: 1 });
}

#[tokio::test]
async fn a_newer_master_replaces_the_one_driving() {
    let (_first_dir, first) = identity();
    let (_second_dir, second) = identity();
    let (_dir, _network, heard) = service(&[&first, &second]);
    let port = port(&heard).await;
    let mut older = connect(&first, port).await.unwrap();
    let newer = connect(&second, port).await.unwrap();
    assert_eq!(next(&heard).await, Stream::Ended { link: 1 });
    assert!(
        closed(&mut older).await,
        "the older master's link stays open"
    );
    newer.sender().input(&[7]).await.unwrap();
    assert_eq!(
        next(&heard).await,
        Stream::Batch {
            link: 2,
            batch: vec![7]
        }
    );
}

#[tokio::test]
async fn a_reset_is_said_to_the_master_of_that_link_only() {
    let (_master_dir, master) = identity();
    let (_dir, network, heard) = service(&[&master]);
    let port = port(&heard).await;
    let mut link = connect(&master, port).await.unwrap();
    // Driving once its first batch is heard.
    link.sender().input(&[1]).await.unwrap();
    next(&heard).await;
    network.reset(99);
    network.reset(1);
    let said = tokio::time::timeout(PATIENCE, async {
        loop {
            match link.next().await {
                Some(Event::Control(message)) => return message,
                Some(_) => {}
                None => panic!("the link ended"),
            }
        }
    })
    .await
    .expect("a control message in time");
    assert_eq!(said, Control::InputReset);
}
