//! A listening client, for tests about what the daemon says when nobody asked.
//!
//! Most of what this hook says is unprompted: a display changed, the screen went off,
//! the panic shortcut fired. All of it goes out through [`Shared::broadcast`], which
//! reaches the client through the server handle published in `Shared` — so until a
//! test could get a server in there, none of it could be observed, and none of it was.
//! That includes the rescue: the one path a user reaches for when the cursor is
//! trapped and nothing can be clicked.
//!
//! This starts a server on a private endpoint, subscribes to it, and hands back
//! something that can be asked what arrived — or asked to confirm that nothing did,
//! which is the whole content of a deduplication.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use lbm_ipc::framing::{read_frame, write_frame};
use lbm_ipc::protocol::{self, Command, Event};
use tokio::io::{AsyncRead, AsyncWrite};

use crate::ipc::server;
use crate::shared::Shared;

/// Long enough that a broadcast in flight has landed, short enough to keep a suite
/// quick. Only ever waited out in full when the answer is "nothing came", which is
/// what a test of a deduplication asks.
const QUIET: Duration = Duration::from_millis(200);

static NEXT: AtomicU64 = AtomicU64::new(1);

fn endpoint() -> String {
    let id = NEXT.fetch_add(1, Ordering::Relaxed);
    #[cfg(windows)]
    {
        format!(r"\\.\pipe\LittleBigMouse-unit-{}-{id}", std::process::id())
    }
    #[cfg(target_os = "linux")]
    {
        std::env::temp_dir()
            .join(format!(
                "littlebigmouse-unit-{}-{id}.sock",
                std::process::id()
            ))
            .to_string_lossy()
            .into_owned()
    }
}

/// A subscribed client of a server serving `shared`.
pub(crate) struct Listening {
    stream: Box<dyn Stream>,
}

trait Stream: AsyncRead + AsyncWrite + Unpin + Send {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send> Stream for T {}

#[cfg(target_os = "linux")]
async fn connect(endpoint: &str) -> Box<dyn Stream> {
    loop {
        match tokio::net::UnixStream::connect(endpoint).await {
            Ok(stream) => return Box::new(stream),
            Err(_) => tokio::time::sleep(Duration::from_millis(5)).await,
        }
    }
}

#[cfg(windows)]
async fn connect(endpoint: &str) -> Box<dyn Stream> {
    use tokio::net::windows::named_pipe::ClientOptions;

    loop {
        match ClientOptions::new().open(endpoint) {
            Ok(stream) => return Box::new(stream),
            Err(_) => tokio::time::sleep(Duration::from_millis(5)).await,
        }
    }
}

impl Listening {
    /// Starts a server for `shared` and subscribes to it. The daemon's answer to the
    /// subscription (its current state) is read here, so what a test reads afterwards
    /// is only what the daemon chose to say.
    pub(crate) async fn to(shared: &'static Shared) -> Listening {
        let endpoint = endpoint();
        server::start_with_endpoint(shared, endpoint.clone()).expect("the endpoint binds");
        let mut stream = tokio::time::timeout(Duration::from_secs(5), connect(&endpoint))
            .await
            .expect("the server accepts");
        write_frame(&mut stream, &protocol::frame(&[Command::Listen]))
            .await
            .expect("Listen is sent");
        tokio::time::timeout(Duration::from_secs(5), read_frame(&mut stream))
            .await
            .expect("the subscription is answered")
            .expect("the subscription is answered");
        Listening { stream }
    }

    /// The next event, or a failure naming what was being waited for.
    pub(crate) async fn next(&mut self, what: &str) -> Event {
        let frame = tokio::time::timeout(Duration::from_secs(5), read_frame(&mut self.stream))
            .await
            .unwrap_or_else(|_| panic!("nothing was said, waiting for {what}"))
            .unwrap_or_else(|error| panic!("the connection ended waiting for {what}: {error}"));
        protocol::parse_event(&frame).unwrap_or_else(|| panic!("not an event: {frame:?}"))
    }

    /// Asserts nothing more is said. The only way to test that something was
    /// deliberately *not* repeated.
    pub(crate) async fn stays_quiet(&mut self) {
        if let Ok(Ok(frame)) = tokio::time::timeout(QUIET, read_frame(&mut self.stream)).await {
            panic!("expected silence, got {frame:?}");
        }
    }
}
