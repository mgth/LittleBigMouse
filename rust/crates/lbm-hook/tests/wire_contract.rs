//! End-to-end local IPC framing and daemon-state contract.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use littlebigmouse_hook::ipc::{framing, protocol, server};
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
    let line = send_and_read(&protocol::frame(&[protocol::Command::Listen])).await;
    assert!(line.contains("Stopped"), "got {line:?}");
}

#[tokio::test]
async fn state_query_replies_stopped() {
    let line = send_and_read(&protocol::frame(&[protocol::Command::State])).await;
    assert!(line.contains("Stopped"), "got {line:?}");
}

/// The one connection carries both directions, for as long as it lives: events are
/// pushed onto it while commands keep being answered on it.
///
/// This is the regression test for the bug the one-client rewrite fixed. The server
/// used to treat `Listen` as a change of role — after it, the reader stopped reading
/// commands, swallowed the next frame and closed the connection. Every test used a
/// second connection to command, so nothing saw it; against a real hook the v6 agent
/// subscribes first, and so could drive nothing at all.
///
/// Interleaved to stay under the bounded outbound queue, as the real hook thread would.
#[tokio::test]
async fn the_connection_answers_commands_while_events_stream_on_it() {
    use littlebigmouse_hook::ipc::protocol;

    let shared: &'static Shared = Box::leak(Box::new(Shared::new()));
    let endpoint = endpoint();
    let (server, _) = server::start_with_endpoint(shared, endpoint.clone()).unwrap();

    let mut agent = tokio::time::timeout(Duration::from_secs(2), connect(&endpoint))
        .await
        .expect("connect timeout");
    framing::write_frame(&mut agent, &protocol::frame(&[protocol::Command::Listen]))
        .await
        .unwrap();
    let ack = framing::read_frame(&mut agent).await.unwrap();
    assert!(ack.contains("Stopped"), "got {ack:?}");

    for _ in 0..50 {
        // Queued before the command is even read, so the two arrive in this order.
        server.broadcast(&protocol::Event::DisplayChanged);
        framing::write_frame(&mut agent, &protocol::frame(&[protocol::Command::State]))
            .await
            .unwrap();

        let event = tokio::time::timeout(Duration::from_secs(2), framing::read_frame(&mut agent))
            .await
            .expect("event timeout")
            .unwrap();
        assert!(event.contains("DisplayChanged"), "got {event:?}");
        let reply = tokio::time::timeout(Duration::from_secs(2), framing::read_frame(&mut agent))
            .await
            .expect("command reply timeout")
            .unwrap();
        assert!(reply.contains("Stopped"), "got {reply:?}");
    }
}

/// One client at a time, and the newest wins.
///
/// An agent that was restarted — or one that took over from a crashed predecessor —
/// must be able to drive the hook straight away, without waiting for the operating
/// system to notice that the connection it replaces is dead. The hook outlives its
/// agent (D5), so this is the ordinary case, not the exception.
#[tokio::test]
async fn a_new_client_takes_the_connection_from_the_old_one() {
    use littlebigmouse_hook::ipc::protocol;

    let shared: &'static Shared = Box::leak(Box::new(Shared::new()));
    let endpoint = endpoint();
    let (server, _) = server::start_with_endpoint(shared, endpoint.clone()).unwrap();

    let mut old = tokio::time::timeout(Duration::from_secs(2), connect(&endpoint))
        .await
        .expect("connect timeout");
    framing::write_frame(&mut old, &protocol::frame(&[protocol::Command::Listen]))
        .await
        .unwrap();
    let ack = framing::read_frame(&mut old).await.unwrap();
    assert!(ack.contains("Stopped"), "got {ack:?}");

    let mut new = tokio::time::timeout(Duration::from_secs(2), connect(&endpoint))
        .await
        .expect("connect timeout");
    framing::write_frame(&mut new, &protocol::frame(&[protocol::Command::Listen]))
        .await
        .unwrap();
    let ack = tokio::time::timeout(Duration::from_secs(2), framing::read_frame(&mut new))
        .await
        .expect("the newcomer is served")
        .unwrap();
    assert!(ack.contains("Stopped"), "got {ack:?}");

    // The evicted connection is closed, not merely ignored: an agent holding a
    // connection nobody serves would wait for events that never come, and would
    // never know to reconnect.
    let end = tokio::time::timeout(Duration::from_secs(2), framing::read_frame(&mut old))
        .await
        .expect("the evicted connection ends");
    assert!(end.is_err(), "got {end:?}");

    // And the survivor is the one the events go to.
    server.broadcast(&protocol::Event::DisplayChanged);
    let event = tokio::time::timeout(Duration::from_secs(2), framing::read_frame(&mut new))
        .await
        .expect("event timeout")
        .unwrap();
    assert!(event.contains("DisplayChanged"), "got {event:?}");
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
            &protocol::frame(&[protocol::Command::Listen]),
        )
        .await
        .unwrap();
        let ack = framing::read_frame(&mut listener).await.unwrap();
        assert!(ack.contains("Stopped"), "got {ack:?}");

        server.broadcast(&protocol::Event::DisplayChanged);
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

    let mut agent = tokio::time::timeout(Duration::from_secs(2), connect(&endpoint))
        .await
        .expect("connect timeout");
    framing::write_frame(&mut agent, &protocol::frame(&[protocol::Command::Listen]))
        .await
        .unwrap();
    let ack = framing::read_frame(&mut agent).await.unwrap();
    assert!(ack.contains("Stopped"), "got {ack:?}");

    let zones = concat!(
        r#"<ZonesLayout Algorithm="Strait" MaxTravelDistance="200" Virtual="True"><MainZones>"#,
        r#"<Zone Id="0" Name="A"><PixelsBounds><Rect Left="0" Top="0" Width="1920" Height="1080"></Rect></PixelsBounds><PhysicalBounds><Rect Left="0" Top="0" Width="500" Height="280"></Rect></PhysicalBounds></Zone>"#,
        r#"</MainZones></ZonesLayout>"#,
    );
    let load = protocol::frame(&[protocol::Command::Load {
        zones: zones.to_owned(),
    }]);
    framing::write_frame(&mut agent, &load).await.unwrap();
    let event = tokio::time::timeout(Duration::from_secs(2), framing::read_frame(&mut agent))
        .await
        .expect("Loaded event timeout")
        .unwrap();
    // What it accepted travels as numbers now, not as a sentence about them.
    assert_eq!(
        protocol::parse_event(&event),
        Some(protocol::Event::Loaded {
            zones: 1,
            main: 1,
            virtual_layout: true
        }),
        "got {event:?}"
    );

    // An empty/unparsable payload reports failure the same way.
    framing::write_frame(
        &mut agent,
        &protocol::frame(&[protocol::Command::Load {
            zones: String::new(),
        }]),
    )
    .await
    .unwrap();
    let event = tokio::time::timeout(Duration::from_secs(2), framing::read_frame(&mut agent))
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
    framing::write_frame(&mut stream, &protocol::frame(&[protocol::Command::State]))
        .await
        .unwrap();
    let reply = tokio::time::timeout(Duration::from_secs(2), framing::read_frame(&mut stream))
        .await
        .expect("reply timeout")
        .unwrap();
    assert!(reply.contains("Stopped"), "got {reply:?}");
}

