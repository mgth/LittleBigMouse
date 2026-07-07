//! Pins the pump-independent parts of the socket/XML wire contract.
//!
//! `Listen`/`State`/empty-line all reply with the current daemon state without
//! needing the Win32 message pump, so they run as plain integration tests here.
//! The `Run`/`Stop` state transitions now follow the *actual* hook install on
//! the pump thread (Phase 1), so they can't be exercised in-process — they are
//! verified by driving the real `lbm-hook.exe` end-to-end instead.

use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;

use littlebigmouse_hook::ipc;
use littlebigmouse_hook::shared::Shared;

/// Connect, send one `\r\n`-terminated line (as .NET `WriteLine` emits), and
/// read one reply line.
fn send_and_read(port: u16, xml: &str) -> String {
    let conn = TcpStream::connect(("127.0.0.1", port)).expect("connect");
    {
        let mut w = conn.try_clone().unwrap();
        w.write_all(xml.as_bytes()).unwrap();
        w.write_all(b"\r\n").unwrap();
    }
    let mut reader = BufReader::new(conn);
    let mut line = String::new();
    reader.read_line(&mut line).expect("read reply");
    line
}

#[test]
fn listen_replies_with_current_state() {
    // A fresh, leaked Shared keeps tests independent of the process-global static.
    let shared: &'static Shared = Box::leak(Box::new(Shared::new()));
    let (_server, port) = ipc::server::start(shared, 0);

    let line = send_and_read(port, r#"<CommandMessage Command="Listen" Payload=""/>"#);
    assert!(
        line.contains("Stopped"),
        "initial state should be Stopped, got {line:?}"
    );
}

#[test]
fn state_query_replies_stopped() {
    let shared: &'static Shared = Box::leak(Box::new(Shared::new()));
    let (_server, port) = ipc::server::start(shared, 0);

    let line = send_and_read(port, r#"<CommandMessage Command="State" Payload=""/>"#);
    assert!(line.contains("Stopped"), "got {line:?}");
}

#[test]
fn empty_line_replies_state() {
    let shared: &'static Shared = Box::leak(Box::new(Shared::new()));
    let (_server, port) = ipc::server::start(shared, 0);

    // C++ ReceiveClientMessage: an empty message just re-reports state.
    let line = send_and_read(port, "");
    assert!(line.contains("Stopped"), "got {line:?}");
}
