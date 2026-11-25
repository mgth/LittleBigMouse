//! The network of the multi-PC mode (docs/multi-pc.md): how two LittleBigMouse agents
//! find out who they are talking to and carry control messages and the input stream
//! between them.
//!
//! - [`identity`]: this machine's certificate, made once; its fingerprint is the
//!   machine id.
//! - [`tls`]: mutual TLS 1.3 where the handshake proves possession of a key and the
//!   caller, not a CA, decides whom to trust.
//! - [`wire`]: the frames and the control vocabulary.
//! - [`peers`]: the machines the user paired, on disk.
//! - [`link`]: a connection, from the handshake to the keepalive.
//! - [`discovery`]: the datagrams by which the machines of a local network propose
//!   themselves, before any pairing.
//!
//! The crate decides nothing about pairing itself: it shows the caller a fingerprint
//! and waits to be told. That decision belongs to the agent, which has the store and
//! the user.

pub mod discovery;
pub mod identity;
pub mod link;
pub mod peers;
pub mod tls;
pub mod wire;

/// The TCP port every node listens on unless told otherwise.
pub const DEFAULT_PORT: u16 = 24817;
