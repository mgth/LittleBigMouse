//! Where the service takes the input stream itself (docs/multi-pc-service-windows.md,
//! § 1, option B): a listener of its own, under an identity of its own, that admits
//! only the masters an administrator approved.
//!
//! The link is `lbm_net::link` as the agents use it — mutual TLS, `Hello`, keepalive,
//! input frames — so a master speaks to the service as to a remote. Before anything
//! but `Hello` is said, the master's certificate is looked up in the store
//! ([`TrustStore::allows`]): any other is refused.
//!
//! Several approved masters may be linked at once, and none of them is closed for
//! another: an agent dials again whatever closes its link, so two masters evicting
//! each other would take turns forever. **Who drives is decided at the start of an
//! input session**: the master whose batch carries an `Enter` drives from then on; the
//! one that drove before sees its stream end here ([`Stream::Ended`], whose `Leave`
//! releases what it held) and is told `InputReset`, which brings its pointer home.
//! Batches of a master that does not drive are dropped. Batches and ends of every link
//! go through the one loop that decides, so nothing of a stream follows its end.
//!
//! - It listens only while the store approves someone: an empty store is a closed port.
//! - A withdrawal takes effect at once: the link of a master no longer approved is
//!   closed, and its stream ends ([`Stream::Ended`]), which makes the injector release
//!   whatever it held.
//! - Only input frames are taken from a master; its control messages are ignored.
//!   What the service says back is `InputReset` ([`Network::reset`]), when its injector
//!   lost the session: the master brings its pointer home.
//!
//! The service's deciding loop is not async: what this module hears goes to it on a
//! plain channel ([`Stream`]), and nothing here touches the system.

use std::net::SocketAddr;

use lbm_net::identity::Identity;
use lbm_net::link::{self, Event, LinkSender};
use lbm_net::wire::{Control, Hello, Role};
use tokio::sync::{mpsc, watch};

use crate::trust::TrustStore;

/// The port the service listens on: the agents' (`lbm_net::DEFAULT_PORT`) and one.
pub const PORT: u16 = lbm_net::DEFAULT_PORT + 1;

/// What the network tells the deciding loop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Stream {
    /// The service listens on this port, or (`None`) no longer listens.
    Listening(Option<u16>),
    /// A batch of the stream of the master on link `link`, still encoded.
    Batch { link: u64, batch: Vec<u8> },
    /// The stream of link `link` is over: closed, lost, withdrawn or replaced.
    Ended { link: u64 },
}

/// The handle the deciding loop keeps.
#[derive(Clone)]
pub struct Network {
    trust: watch::Sender<TrustStore>,
    resets: mpsc::UnboundedSender<u64>,
}

impl Network {
    /// The approved masters changed: listening starts or stops, and the link of a
    /// master no longer approved is closed.
    pub fn trust(&self, store: TrustStore) {
        self.trust.send_replace(store);
    }

    /// The injector lost the session of link `link`: its master is told `InputReset`.
    pub fn reset(&self, link: u64) {
        let _ = self.resets.send(link);
    }
}

/// Runs the network until the [`Network`] handle is dropped. `bind` is the address to
/// listen on, with its port (0 in tests).
pub fn start(
    bind: SocketAddr,
    identity: Identity,
    hello: Hello,
    options: link::Options,
    trust: TrustStore,
    out: std::sync::mpsc::Sender<Stream>,
) -> (Network, impl std::future::Future<Output = ()>) {
    let (trust_sender, trust_receiver) = watch::channel(trust);
    let (resets, resets_receiver) = mpsc::unbounded_channel();
    let network = Network {
        trust: trust_sender,
        resets,
    };
    let serving = serve(
        bind,
        identity,
        hello,
        options,
        trust_receiver,
        resets_receiver,
        out,
    );
    (network, serving)
}

/// The `Hello` of the service: a remote, as the masters expect.
pub fn hello(name: &str) -> Hello {
    Hello::new(name, env!("CARGO_PKG_VERSION"), Role::Remote)
}

enum Inner {
    // Boxed: a live link is large, and the other messages are small.
    Linked(Box<link::Link>),
    Batch(u64, Vec<u8>),
    Closed(u64),
}

/// A master linked now.
struct Linked {
    fingerprint: String,
    sender: LinkSender,
}

/// What a batch does to who drives: whether it carries an `Enter` (a session
/// starts), and whether it ends on a `Leave` (the session is over).
fn edges(batch: &[u8]) -> (bool, bool) {
    let enters = lbm_ipc::input::decode(batch).is_ok_and(|events| {
        events
            .iter()
            .any(|event| matches!(event, lbm_ipc::input::InputEvent::Enter { .. }))
    });
    (enters, lbm_ipc::input::crossing(batch) == Some(false))
}

fn say_reset(sender: &LinkSender) {
    let sender = sender.clone();
    tokio::spawn(async move {
        let _ = sender.control(&Control::InputReset).await;
    });
}

