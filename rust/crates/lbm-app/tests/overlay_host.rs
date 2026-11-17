//! Exercise the real private entry point against an isolated, deliberately silent display.
//! No connection to the user's compositor, X server or agent is made.
#![cfg(target_os = "linux")]
use std::{
    io::{Read, Write},
    os::unix::net::{UnixListener, UnixStream},
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

struct Host(Child);
impl Drop for Host {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
fn start(path: &std::path::Path) -> Host {
    Host(
        Command::new(env!("CARGO_BIN_EXE_lbm-app"))
            .arg("--screen-overlay-host")
            .env("XDG_SESSION_TYPE", "wayland")
            .env("WAYLAND_DISPLAY", path)
            .env_remove("WAYLAND_SOCKET")
            .env_remove("DISPLAY")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap(),
    )
}
fn silent() -> (tempfile::TempDir, Host, UnixStream) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("wayland-private");
    let listener = UnixListener::bind(&path).unwrap();
    listener.set_nonblocking(true).unwrap();
    let mut host = start(&path);
    let until = Instant::now() + Duration::from_secs(5);
    let peer = loop {
        match listener.accept() {
            Ok((peer, _)) => break peer,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(e) => panic!("{e}"),
        }
        assert!(
            host.0.try_wait().unwrap().is_none(),
            "host exited before connecting"
        );
        assert!(Instant::now() < until, "host never connected");
        std::thread::sleep(Duration::from_millis(10));
    };
    (directory, host, peer)
}
fn finish(host: &mut Host, pulses: bool) -> String {
    let until = Instant::now() + Duration::from_secs(5);
    loop {
        if host.0.try_wait().unwrap().is_some() {
            break;
        }
        assert!(
            Instant::now() < until,
            "host did not close its display connection"
        );
        if pulses {
            let _ = host.0.stdin.as_mut().unwrap().write_all(b"\"Pulse\"\n");
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    let mut output = String::new();
    host.0
        .stdout
        .take()
        .unwrap()
        .read_to_string(&mut output)
        .unwrap();
    output
}
fn assert_closed(mut peer: UnixStream) {
    peer.set_read_timeout(Some(Duration::from_secs(1))).unwrap();
    let mut bytes = Vec::new();
    peer.read_to_end(&mut bytes)
        .expect("host kept its compositor connection alive");
}
#[test]
fn eof_closes_the_display_even_during_a_blocked_registry_roundtrip() {
    let (_dir, mut host, peer) = silent();
    host.0.stdin.take();
    let start = Instant::now();
    assert!(finish(&mut host, false).is_empty());
    assert!(start.elapsed() < Duration::from_secs(1));
    assert!(host.0.wait().unwrap().success());
    assert_closed(peer);
}
#[test]
fn watchdog_closes_a_blocked_compositor_even_while_the_editor_keeps_refreshing() {
    let (_dir, mut host, peer) = silent();
    let output = finish(&mut host, true);
    assert!(
        output.contains("display server stopped answering"),
        "{output}"
    );
    assert_closed(peer);
}
#[test]
fn watchdog_expires_the_editors_lease_when_its_event_loop_stops() {
    let (_dir, mut host, peer) = silent();
    let output = finish(&mut host, false);
    assert!(output.contains("editor stopped refreshing"), "{output}");
    assert_closed(peer);
}
#[test]
fn malformed_and_oversized_editor_messages_close_the_display() {
    for bytes in [b"not-json\n".to_vec(), vec![b'x'; 262_145]] {
        let (_dir, mut host, peer) = silent();
        host.0.stdin.as_mut().unwrap().write_all(&bytes).unwrap();
        let output = finish(&mut host, false);
        assert!(
            output.contains("Invalid overlay message") || output.contains("size limit"),
            "{output}"
        );
        assert_closed(peer);
    }
}
#[test]
fn missing_wayland_server_returns_a_structured_error_without_starting_egui() {
    let dir = tempfile::tempdir().unwrap();
    let mut host = start(&dir.path().join("absent"));
    let output = finish(&mut host, false);
    let reply: serde_json::Value = serde_json::from_str(output.trim()).unwrap();
    assert!(reply["Error"]
        .as_str()
        .unwrap()
        .starts_with("Wayland overlay:"));
    assert!(!host.0.wait().unwrap().success());
}
