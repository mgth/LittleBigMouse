//! Inter-process communication with the C# UI.
//!
//! Windows uses a current-session named pipe with a user/SYSTEM DACL; Linux uses
//! a 0600 Unix-domain socket. Both carry bounded length-prefixed UTF-8 XML.

// Framing and protocol live in the lbm-ipc crate; the server stays here, next to
// the daemon state it serves.
pub use lbm_ipc::{framing, protocol};
pub mod server;