/// The greeting names the layout the hook holds, so an agent that finds it already
/// running (D5) can tell whether it is the one it wants — and only recapture the mice
/// when it is not.
#[tokio::test]
async fn the_greeting_names_the_layout_the_hook_holds() {
    let shared: &'static Shared = Box::leak(Box::new(Shared::new()));
    let endpoint = endpoint();
    let (_server, _) = server::start_with_endpoint(shared, endpoint.clone()).unwrap();

    let mut agent = tokio::time::timeout(Duration::from_secs(2), connect(&endpoint))
        .await
        .expect("connect timeout");

    // Nothing loaded yet: it holds no layout, and says so.
    framing::write_frame(
        &mut agent,
        &protocol::frame(&[protocol::Command::Hello {
            protocol: lbm_ipc::protocol::PROTOCOL,
        }]),
    )
    .await
    .unwrap();
    let greeting = framing::read_frame(&mut agent).await.unwrap();
    let Some(protocol::Event::Hello { layout, .. }) = protocol::parse_event(&greeting) else {
        panic!("a greeting came back, got {greeting:?}");
    };
    assert_eq!(layout, "");

    let zones = concat!(
        r#"<ZonesLayout Algorithm="Strait" MaxTravelDistance="200"><MainZones>"#,
        r#"<Zone Id="0" Name="A"><PixelsBounds><Rect Left="0" Top="0" Width="1920" Height="1080"></Rect></PixelsBounds><PhysicalBounds><Rect Left="0" Top="0" Width="500" Height="280"></Rect></PhysicalBounds></Zone>"#,
        r#"</MainZones></ZonesLayout>"#,
    );
    framing::write_frame(
        &mut agent,
        &protocol::frame(&[
            protocol::Command::Load {
                zones: zones.to_owned(),
            },
            protocol::Command::Hello {
                protocol: lbm_ipc::protocol::PROTOCOL,
            },
        ]),
    )
    .await
    .unwrap();
    let greeting = tokio::time::timeout(Duration::from_secs(2), framing::read_frame(&mut agent))
        .await
        .expect("greeting timeout")
        .unwrap();
    let Some(protocol::Event::Hello { layout, .. }) = protocol::parse_event(&greeting) else {
        panic!("a greeting came back, got {greeting:?}");
    };
    // Exactly what the agent computes over the document it sent: that is the whole
    // contract — two processes naming the same layout the same way.
    assert_eq!(layout, lbm_ipc::protocol::fingerprint(zones));
}

/// A Run the daemon will not honour still answers. Silence would leave the agent
/// waiting for a `Running` that never comes — nothing retries a Start — and the user
/// who pressed it looking at "stopped" with no reason given.
#[tokio::test]
async fn a_refused_run_says_what_the_engine_is_doing() {
    let shared: &'static Shared = Box::leak(Box::new(Shared::new()));
    let endpoint = endpoint();
    let (_server, _) = server::start_with_endpoint(shared, endpoint.clone()).unwrap();

    let mut agent = tokio::time::timeout(Duration::from_secs(2), connect(&endpoint))
        .await
        .expect("connect timeout");
    framing::write_frame(&mut agent, &protocol::frame(&[protocol::Command::Listen]))
        .await
        .unwrap();
    let ack = framing::read_frame(&mut agent).await.unwrap();
    assert!(ack.contains("Stopped"), "got {ack:?}");

    // Nothing was ever loaded: the engine has no zones to route between, so the Run
    // is refused — and the refusal is reported, not swallowed.
    framing::write_frame(&mut agent, &protocol::frame(&[protocol::Command::Run]))
        .await
        .unwrap();
    let answer = tokio::time::timeout(Duration::from_secs(2), framing::read_frame(&mut agent))
        .await
        .expect("a refused Run answers")
        .unwrap();
    assert_eq!(
        protocol::parse_event(&answer),
        Some(protocol::Event::Stopped),
        "got {answer:?}"
    );
}
