//! End-to-end local IPC framing and daemon-state contract.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use littlebigmouse_hook::ipc::{framing, server};
use littlebigmouse_hook::shared::Shared;
use tokio::io::{AsyncRead, AsyncWrite};

static NEXT_ENDPOINT: AtomicU64 = AtomicU64::new(1);

trait TestStream: AsyncRead + AsyncWrite + Unpin {}
impl<T: AsyncRead + AsyncWrite + Unpin> TestStream for T {}

fn endpoint() -> String {
    let id = NEXT_ENDPOINT.fetch_add(1, Ordering::Relaxed);
    #[cfg(windows)]
    {
        format!(r"\\.\pipe\LittleBigMouse-test-{}-{id}", std::process::id())
    }
    #[cfg(target_os = "linux")]
    {
        std::env::temp_dir()
            .join(format!(
                "littlebigmouse-test-{}-{id}.sock",
                std::process::id()
            ))
            .to_string_lossy()
            .into_owned()
    }
}

#[test]
fn server_can_start_from_synchronous_main_without_an_existing_tokio_reactor() {
    let shared: &'static Shared = Box::leak(Box::new(Shared::new()));
    let endpoint = endpoint();

    let result = std::panic::catch_unwind(|| server::start_with_endpoint(shared, endpoint));

    assert!(result.is_ok(), "local IPC construction must not panic");
    assert!(result.unwrap().is_ok(), "local IPC endpoint must bind");
}

#[cfg(windows)]
async fn connect(endpoint: &str) -> Box<dyn TestStream> {
    use tokio::net::windows::named_pipe::ClientOptions;

    loop {
        match ClientOptions::new().open(endpoint) {
            Ok(stream) => return Box::new(stream),
            Err(_) => tokio::time::sleep(Duration::from_millis(10)).await,
        }
    }
}

#[cfg(target_os = "linux")]
async fn connect(endpoint: &str) -> Box<dyn TestStream> {
    loop {
        match tokio::net::UnixStream::connect(endpoint).await {
            Ok(stream) => return Box::new(stream),
            Err(_) => tokio::time::sleep(Duration::from_millis(10)).await,
        }
    }
}

async fn send_and_read(xml: &str) -> String {
    let shared: &'static Shared = Box::leak(Box::new(Shared::new()));
    let endpoint = endpoint();
    let (_server, _) = server::start_with_endpoint(shared, endpoint.clone()).unwrap();
    let mut stream = tokio::time::timeout(Duration::from_secs(2), connect(&endpoint))
        .await
        .expect("connect timeout");
    framing::write_frame(&mut stream, xml).await.unwrap();
    tokio::time::timeout(Duration::from_secs(2), framing::read_frame(&mut stream))
        .await
        .expect("reply timeout")
        .unwrap()
}

