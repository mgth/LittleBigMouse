//! The machines the user paired with this one, kept in `peers.json`.
//!
//! Being in this file is what "trusted" means: a connection whose certificate
//! fingerprint is a `MachineId` here is accepted without asking the user again.

use std::io::{self, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::identity::{private_options, temporary_path};
use crate::wire::Role;

/// The file, in the configuration directory, that lists the paired machines.
pub const PEERS_FILE: &str = "peers.json";

/// One paired machine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct PairedPeer {
    /// The fingerprint of its certificate.
    pub machine_id: String,
    #[serde(default)]
    pub name: String,
    /// Where to reach it, `host:port`, as the user typed it; empty for a machine that
    /// only ever connected to this one.
    #[serde(default)]
    pub address: String,
    /// The role that machine plays on the link with this one.
    pub role: Role,
}

/// The pairing store: read once, written whole on every change.
#[derive(Debug)]
pub struct PeerStore {
    directory: PathBuf,
    peers: Vec<PairedPeer>,
}

impl PeerStore {
    /// The store kept in `directory`.
    ///
    /// A file that cannot be read as a list of peers counts as no file: pairing again
    /// is a minute's work, while refusing to start the mode over a damaged file would
    /// leave the user nothing to do. The damaged file is replaced at the next change.
    pub fn load(directory: &Path) -> PeerStore {
        let peers = std::fs::read(directory.join(PEERS_FILE))
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        PeerStore {
            directory: directory.to_owned(),
            peers,
        }
    }

    pub fn peers(&self) -> &[PairedPeer] {
        &self.peers
    }

    /// The paired machine with this fingerprint, if it is one.
    pub fn get(&self, machine_id: &str) -> Option<&PairedPeer> {
        self.peers.iter().find(|peer| peer.machine_id == machine_id)
    }

    /// Adds a machine, or replaces what is known of it, and saves.
    pub fn insert(&mut self, peer: PairedPeer) -> io::Result<()> {
        let mut peers = self.peers.clone();
        match peers.iter_mut().find(|p| p.machine_id == peer.machine_id) {
            Some(known) => *known = peer,
            None => peers.push(peer),
        }
        self.commit(peers)
    }

    /// Forgets a machine and saves. Returns whether it was known.
    pub fn remove(&mut self, machine_id: &str) -> io::Result<bool> {
        let mut peers = self.peers.clone();
        peers.retain(|peer| peer.machine_id != machine_id);
        if peers.len() == self.peers.len() {
            return Ok(false);
        }
        self.commit(peers)?;
        Ok(true)
    }

    /// Memory follows only once the file is on disk, so that the two never disagree
    /// about who is trusted.
    fn commit(&mut self, peers: Vec<PairedPeer>) -> io::Result<()> {
        let bytes = serde_json::to_vec_pretty(&peers).map_err(io::Error::other)?;
        std::fs::create_dir_all(&self.directory)?;
        let temporary = temporary_path(&self.directory, PEERS_FILE);
        let result = (|| {
            let mut file = private_options()
                .create(true)
                .truncate(true)
                .open(&temporary)?;
            file.write_all(&bytes)?;
            file.sync_all()?;
            drop(file);
            std::fs::rename(&temporary, self.directory.join(PEERS_FILE))
        })();
        if result.is_err() {
            let _ = std::fs::remove_file(&temporary);
        }
        result?;
        self.peers = peers;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn peer(id: &str, name: &str) -> PairedPeer {
        PairedPeer {
            machine_id: id.into(),
            name: name.into(),
            address: "192.168.1.30:24817".into(),
            role: Role::Remote,
        }
    }

    #[test]
    fn paired_machines_survive_a_restart() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = PeerStore::load(dir.path());
        assert!(store.peers().is_empty());
        store.insert(peer("aa", "laptop")).unwrap();
        store.insert(peer("bb", "tower")).unwrap();
        store.insert(peer("aa", "renamed")).unwrap();

        let reopened = PeerStore::load(dir.path());
        assert_eq!(
            reopened.peers(),
            [peer("aa", "renamed"), peer("bb", "tower")]
        );
        assert_eq!(reopened.get("bb").unwrap().name, "tower");
        assert!(reopened.get("cc").is_none());

        let text = std::fs::read_to_string(dir.path().join(PEERS_FILE)).unwrap();
        for key in [
            "\"MachineId\"",
            "\"Name\"",
            "\"Address\"",
            "\"Role\": \"Remote\"",
        ] {
            assert!(text.contains(key), "{key} in {text}");
        }

        let mut store = reopened;
        assert!(store.remove("aa").unwrap());
        assert!(!store.remove("aa").unwrap());
        assert_eq!(PeerStore::load(dir.path()).peers(), [peer("bb", "tower")]);

        let names: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        assert_eq!(names, [PEERS_FILE], "no temporary file left behind");
    }

    #[test]
    fn a_corrupt_store_counts_as_empty_and_is_replaced() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(PEERS_FILE), "{ not json").unwrap();
        let mut store = PeerStore::load(dir.path());
        assert!(store.peers().is_empty());
        store.insert(peer("aa", "laptop")).unwrap();
        assert_eq!(PeerStore::load(dir.path()).peers(), [peer("aa", "laptop")]);
    }
}