async fn serve(
    bind: SocketAddr,
    identity: Identity,
    hello: Hello,
    options: link::Options,
    mut trust: watch::Receiver<TrustStore>,
    mut resets: mpsc::UnboundedReceiver<u64>,
    out: std::sync::mpsc::Sender<Stream>,
) {
    let (inner, mut inner_rx) = mpsc::unbounded_channel::<Inner>();
    let mut listener: Option<link::Listener> = None;
    let mut links: std::collections::HashMap<u64, Linked> = std::collections::HashMap::new();
    let mut driving: Option<u64> = None;
    let mut next_id = 0u64;
    // The first look at the store opens the port if it approves someone.
    let mut trust_changed = true;
    loop {
        if std::mem::take(&mut trust_changed) {
            let store = trust.borrow_and_update().clone();
            if store.is_empty() {
                if listener.take().is_some() {
                    let _ = out.send(Stream::Listening(None));
                }
            } else if listener.is_none() {
                match link::listen(bind, &identity, hello.clone(), options.clone()).await {
                    Ok(opened) => {
                        let _ = out.send(Stream::Listening(Some(opened.local_addr().port())));
                        listener = Some(opened);
                    }
                    Err(error) => eprintln!("[lbm-service] cannot listen on {bind}: {error}"),
                }
            }
            // Withdrawn: closed at once; the end of its stream follows from its reader.
            for linked in links.values() {
                if !store.allows(&linked.fingerprint) {
                    let sender = linked.sender.clone();
                    tokio::spawn(async move { sender.close("no longer approved").await });
                }
            }
        }
        tokio::select! {
            changed = trust.changed() => match changed {
                Ok(()) => trust_changed = true,
                // The handle is gone: the service is stopping.
                Err(_) => break,
            },
            incoming = async {
                match &mut listener {
                    Some(listener) => listener.next().await,
                    None => std::future::pending().await,
                }
            } => match incoming {
                Some(Ok(pending)) => {
                    let fingerprint = pending.peer().fingerprint.clone();
                    if trust.borrow().allows(&fingerprint) {
                        let inner = inner.clone();
                        tokio::spawn(async move {
                            match pending.accept().await {
                                Ok(link) => {
                                    let _ = inner.send(Inner::Linked(Box::new(link)));
                                }
                                Err(error) => eprintln!(
                                    "[lbm-service] no link with master {fingerprint}: {error}"
                                ),
                            }
                        });
                    } else {
                        eprintln!("[lbm-service] master {fingerprint} is not approved: refused");
                        tokio::spawn(pending.refuse("not approved on this machine"));
                    }
                }
                Some(Err(error)) => eprintln!("[lbm-service] an incoming connection failed: {error}"),
                None => listener = None,
            },
            Some(message) = inner_rx.recv() => match message {
                Inner::Linked(link) => {
                    let (peer, sender, mut events) = (*link).into_parts();
                    // Approved when it knocked; maybe not any more.
                    if !trust.borrow().allows(&peer.fingerprint) {
                        tokio::spawn(async move { sender.close("no longer approved").await });
                        continue;
                    }
                    next_id += 1;
                    let id = next_id;
                    eprintln!("[lbm-service] master {} ({}) linked", peer.hello.name, peer.fingerprint);
                    links.insert(id, Linked { fingerprint: peer.fingerprint, sender });
                    let inner = inner.clone();
                    tokio::spawn(async move {
                        while let Some(event) = events.recv().await {
                            match event {
                                Event::Input(batch) => {
                                    if inner.send(Inner::Batch(id, batch)).is_err() {
                                        return;
                                    }
                                }
                                Event::Closed(_) => break,
                                Event::Control(_) | Event::Bulk(_) => {}
                            }
                        }
                        let _ = inner.send(Inner::Closed(id));
                    });
                }
                Inner::Batch(id, batch) => {
                    if !links.contains_key(&id) {
                        continue;
                    }
                    let (enters, leaves) = edges(&batch);
                    if enters && driving != Some(id) {
                        // A session of another master starts: the one driving is done.
                        if let Some(previous) = driving.take() {
                            let _ = out.send(Stream::Ended { link: previous });
                            if let Some(linked) = links.get(&previous) {
                                say_reset(&linked.sender);
                            }
                        }
                        driving = Some(id);
                    }
                    if driving == Some(id) {
                        if out.send(Stream::Batch { link: id, batch }).is_err() {
                            break;
                        }
                        if leaves {
                            driving = None;
                        }
                    }
                }
                Inner::Closed(id) => {
                    links.remove(&id);
                    if driving == Some(id) {
                        driving = None;
                        let _ = out.send(Stream::Ended { link: id });
                    }
                }
            },
            Some(id) = resets.recv() => {
                if let Some(linked) = links.get(&id) {
                    say_reset(&linked.sender);
                }
                if driving == Some(id) {
                    driving = None;
                }
            }
        }
    }
    for (_, linked) in links.drain() {
        let _ = linked.sender.close("the service stops").await;
    }
    if listener.take().is_some() {
        let _ = out.send(Stream::Listening(None));
    }
}

#[cfg(test)]
mod tests;
