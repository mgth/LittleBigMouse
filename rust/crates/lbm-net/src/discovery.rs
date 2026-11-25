//! How the machines of a local network learn of each other without anyone typing an
//! address (docs/multi-pc-etat.md, "Découverte automatique").
//!
//! A node with the mode on keeps one UDP socket on [`DISCOVERY_PORT`]. It says who it
//! is when the mode starts — a [`Kind::Query`], broadcast on every IPv4 network it is
//! on, which also asks the others to say who they are — and answers each query it
//! hears with a [`Kind::Announce`] sent back to whoever asked. When the mode stops it
//! says [`Kind::Bye`]. Nothing else is sent, and nothing is sent on a timer: a machine
//! that went away without a word stays listed until it is asked again.
//!
//! A datagram carries the machine's name, its fingerprint and the TCP port its node
//! listens on, nothing more. None of that is a secret: the fingerprint is what its
//! certificate shows anyone who connects. Nor is any of it trusted: discovery only
//! proposes a machine, and linking with it is the pairing of [`crate::link`], confirmed
//! on both machines against the fingerprint, which a lying datagram cannot fake.
//!
//! Why a datagram of our own rather than mDNS/DNS-SD: docs/multi-pc-etat.md.

use std::io;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use serde::{Deserialize, Serialize};
use socket2::{Domain, Protocol, Socket, Type};
use tokio::net::UdpSocket;

/// The UDP port every node listens on for the others' datagrams. The same number as the
/// TCP port of the links ([`crate::DEFAULT_PORT`]), for one firewall rule to name; it is
/// fixed, whatever TCP port a node was told to use, since a machine has to know where to
/// ask before it knows anything about the others.
pub const DISCOVERY_PORT: u16 = 24817;

/// What a datagram starts by saying it is, so that anything else sent to the port is
/// told apart at once.
const MAGIC: &str = "LittleBigMouse";

/// The version of the datagram. A newer one is read as far as this one understands it.
pub const VERSION: u32 = 1;

/// The largest datagram read or written: a name, a fingerprint and a port fit many times
/// over, and anything larger is not ours.
pub const MAX_DATAGRAM: usize = 1024;

/// The longest name sent, in characters; a longer one is cut.
pub const MAX_NAME: usize = 64;

/// What a datagram says.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Kind {
    /// "Here I am; who else is there?" Sent to everyone when the mode starts.
    Query,
    /// "Here I am", in answer to a query, to the machine that asked.
    Announce,
    /// "I am going": the mode stops on that machine.
    Bye,
}

/// One machine saying who it is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Beacon {
    pub kind: Kind,
    /// Its certificate's fingerprint (`identity::fingerprint`).
    pub machine_id: String,
    /// What it is called.
    pub name: String,
    /// The TCP port its node listens on.
    pub port: u16,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Datagram {
    lbm: String,
    v: u32,
    kind: Kind,
    machine_id: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    port: u16,
}

impl Beacon {
    /// The datagram, with the name cut to [`MAX_NAME`].
    pub fn encode(&self) -> Vec<u8> {
        serde_json::to_vec(&Datagram {
            lbm: MAGIC.to_owned(),
            v: VERSION,
            kind: self.kind,
            machine_id: self.machine_id.clone(),
            name: self.name.chars().take(MAX_NAME).collect(),
            port: self.port,
        })
        .expect("a datagram always serializes")
    }

    /// What a datagram says, if it is one of ours and makes sense: a fingerprint as
    /// [`crate::identity::fingerprint`] writes it, and a port for anything but `Bye`.
    pub fn decode(bytes: &[u8]) -> Option<Beacon> {
        if bytes.len() > MAX_DATAGRAM {
            return None;
        }
        let datagram: Datagram = serde_json::from_slice(bytes).ok()?;
        let fingerprint = datagram.machine_id.len() == 64
            && datagram
                .machine_id
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
        if datagram.lbm != MAGIC || datagram.v == 0 || !fingerprint {
            return None;
        }
        if datagram.kind != Kind::Bye && datagram.port == 0 {
            return None;
        }
        Some(Beacon {
            kind: datagram.kind,
            machine_id: datagram.machine_id,
            // Control characters would only garble the page that shows it.
            name: datagram
                .name
                .chars()
                .filter(|c| !c.is_control())
                .take(MAX_NAME)
                .collect(),
            port: datagram.port,
        })
    }
}

/// Where the datagrams sent to everyone go.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Targets {
    /// The broadcast address of every IPv4 network this machine is on, read anew at
    /// every send (a network joined since the mode started is reached), at the
    /// discovery port.
    Lan,
    /// These addresses: tests, on loopback.
    Fixed(Vec<SocketAddr>),
}

/// Where a node listens for datagrams and where it sends them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Config {
    pub bind: SocketAddr,
    pub targets: Targets,
}

