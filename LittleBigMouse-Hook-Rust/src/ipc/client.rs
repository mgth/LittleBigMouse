//! Per-connection reader loop.
//!
//! Port of `RemoteClient::RunThread`: read bytes, frame on `\n`, and dispatch
//! each complete line. Mirrors the C++ quirk that once a client sends `Listen`,
//! the reader returns (stops reading) while the socket stays open in the registry
//! for the server to push events to.

use std::io::Read;
use std::net::TcpStream;

use crate::daemon;
use crate::ipc::server::{ClientId, ServerHandle};
use crate::shared::Shared;

pub fn run(id: ClientId, mut stream: TcpStream, server: ServerHandle, shared: &'static Shared) {
    let mut buf = [0u8; 4096];
    let mut message = String::new();

    loop {
        let n = match stream.read(&mut buf) {
            Ok(0) => break, // peer closed the connection
            Ok(n) => n,
            Err(_) => break,
        };

        // Bytes on the wire are UTF-8 XML; lossy decoding keeps us robust to any
        // stray byte without panicking on the hot path.
        let chunk = String::from_utf8_lossy(&buf[..n]);
        let mut rest: &str = &chunk;

        while let Some(pos) = rest.find('\n') {
            message.push_str(&rest[..pos]);
            rest = &rest[pos + 1..];

            let became_listening = daemon::receive_message(&message, id, &server, shared);
            message.clear();

            if became_listening {
                // Keep the socket open (held by the registry) for event pushes,
                // but stop reading — exactly as the C++ client does.
                return;
            }
        }

        message.push_str(rest);
    }

    server.remove(id);
}
