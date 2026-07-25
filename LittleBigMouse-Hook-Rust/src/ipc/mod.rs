//! Inter-process communication with the C# UI.
//!
//! Windows uses a current-session named pipe with a user/SYSTEM DACL; Linux uses
//! a 0600 Unix-domain socket. Both carry bounded length-prefixed UTF-8 XML.

pub mod framing;
pub mod protocol;
pub mod server;