impl Config {
    /// The real thing: every interface, [`DISCOVERY_PORT`], every local network.
    pub fn lan() -> Config {
        Config {
            bind: SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), DISCOVERY_PORT),
            targets: Targets::Lan,
        }
    }
}

/// The socket, while the mode is on.
#[derive(Debug)]
pub struct Discovery {
    socket: UdpSocket,
    targets: Targets,
    port: u16,
}

impl Discovery {
    /// Binds the socket. Inside a Tokio runtime.
    ///
    /// The address is shared (`SO_REUSEADDR`): another agent on the same machine — a
    /// second session on Windows — listens there too, and every broadcast reaches both.
    pub fn bind(config: &Config) -> io::Result<Discovery> {
        let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
        socket.set_reuse_address(true)?;
        socket.set_broadcast(true)?;
        socket.set_nonblocking(true)?;
        socket.bind(&config.bind.into())?;
        let socket = UdpSocket::from_std(socket.into())?;
        Ok(Discovery {
            port: config.bind.port(),
            socket,
            targets: config.targets.clone(),
        })
    }

    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.socket.local_addr()
    }

    /// Where a datagram to everyone goes now.
    pub fn targets(&self) -> Vec<SocketAddr> {
        match &self.targets {
            Targets::Fixed(addresses) => addresses.clone(),
            Targets::Lan => lan_broadcasts(self.port),
        }
    }

    /// Sends `beacon` to everyone, without waiting: a socket with no room for a
    /// datagram drops it, as the network would. Says how many networks it went out
    /// to, or why it went out to none.
    pub fn broadcast(&self, beacon: &Beacon) -> io::Result<usize> {
        let bytes = beacon.encode();
        let mut sent = 0;
        let mut last_error = None;
        for target in self.targets() {
            match self.socket.try_send_to(&bytes, target) {
                Ok(_) => sent += 1,
                Err(error) => last_error = Some(error),
            }
        }
        match (sent, last_error) {
            (0, Some(error)) => Err(error),
            (0, None) => Err(io::Error::other("no IPv4 network to broadcast on")),
            (sent, _) => Ok(sent),
        }
    }

    /// Sends `beacon` to one machine, without waiting.
    pub fn send_to(&self, beacon: &Beacon, to: SocketAddr) -> io::Result<()> {
        self.socket.try_send_to(&beacon.encode(), to).map(|_| ())
    }

    /// The next datagram of ours, and where it came from. What is not one is skipped.
    /// Cancel-safe.
    pub async fn recv(&self) -> io::Result<(Beacon, SocketAddr)> {
        let mut buffer = [0u8; MAX_DATAGRAM + 1];
        loop {
            match self.socket.recv_from(&mut buffer).await {
                Ok((length, from)) => {
                    if let Some(beacon) = Beacon::decode(&buffer[..length]) {
                        return Ok((beacon, from));
                    }
                }
                // Windows reports an ICMP "port unreachable" that came back for an
                // earlier datagram as an error of the next receive (WSAECONNRESET), and
                // Linux may do the same with ECONNREFUSED: nothing wrong with this socket.
                Err(error)
                    if matches!(
                        error.kind(),
                        io::ErrorKind::ConnectionReset | io::ErrorKind::ConnectionRefused
                    ) => {}
                Err(error) => return Err(error),
            }
        }
    }
}

/// The broadcast address of every IPv4 network of this machine but loopback, at `port`.
/// The limited broadcast (255.255.255.255) leaves by one interface only, the one of the
/// default route, so each network is named; it is the fallback when none can be read.
fn lan_broadcasts(port: u16) -> Vec<SocketAddr> {
    let mut targets: Vec<SocketAddr> = if_addrs::get_if_addrs()
        .unwrap_or_default()
        .into_iter()
        .filter(|interface| !interface.is_loopback())
        .filter_map(|interface| match interface.addr {
            if_addrs::IfAddr::V4(v4) => Some(
                v4.broadcast
                    .unwrap_or_else(|| directed_broadcast(v4.ip, v4.netmask)),
            ),
            if_addrs::IfAddr::V6(_) => None,
        })
        .filter(|broadcast| !broadcast.is_loopback() && !broadcast.is_unspecified())
        .map(|broadcast| SocketAddr::new(IpAddr::V4(broadcast), port))
        .collect();
    targets.sort();
    targets.dedup();
    if targets.is_empty() {
        targets.push(SocketAddr::new(IpAddr::V4(Ipv4Addr::BROADCAST), port));
    }
    targets
}

