//! Pins the socket/XML wire contract the C# UI relies on.
//!
//! Reproduces the UI's sequence: a persistent "Listen" connection that receives
//! state broadcasts, plus short-lived per-command connections (`Run`, `Stop`) —
//! exactly what `LittleBigMouseClientService` / `RemoteClientSocket` do. Lines
//! are `\r\n`-terminated, as .NET `StreamWriter.WriteLine` emits.

use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;

use littlebigmouse_hook::ipc;
use littlebigmouse_hook::shared::Shared;

/// Send one command on a short-lived connection and close it (the UI's
/// `TrySendMessageAsync` pattern).
fn send_command(port: u16, xml: &str) {
    let mut conn = TcpStream::connect(("127.0.0.1", port)).expect("connect");
    conn.write_all(xml.as_bytes()).expect("write");
    conn.write_all(b"\r\n").expect("write newline");
    // Drop closes the connection.
}

#[test]
fn listen_run_stop_roundtrip() {
    // A fresh, leaked Shared keeps tests independent of the process-global static.
    let shared: &'static Shared = Box::leak(Box::new(Shared::new()));
    let (_server, port) = ipc::server::start(shared, 0);

    // Persistent "Listen" connection.
    let listen = TcpStream::connect(("127.0.0.1", port)).expect("connect listen");
    {
        let mut w = listen.try_clone().unwrap();
        w.write_all(br#"<CommandMessage Command="Listen" Payload=""/>"#)
            .unwrap();
        w.write_all(b"\r\n").unwrap();
    }
    let mut reader = BufReader::new(listen.try_clone().unwrap());

    // On Listen the daemon replies with current state to this client: Stopped.
    let mut line = String::new();
    reader.read_line(&mut line).unwrap();
    assert!(
        line.contains("Stopped"),
        "initial state should be Stopped, got {line:?}"
    );

    // Run on a separate connection -> Running broadcast on the listen socket.
    send_command(port, r#"<CommandMessage Command="Run" Payload=""/>"#);
    line.clear();
    reader.read_line(&mut line).unwrap();
    assert!(
        line.contains("Running"),
        "expected Running broadcast, got {line:?}"
    );

    // Stop -> Stopped broadcast.
    send_command(port, r#"<CommandMessage Command="Stop" Payload=""/>"#);
    line.clear();
    reader.read_line(&mut line).unwrap();
    assert!(
        line.contains("Stopped"),
        "expected Stopped broadcast, got {line:?}"
    );
}

#[test]
fn load_then_run_batched_on_one_connection() {
    // The UI's StartAsync sends `Load\nRun\n` on a single connection.
    let shared: &'static Shared = Box::leak(Box::new(Shared::new()));
    let (_server, port) = ipc::server::start(shared, 0);

    let listen = TcpStream::connect(("127.0.0.1", port)).unwrap();
    {
        let mut w = listen.try_clone().unwrap();
        w.write_all(br#"<CommandMessage Command="Listen" Payload=""/>"#)
            .unwrap();
        w.write_all(b"\r\n").unwrap();
    }
    let mut reader = BufReader::new(listen.try_clone().unwrap());
    let mut line = String::new();
    reader.read_line(&mut line).unwrap(); // Stopped

    // Batched Load + Run on one short-lived connection.
    {
        let mut conn = TcpStream::connect(("127.0.0.1", port)).unwrap();
        let batch = concat!(
            "<CommandMessage Command=\"Load\"><Payload><ZonesLayout MaxTravelDistance=\"200\"/></Payload></CommandMessage>\r\n",
            "<CommandMessage Command=\"Run\" Payload=\"\"/>\r\n"
        );
        conn.write_all(batch.as_bytes()).unwrap();
    }

    line.clear();
    reader.read_line(&mut line).unwrap();
    assert!(
        line.contains("Running"),
        "batched Load+Run should end Running, got {line:?}"
    );
}
