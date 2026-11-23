//! This machine's identity: a key and a self-signed certificate, made the first time
//! the mode runs and kept from then on. The fingerprint of the certificate is the
//! machine id every peer pins.

use std::io::{self, Write};
use std::path::{Path, PathBuf};

use rustls::pki_types::pem::PemObject;
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use sha2::{Digest, Sha256};

/// The file, in the configuration directory, that holds the certificate and its key.
pub const IDENTITY_FILE: &str = "peer-identity.pem";

/// The name written in the certificate. Nobody checks it — trust is the fingerprint —
/// but a certificate with no name at all is refused by some tooling a user might point
/// at it.
const SUBJECT: &str = "littlebigmouse";

/// The certificate and private key this machine presents to its peers.
pub struct Identity {
    certificate: CertificateDer<'static>,
    key: PrivateKeyDer<'static>,
    fingerprint: String,
}

impl Clone for Identity {
    fn clone(&self) -> Self {
        Self {
            certificate: self.certificate.clone(),
            key: self.key.clone_key(),
            fingerprint: self.fingerprint.clone(),
        }
    }
}

impl std::fmt::Debug for Identity {
    // The key stays out of logs whatever the caller prints.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Identity")
            .field("fingerprint", &self.fingerprint)
            .finish_non_exhaustive()
    }
}

impl Identity {
    /// The identity kept in `directory`, made there if there is none yet.
    ///
    /// A file that exists but cannot be read as an identity is an error, not a reason
    /// to make a new one: a new certificate is a new machine to every peer, and
    /// replacing it silently would turn a damaged file into "every pairing broke for
    /// no reason". The caller can show the error and let the user delete the file.
    pub fn load_or_create(directory: &Path) -> io::Result<Identity> {
        let path = directory.join(IDENTITY_FILE);
        match std::fs::read(&path) {
            Ok(bytes) => return Self::from_pem(&bytes),
            Err(error) if error.kind() != io::ErrorKind::NotFound => return Err(error),
            Err(_) => {}
        }
        std::fs::create_dir_all(directory)?;
        let pem = generate()?;
        publish(directory, &path, pem.as_bytes())?;
        // Read back what is on disk rather than trusting `pem`: if another process
        // made its own identity at the same moment, its file won and is the one both
        // must use.
        Self::from_pem(&std::fs::read(&path)?)
    }

    /// An identity from the PEM text [`load_or_create`](Self::load_or_create) keeps.
    pub fn from_pem(pem: &[u8]) -> io::Result<Identity> {
        let invalid = |what: &str| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("{IDENTITY_FILE}: {what}"),
            )
        };
        let certificate = CertificateDer::from_pem_slice(pem)
            .map_err(|_| invalid("no readable certificate"))?
            .into_owned();
        let key = PrivateKeyDer::from_pem_slice(pem)
            .map_err(|_| invalid("no readable private key"))?
            .clone_key();
        // A key the provider cannot use would only fail later, at the first
        // connection, far from the file that is wrong.
        crate::tls::provider()
            .key_provider
            .load_private_key(key.clone_key())
            .map_err(|_| invalid("the private key is not one this build can use"))?;
        let fingerprint = fingerprint(&certificate);
        Ok(Identity {
            certificate,
            key,
            fingerprint,
        })
    }

    /// The machine id: the SHA-256 of the certificate, lowercase hex.
    pub fn fingerprint(&self) -> &str {
        &self.fingerprint
    }

    pub fn certificate(&self) -> &CertificateDer<'static> {
        &self.certificate
    }

    pub(crate) fn key(&self) -> PrivateKeyDer<'static> {
        self.key.clone_key()
    }
}

/// The SHA-256 of a DER certificate, lowercase hex: how a machine is named and pinned.
pub fn fingerprint(certificate: &[u8]) -> String {
    Sha256::digest(certificate)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn generate() -> io::Result<String> {
    let certified = rcgen::generate_simple_self_signed(vec![SUBJECT.to_owned()])
        .map_err(|error| io::Error::other(format!("cannot make a certificate: {error}")))?;
    Ok(format!(
        "{}{}",
        certified.cert.pem(),
        certified.signing_key.serialize_pem()
    ))
}

/// Puts the file in place whole or not at all, and never over an existing one.
///
/// Written beside its final name, then hard-linked there: unlike a rename, a link
/// fails when the name is taken, so two processes making an identity at once cannot
/// each believe theirs is the one on disk.
fn publish(directory: &Path, path: &Path, contents: &[u8]) -> io::Result<()> {
    let temporary = temporary_path(directory, IDENTITY_FILE);
    let result = (|| {
        let mut file = private_options().create_new(true).open(&temporary)?;
        file.write_all(contents)?;
        file.sync_all()?;
        drop(file);
        match std::fs::hard_link(&temporary, path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => Ok(()),
            // A file system without hard links (FAT on a stick, some network shares):
            // fall back on a rename, which loses only the race above.
            Err(_) => std::fs::rename(&temporary, path),
        }
    })();
    let _ = std::fs::remove_file(&temporary);
    result
}

pub(crate) fn temporary_path(directory: &Path, name: &str) -> PathBuf {
    directory.join(format!(".{name}.{}.tmp", std::process::id()))
}

/// Options for a file only this user may read.
///
/// On Windows nothing is set here: the file takes the ACL of the configuration
/// directory, which under `%APPDATA%` is the user's own (plus SYSTEM and
/// Administrators). Writing an explicit user-only DACL would be stricter and is not
/// done yet.
pub(crate) fn private_options() -> std::fs::OpenOptions {
    let mut options = std::fs::OpenOptions::new();
    options.write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_identity_is_made_once_and_kept() {
        let dir = tempfile::tempdir().unwrap();
        let first = Identity::load_or_create(dir.path()).unwrap();
        let again = Identity::load_or_create(dir.path()).unwrap();
        assert_eq!(first.fingerprint(), again.fingerprint());
        assert_eq!(first.fingerprint().len(), 64);
        assert!(first
            .fingerprint()
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)));
        assert_eq!(first.fingerprint(), fingerprint(first.certificate()));

        let other = tempfile::tempdir().unwrap();
        let stranger = Identity::load_or_create(other.path()).unwrap();
        assert_ne!(first.fingerprint(), stranger.fingerprint());

        // Nothing left behind but the identity itself.
        let names: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        assert_eq!(names, [IDENTITY_FILE]);
    }

    #[cfg(unix)]
    #[test]
    fn only_this_user_can_read_the_identity() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        Identity::load_or_create(dir.path()).unwrap();
        let mode = std::fs::metadata(dir.path().join(IDENTITY_FILE))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
    }

    #[test]
    fn a_damaged_identity_is_reported_not_replaced() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(IDENTITY_FILE);
        std::fs::write(&path, "not a certificate").unwrap();
        let error = Identity::load_or_create(dir.path()).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "not a certificate");
    }
}
