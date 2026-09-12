//! Local IPC between the LittleBigMouse processes.
//!
//! [`framing`] is the transport-independent part: bounded, length-prefixed
//! UTF-8 frames over any async byte stream. [`protocol`] is what the two processes
//! say to each other — one vocabulary, read by both sides from the same types — and
//! [`endpoint`] says where the daemon listens. The server that carries them over a
//! current-session named pipe (Windows) or a 0600 Unix-domain socket (Linux)
//! stays in the hook crate, next to the daemon state it serves.

pub mod endpoint;
pub mod framing;
pub mod instance;
pub mod protocol;
