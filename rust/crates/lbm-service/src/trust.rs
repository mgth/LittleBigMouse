//! The masters an administrator of this machine allowed to drive its secure desktop
//! and the windows above its users (docs/multi-pc-service-windows.md, § 1).
//!
//! One file, `trusted-masters.json`, in a folder only SYSTEM and Administrators may
//! change: being listed there is the whole of the trust. The service reads it; only
//! `lbm-service trust` / `untrust`, run elevated, write it. A master is named by the
//! fingerprint of its certificate, the `MachineId` its own page Machines shows, so
//! that the administrator can compare the two.

use serde::{Deserialize, Serialize};

/// The file, in the service's folder under `%ProgramData%`.
pub const FILE: &str = "trusted-masters.json";

/// One approved master.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Master {
    /// The SHA-256 of its certificate, lowercase hex.
    pub machine_id: String,
    /// The name shown when it was approved: a reminder for the administrator, never
    /// used to decide anything.
    #[serde(default)]
    pub name: String,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct TrustStore {
    #[serde(default)]
    masters: Vec<Master>,
}

/// What [`TrustStore::trust`] did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Trusted {
    Added,
    /// Already there: its name is the new one.
    Renamed,
}

impl TrustStore {
    /// Reads the file's text. A file that does not parse, or lists a fingerprint that
    /// is not one, is refused whole: a store half understood is not a store.
    pub fn parse(text: &str) -> Result<Self, String> {
        let store: Self =
            serde_json::from_str(text).map_err(|e| format!("{FILE} does not parse: {e}"))?;
        if let Some(bad) = store
            .masters
            .iter()
            .find(|m| !is_fingerprint(&m.machine_id))
        {
            return Err(format!("{FILE}: {:?} is not a fingerprint", bad.machine_id));
        }
        Ok(store)
    }

    pub fn to_text(&self) -> String {
        serde_json::to_string_pretty(self).expect("plain strings serialise")
    }

    pub fn masters(&self) -> &[Master] {
        &self.masters
    }

    pub fn is_empty(&self) -> bool {
        self.masters.is_empty()
    }

    /// Whether the certificate with this fingerprint belongs to an approved master.
    pub fn allows(&self, machine_id: &str) -> bool {
        self.masters.iter().any(|m| m.machine_id == machine_id)
    }

    /// Approves a master. The fingerprint is taken in any case and with or without
    /// separators, as a user may copy it, and kept in the one form connections use.
    pub fn trust(&mut self, machine_id: &str, name: &str) -> Result<Trusted, String> {
        let machine_id = normalise(machine_id)?;
        if let Some(known) = self.masters.iter_mut().find(|m| m.machine_id == machine_id) {
            known.name = name.to_owned();
            return Ok(Trusted::Renamed);
        }
        self.masters.push(Master {
            machine_id,
            name: name.to_owned(),
        });
        Ok(Trusted::Added)
    }

    /// Withdraws a master; `false` when it was not there.
    pub fn untrust(&mut self, machine_id: &str) -> Result<bool, String> {
        let machine_id = normalise(machine_id)?;
        let before = self.masters.len();
        self.masters.retain(|m| m.machine_id != machine_id);
        Ok(self.masters.len() != before)
    }
}

/// The fingerprint in its one form: 64 lowercase hex digits. Spaces and colons are
/// dropped, as a copy from a page may carry them.
pub fn normalise(machine_id: &str) -> Result<String, String> {
    let digits: String = machine_id
        .chars()
        .filter(|c| !matches!(c, ' ' | ':' | '-'))
        .map(|c| c.to_ascii_lowercase())
        .collect();
    if is_fingerprint(&digits) {
        Ok(digits)
    } else {
        Err(format!(
            "{machine_id:?} is not a machine id (64 hex digits)"
        ))
    }
}

fn is_fingerprint(text: &str) -> bool {
    text.len() == 64 && text.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    const B: &str = "fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210";

    #[test]
    fn a_trusted_master_is_allowed_and_another_is_not() {
        let mut store = TrustStore::default();
        assert_eq!(store.trust(A, "linux"), Ok(Trusted::Added));
        assert!(store.allows(A));
        assert!(!store.allows(B));
    }

    #[test]
    fn a_fingerprint_is_kept_in_its_one_form() {
        let mut store = TrustStore::default();
        let spaced: String = A
            .to_uppercase()
            .as_bytes()
            .chunks(2)
            .map(|pair| std::str::from_utf8(pair).unwrap())
            .collect::<Vec<_>>()
            .join(":");
        store.trust(&spaced, "linux").unwrap();
        assert!(store.allows(A));
    }

    #[test]
    fn trusting_twice_keeps_one_entry_with_the_new_name() {
        let mut store = TrustStore::default();
        store.trust(A, "old").unwrap();
        assert_eq!(store.trust(A, "new"), Ok(Trusted::Renamed));
        assert_eq!(store.masters().len(), 1);
        assert_eq!(store.masters()[0].name, "new");
    }

    #[test]
    fn something_that_is_not_a_fingerprint_is_refused() {
        let mut store = TrustStore::default();
        assert!(store.trust("abc", "x").is_err());
        assert!(store.trust(&A.replace('0', "g"), "x").is_err());
        assert!(store.untrust("abc").is_err());
        assert!(store.is_empty());
    }

    #[test]
    fn untrust_withdraws_and_says_whether_it_did() {
        let mut store = TrustStore::default();
        store.trust(A, "a").unwrap();
        assert_eq!(store.untrust(B), Ok(false));
        assert_eq!(store.untrust(A), Ok(true));
        assert!(!store.allows(A));
    }

    #[test]
    fn the_file_round_trips() {
        let mut store = TrustStore::default();
        store.trust(A, "linux").unwrap();
        store.trust(B, "laptop").unwrap();
        assert_eq!(TrustStore::parse(&store.to_text()), Ok(store));
    }

    #[test]
    fn a_file_with_one_bad_fingerprint_is_refused_whole() {
        let text = format!(
            r#"{{"Masters":[{{"MachineId":"{A}","Name":"a"}},{{"MachineId":"nope","Name":"b"}}]}}"#
        );
        assert!(TrustStore::parse(&text).is_err());
        assert!(TrustStore::parse("not json").is_err());
    }

    #[test]
    fn an_empty_object_is_an_empty_store() {
        assert_eq!(TrustStore::parse("{}"), Ok(TrustStore::default()));
    }
}
