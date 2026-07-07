//! Inter-process communication with the C# UI.
//!
//! Port of the C++ `Remote/` layer: a loopback TCP server (port 25196) speaking
//! `\n`-delimited XML. `protocol` handles message (de)serialization, `server`
//! owns the listener and the client registry, and `client` runs one reader per
//! connection.

pub mod client;
pub mod protocol;
pub mod server;
