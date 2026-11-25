//! Discovery in the manager (`lbm_net::discovery`): the machines of the local network
//! that said who they are, shown in the state for the user to pair with — and a paired
//! remote heard of, called on at once, at the address it was heard from.
//!
//! It runs with the manager and with it only: the socket is bound when the mode starts,
//! once the listener is up (a machine nobody can connect to has nothing to announce),
//! and dropped when it stops. The datagrams sent are a query when it starts, sent again
//! twice in the first seconds in case one was lost, an answer to every query heard, a
//! query when the user asks to look again, and a `Bye` when it stops. No timer
//! otherwise.
//!
//! What is heard is never trusted: a machine is only listed. Pairing with it is the
//! user's `PeerConnect` to the address shown, and the fingerprint is checked on both
//! machines as for an address typed by hand. A paired remote heard of at another address
//! is dialled there as that fingerprint, so that its stored address only changes once
//! the link proves it is the machine paired ([`Actor::admit`] stores it).

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use lbm_net::discovery::{Beacon, Config, Discovery, Kind};
use lbm_net::wire::Role;

use super::Actor;
use crate::peers::DiscoveredPeer;

/// When the first query is sent again, after it: a datagram lost on a busy Wi-Fi would
/// otherwise leave this machine unseen until the others look again.
const QUERY_REPEATS: [Duration; 2] = [Duration::from_secs(1), Duration::from_secs(3)];

/// The most addresses kept per machine ([`Seen::heard_at`]).
const MAX_ADDRESSES: usize = 8;

/// The most machines listed. A network flooded with made-up fingerprints fills the
/// list, never the memory.
pub(super) const MAX_SEEN: usize = 64;

/// A machine heard of.
pub(super) struct Seen {
    name: String,
    /// The address of its node: where its datagram came from, at the port it gave.
    address: SocketAddr,
    /// Every address it was heard from. A machine on several networks — a LAN and a
    /// container bridge, wired and Wi-Fi — is heard on each: the address shown changes
    /// for one never heard before (a lease that changed), not at every datagram.
    heard_at: Vec<SocketAddr>,
    /// The order it was first heard in, for a list that does not jump around.
    serial: u64,
}

/// Discovery's part of the manager.
#[derive(Default)]
pub(super) struct Part {
    socket: Option<Arc<Discovery>>,
    /// Why it does not run, when it was meant to.
    error: Option<String>,
    seen: std::collections::HashMap<String, Seen>,
    next_serial: u64,
}

impl Part {
    /// The socket, for the manager's loop to read from.
    pub(super) fn socket(&self) -> Option<Arc<Discovery>> {
        self.socket.clone()
    }
}

impl Actor {
    /// What this machine says of itself; `None` while it does not listen.
    fn beacon(&self, kind: Kind) -> Option<Beacon> {
        Some(Beacon {
            kind,
            machine_id: self.identity.fingerprint().to_owned(),
            name: self.name.clone(),
            port: self.listen_address?.port(),
        })
    }

    /// Binds the socket and asks who is there, when there is a configuration and the
    /// listener is up.
    pub(super) fn discovery_start(&mut self, config: Option<&Config>) {
        let Some(config) = config else {
            return;
        };
        let Some(query) = self.beacon(Kind::Query) else {
            return;
        };
        let socket = match Discovery::bind(config) {
            Ok(socket) => Arc::new(socket),
            Err(error) => {
                let error = format!(
                    "cannot listen for the other machines on UDP {}: {error}",
                    config.bind.port()
                );
                eprintln!("[lbm-agent] multi-PC: {error}");
                self.discovery.error = Some(error);
                return;
            }
        };
        match socket.broadcast(&query) {
            Ok(networks) => eprintln!(
                "[lbm-agent] multi-PC: discovery on UDP {}, asked {networks} network(s) who is there",
                config.bind.port()
            ),
            Err(error) => {
                eprintln!("[lbm-agent] multi-PC: discovery cannot ask the network: {error}")
            }
        }
        let repeat = socket.clone();
        self.spawn(async move {
            let mut waited = Duration::ZERO;
            for after in QUERY_REPEATS {
                tokio::time::sleep(after - waited).await;
                waited = after;
                let _ = repeat.broadcast(&query);
            }
        });
        self.discovery.socket = Some(socket);
    }

    /// The socket could not be read from: discovery stops, the rest goes on.
    pub(super) fn discovery_failed(&mut self, error: std::io::Error) {
        eprintln!("[lbm-agent] multi-PC: discovery stopped: {error}");
        self.discovery.socket = None;
        self.discovery.seen.clear();
        self.discovery.error = Some(format!("discovery stopped: {error}"));
    }