/// The broadcast address of the network of `ip` under `netmask`.
fn directed_broadcast(ip: Ipv4Addr, netmask: Ipv4Addr) -> Ipv4Addr {
    Ipv4Addr::from(u32::from(ip) | !u32::from(netmask))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    const ID: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    fn beacon(kind: Kind) -> Beacon {
        Beacon {
            kind,
            machine_id: ID.to_owned(),
            name: "tower".to_owned(),
            port: 24900,
        }
    }

    #[test]
    fn a_beacon_reads_back_as_it_was_sent() {
        for kind in [Kind::Query, Kind::Announce, Kind::Bye] {
            let sent = beacon(kind);
            assert_eq!(Beacon::decode(&sent.encode()), Some(sent));
        }
        let text = String::from_utf8(beacon(Kind::Query).encode()).unwrap();
        for key in [
            "\"Lbm\":\"LittleBigMouse\"",
            "\"V\":1",
            "\"Kind\":\"Query\"",
            "\"Port\":24900",
        ] {
            assert!(text.contains(key), "{key} in {text}");
        }
    }

    #[test]
    fn a_datagram_says_only_the_name_the_fingerprint_and_the_port() {
        let value: serde_json::Value =
            serde_json::from_slice(&beacon(Kind::Query).encode()).unwrap();
        let mut keys: Vec<_> = value.as_object().unwrap().keys().cloned().collect();
        keys.sort();
        assert_eq!(keys, ["Kind", "Lbm", "MachineId", "Name", "Port", "V"]);
    }

    #[test]
    fn what_is_not_ours_or_makes_no_sense_is_not_read() {
        let mut long = beacon(Kind::Query);
        long.name = "x".repeat(500);
        assert_eq!(
            Beacon::decode(&long.encode()).unwrap().name.chars().count(),
            MAX_NAME
        );
        let reads = |json: serde_json::Value| Beacon::decode(json.to_string().as_bytes());
        let good = serde_json::json!({
            "Lbm": "LittleBigMouse", "V": 1, "Kind": "Query",
            "MachineId": ID, "Name": "a\u{7}b", "Port": 1
        });
        assert_eq!(reads(good.clone()).unwrap().name, "ab");
        // A newer version, with a field this one does not know, still reads.
        let mut newer = good.clone();
        newer["V"] = 2.into();
        newer["Extra"] = true.into();
        assert!(reads(newer).is_some());
        for (key, bad) in [
            ("Lbm", serde_json::json!("Synergy")),
            ("V", 0.into()),
            ("Kind", "Hello".into()),
            ("MachineId", "ABCDEF".into()),
            ("MachineId", ID.to_uppercase().into()),
            ("Port", 0.into()),
        ] {
            let mut datagram = good.clone();
            datagram[key] = bad;
            assert_eq!(reads(datagram.clone()), None, "{datagram}");
        }
        assert_eq!(Beacon::decode(b"not json"), None);
        assert_eq!(Beacon::decode(&vec![b' '; MAX_DATAGRAM + 1]), None);
        // A Bye needs no port.
        let mut bye = good;
        bye["Kind"] = "Bye".into();
        bye["Port"] = 0.into();
        assert!(reads(bye).is_some());
    }

    #[test]
    fn a_network_broadcasts_on_its_highest_address() {
        assert_eq!(
            directed_broadcast(
                "192.168.4.99".parse().unwrap(),
                "255.255.255.0".parse().unwrap()
            ),
            Ipv4Addr::new(192, 168, 4, 255)
        );
        assert_eq!(
            directed_broadcast("10.1.2.3".parse().unwrap(), "255.255.0.0".parse().unwrap()),
            Ipv4Addr::new(10, 1, 255, 255)
        );
        let targets = lan_broadcasts(DISCOVERY_PORT);
        assert!(!targets.is_empty());
        assert!(targets
            .iter()
            .all(|t| t.port() == DISCOVERY_PORT && !t.ip().is_loopback()));
    }

    #[tokio::test]
    async fn two_sockets_hear_each_other_on_loopback() {
        let loopback = |port| SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port);
        let a = Discovery::bind(&Config {
            bind: loopback(0),
            targets: Targets::Fixed(Vec::new()),
        })
        .unwrap();
        let b = Discovery::bind(&Config {
            bind: loopback(0),
            targets: Targets::Fixed(vec![a.local_addr().unwrap()]),
        })
        .unwrap();
        // Something that is not ours first: skipped, not an error.
        b.socket
            .send_to(b"hello", a.local_addr().unwrap())
            .await
            .unwrap();
        assert_eq!(b.broadcast(&beacon(Kind::Query)).unwrap(), 1);
        let (heard, from) = tokio::time::timeout(Duration::from_secs(5), a.recv())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(heard, beacon(Kind::Query));
        assert_eq!(from, b.local_addr().unwrap());
        a.send_to(&beacon(Kind::Announce), from).unwrap();
        let (answer, _) = tokio::time::timeout(Duration::from_secs(5), b.recv())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(answer.kind, Kind::Announce);
    }

    #[test]
    fn broadcasting_to_nobody_says_so() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            let lonely = Discovery::bind(&Config {
                bind: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0),
                targets: Targets::Fixed(Vec::new()),
            })
            .unwrap();
            assert!(lonely.broadcast(&beacon(Kind::Bye)).is_err());
        });
    }
}
