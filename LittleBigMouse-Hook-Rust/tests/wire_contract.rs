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