#[tokio::test]
async fn listen_replies_with_current_state() {
    let line = send_and_read(r#"<CommandMessage Command="Listen" Payload=""/>"#).await;
    assert!(line.contains("Stopped"), "got {line:?}");
}

#[tokio::test]
async fn state_query_replies_stopped() {
    let line = send_and_read(r#"<CommandMessage Command="State" Payload=""/>"#).await;
    assert!(line.contains("Stopped"), "got {line:?}");
}

/// The historical pipe transport died on exactly this: events pushed to a
/// listener while another client sends commands (full duplex). Interleaved to
/// stay under the bounded per-client queue, as the real hook thread would.
#[tokio::test]
async fn events_stream_to_listener_while_commands_flow() {
    use littlebigmouse_hook::ipc::protocol;

    let shared: &'static Shared = Box::leak(Box::new(Shared::new()));
    let endpoint = endpoint();
    let (server, _) = server::start_with_endpoint(shared, endpoint.clone()).unwrap();

    let mut listener = tokio::time::timeout(Duration::from_secs(2), connect(&endpoint))
        .await
        .expect("listener connect timeout");
    framing::write_frame(
        &mut listener,
        r#"<CommandMessage Command="Listen" Payload=""/>"#,
    )
    .await
    .unwrap();
    let ack = framing::read_frame(&mut listener).await.unwrap();
    assert!(ack.contains("Stopped"), "got {ack:?}");

    let mut commander = tokio::time::timeout(Duration::from_secs(2), connect(&endpoint))
        .await
        .expect("commander connect timeout");

    for _ in 0..50 {
        server.broadcast(protocol::DISPLAY_CHANGED);
        framing::write_frame(
            &mut commander,
            r#"<CommandMessage Command="State" Payload=""/>"#,
        )
        .await
        .unwrap();
        let reply =
            tokio::time::timeout(Duration::from_secs(2), framing::read_frame(&mut commander))
                .await
                .expect("command reply timeout")
                .unwrap();
        assert!(reply.contains("Stopped"), "got {reply:?}");
        let event =
            tokio::time::timeout(Duration::from_secs(2), framing::read_frame(&mut listener))
                .await
                .expect("event timeout")
                .unwrap();
        assert!(event.contains("DisplayChanged"), "got {event:?}");
    }
}

/// A dropped listener must not poison the server: a new connection gets a
/// fresh Listen with working event delivery.
#[tokio::test]
async fn listener_can_reconnect_after_disconnect() {
    use littlebigmouse_hook::ipc::protocol;

    let shared: &'static Shared = Box::leak(Box::new(Shared::new()));
    let endpoint = endpoint();
    let (server, _) = server::start_with_endpoint(shared, endpoint.clone()).unwrap();

    for _ in 0..3 {
        let mut listener = tokio::time::timeout(Duration::from_secs(2), connect(&endpoint))
            .await
            .expect("connect timeout");
        framing::write_frame(
            &mut listener,
            r#"<CommandMessage Command="Listen" Payload=""/>"#,
        )
        .await
        .unwrap();
        let ack = framing::read_frame(&mut listener).await.unwrap();
        assert!(ack.contains("Stopped"), "got {ack:?}");

        server.broadcast(protocol::DISPLAY_CHANGED);
        let event =
            tokio::time::timeout(Duration::from_secs(2), framing::read_frame(&mut listener))
                .await
                .expect("event timeout")
                .unwrap();
        assert!(event.contains("DisplayChanged"), "got {event:?}");
        drop(listener);
    }
}

/// A Load's outcome must reach the listening client: Loaded with a summary on
/// success (the virtual-layout "simulate" flow has no Running event to wait
/// for), LoadFailed when the payload cannot be parsed.
#[tokio::test]
async fn load_outcome_is_broadcast_to_listener() {
    let shared: &'static Shared = Box::leak(Box::new(Shared::new()));
    let endpoint = endpoint();
    let (_server, _) = server::start_with_endpoint(shared, endpoint.clone()).unwrap();

    let mut listener = tokio::time::timeout(Duration::from_secs(2), connect(&endpoint))
        .await
        .expect("listener connect timeout");
    framing::write_frame(
        &mut listener,
        r#"<CommandMessage Command="Listen" Payload=""/>"#,
    )
    .await
    .unwrap();
    let ack = framing::read_frame(&mut listener).await.unwrap();
    assert!(ack.contains("Stopped"), "got {ack:?}");

    let mut commander = tokio::time::timeout(Duration::from_secs(2), connect(&endpoint))
        .await
        .expect("commander connect timeout");

    let load = concat!(
        r#"<CommandMessage Command="Load"><Payload>"#,
        r#"<ZonesLayout Algorithm="Strait" MaxTravelDistance="200" Virtual="True"><MainZones>"#,
        r#"<Zone Id="0" Name="A"><PixelsBounds><Rect Left="0" Top="0" Width="1920" Height="1080"></Rect></PixelsBounds><PhysicalBounds><Rect Left="0" Top="0" Width="500" Height="280"></Rect></PhysicalBounds></Zone>"#,
        r#"</MainZones></ZonesLayout></Payload></CommandMessage>"#,
    );
    framing::write_frame(&mut commander, load).await.unwrap();
    let event = tokio::time::timeout(Duration::from_secs(2), framing::read_frame(&mut listener))
        .await
        .expect("Loaded event timeout")
        .unwrap();
    assert!(event.contains("Loaded"), "got {event:?}");
    assert!(event.contains("1 zones (1 main), virtual"), "got {event:?}");

    // A virtual load is auto-probed: the edge report follows immediately.
    let event = tokio::time::timeout(Duration::from_secs(2), framing::read_frame(&mut listener))
        .await
        .expect("Probed event timeout")
        .unwrap();
    assert!(event.contains("Probed"), "got {event:?}");
    assert!(event.contains("ProbeReport"), "got {event:?}");

    // An empty/unparsable payload reports failure the same way.
    framing::write_frame(&mut commander, r#"<CommandMessage Command="Load"/>"#)
        .await
        .unwrap();
    let event = tokio::time::timeout(Duration::from_secs(2), framing::read_frame(&mut listener))
        .await
        .expect("LoadFailed event timeout")
        .unwrap();
    assert!(event.contains("LoadFailed"), "got {event:?}");
}

#[tokio::test]
async fn malformed_frame_is_ignored_without_crashing_server() {
    let shared: &'static Shared = Box::leak(Box::new(Shared::new()));
    let endpoint = endpoint();
    let (_server, _) = server::start_with_endpoint(shared, endpoint.clone()).unwrap();
    let mut stream = tokio::time::timeout(Duration::from_secs(2), connect(&endpoint))
        .await
        .expect("connect timeout");

    framing::write_frame(&mut stream, "not xml <<<")
        .await
        .unwrap();
    framing::write_frame(
        &mut stream,
        r#"<CommandMessage Command="State" Payload=""/>"#,
    )
    .await
    .unwrap();
    let reply = tokio::time::timeout(Duration::from_secs(2), framing::read_frame(&mut stream))
        .await
        .expect("reply timeout")
        .unwrap();
    assert!(reply.contains("Stopped"), "got {reply:?}");
}