    /// The user asks who is there again: the machines that are not paired are listed
    /// anew from the answers, so that one gone without a word leaves the list.
    pub(super) fn discover_again(&mut self) -> Result<(), String> {
        let socket = self.discovery.socket.clone().ok_or_else(|| {
            self.discovery
                .error
                .clone()
                .unwrap_or_else(|| "discovery does not run".to_owned())
        })?;
        let query = self
            .beacon(Kind::Query)
            .ok_or("this machine does not listen")?;
        let store = &self.store;
        self.discovery
            .seen
            .retain(|machine_id, _| store.get(machine_id).is_some());
        socket
            .broadcast(&query)
            .map(|_| ())
            .map_err(|error| format!("cannot ask the network: {error}"))
    }

    /// Says `Bye`, when the mode stops.
    pub(super) fn discovery_stop(&mut self) {
        if let (Some(socket), Some(bye)) = (self.discovery.socket.take(), self.beacon(Kind::Bye)) {
            let _ = socket.broadcast(&bye);
        }
        self.discovery.seen.clear();
    }

    /// A datagram from another machine.
    pub(super) fn heard(&mut self, beacon: Beacon, from: SocketAddr) {
        if beacon.machine_id == self.identity.fingerprint() {
            // This machine's own broadcast, looped back.
            return;
        }
        if beacon.kind == Kind::Bye {
            if let Some(seen) = self.discovery.seen.remove(&beacon.machine_id) {
                eprintln!(
                    "[lbm-agent] multi-PC: {} ({}) left the network",
                    seen.name, beacon.machine_id
                );
            }
            return;
        }
        if beacon.kind == Kind::Query {
            if let (Some(socket), Some(announce)) =
                (&self.discovery.socket, self.beacon(Kind::Announce))
            {
                if let Err(error) = socket.send_to(&announce, from) {
                    eprintln!("[lbm-agent] multi-PC: cannot answer {from}: {error}");
                }
            }
        }
        let address = SocketAddr::new(from.ip(), beacon.port);
        let known = self.discovery.seen.len();
        match self.discovery.seen.get_mut(&beacon.machine_id) {
            Some(seen) => {
                if !seen.heard_at.contains(&address) {
                    if seen.heard_at.len() == MAX_ADDRESSES {
                        seen.heard_at.remove(0);
                    }
                    seen.heard_at.push(address);
                    seen.address = address;
                    eprintln!(
                        "[lbm-agent] multi-PC: {} ({}) is now at {address}",
                        beacon.name, beacon.machine_id
                    );
                }
                seen.name = beacon.name.clone();
            }
            None if known >= MAX_SEEN => return,
            None => {
                eprintln!(
                    "[lbm-agent] multi-PC: {} ({}) is on the network at {address}",
                    beacon.name, beacon.machine_id
                );
                self.discovery.next_serial += 1;
                self.discovery.seen.insert(
                    beacon.machine_id.clone(),
                    Seen {
                        name: beacon.name.clone(),
                        address,
                        heard_at: vec![address],
                        serial: self.discovery.next_serial,
                    },
                );
            }
        }
        // Called where this datagram came from, not at the address shown: that one
        // stays put for a machine heard on several networks, and may be one it left —
        // a machine back at an address heard before (A, then B, then A with B gone)
        // would be called at B forever. The link proves the address before it is kept.
        self.reach_paired(&beacon.machine_id, address);
    }

    /// A paired remote that is not linked is called on now that it said it is there:
    /// at the address heard when the one stored is an IP address that differs (a lease
    /// that changed), at the stored one otherwise — a name the user typed outlives
    /// leases, and resolves to wherever the machine is.
    fn reach_paired(&mut self, machine_id: &str, address: SocketAddr) {
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
        let stored_ip = paired
            .address
            .parse::<SocketAddr>()
            .map(|stored| stored.ip())
            .ok();
        let moved = stored_ip.is_some() && paired.address != address.to_string();
        if moved {
            eprintln!(
                "[lbm-agent] multi-PC: {machine_id} is heard at {address}, not {}: calling it there",
                paired.address
            );
            self.dial(address.to_string(), Some(machine_id.to_owned()), None);
        } else {
            self.call_back(machine_id);
        }
    }

    /// The machines heard of, in the order they were first heard; `None` while
    /// discovery does not run.
    pub(super) fn discovered(&self) -> Option<Vec<DiscoveredPeer>> {
        self.discovery.socket.as_ref()?;
        let mut seen: Vec<_> = self.discovery.seen.iter().collect();
        seen.sort_by_key(|(_, seen)| seen.serial);
        Some(
            seen.into_iter()
                .map(|(machine_id, seen)| DiscoveredPeer {
                    machine_id: machine_id.clone(),
                    name: seen.name.clone(),
                    address: seen.address.to_string(),
                    paired: self.store.get(machine_id).is_some(),
                })
                .collect(),
        )
    }

    pub(super) fn discovery_error(&self) -> Option<String> {
        self.discovery.error.clone()
    }
}
