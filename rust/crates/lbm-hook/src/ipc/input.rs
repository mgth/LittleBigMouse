//! The input endpoint of the multi-PC mode (docs/multi-pc.md): the second endpoint
//! this hook serves, beside its command endpoint (`lbm_ipc::input::input_endpoint_beside`).
//!
//! One client, the agent, and the newest connection wins, as on the command endpoint:
//! an agent that restarted must not wait for the system to notice its predecessor's
//! connection is dead. What travels are `lbm_ipc::input` batches — four-byte
//! little-endian length, then whole events — and on a remote they go one way, agent
//! to hook to system, through a [`Session`] and the platform's [`Injector`].
//!
//! The connection is a full duplex stream on purpose: on a master, the hook will write
//! captured batches to the agent over the same one (lot 6). Nothing writes yet.
//!
//! Two threads, neither of them the routing thread. The endpoint's own, with a small
//! runtime, reads and decodes; the injector's, which owns the session, does the
//! system calls. A stalled injection holds back the reader — the stream is bounded —
//! and never the mouse this machine routes.

use std::io;
use std::sync::mpsc as std_mpsc;
use std::time::Duration;

use lbm_ipc::input::{decode, read_batch, InputEvent};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::{mpsc, watch};

use crate::ipc::transport;
use crate::remote::{Injector, Session};

/// Batches waiting for the injector. A device report is one batch, so this is a few
/// dozen milliseconds of a fast mouse — enough to ride out a slow system call, short
/// enough that a stalled injector pushes back on the agent instead of piling up.
const QUEUE: usize = 64;

/// How long `stop` waits for the injector to have released everything.
const STOP_TIMEOUT: Duration = Duration::from_secs(1);

type ClientId = u64;

enum Message {
    /// A new client took the endpoint: whatever the previous one left pressed is
    /// released, and only this one is listened to.
    Adopt(ClientId),
    Batch(ClientId, Vec<InputEvent>),
    /// A client's stream ended: gone, garbage, or replaced.
    End(ClientId),
    /// The hook is leaving.
    Stop(std_mpsc::SyncSender<()>),
}

/// The running endpoint.
pub struct InputServer {
    messages: mpsc::Sender<Message>,
    endpoint: String,
}

impl InputServer {
    /// The endpoint's path or pipe name, for diagnostics.
    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }

    /// Releases whatever the stream holds pressed and stops injecting, waiting a
    /// bounded time for it to be done. For the hook's way out: a key that
    /// `SendInput` pressed stays down after the process that pressed it is gone.
    /// Synchronous, for `main`: not to be called from inside an async runtime.
    pub fn stop(&self) {
        let (done, wait) = std_mpsc::sync_channel(1);
        if self.messages.blocking_send(Message::Stop(done)).is_ok() {
            let _ = wait.recv_timeout(STOP_TIMEOUT);
        }
    }
}

/// Serves the input endpoint at `endpoint`, injecting through `injector`. Binds before
/// returning, so a client may connect as soon as this has returned.
pub fn start(endpoint: String, injector: Box<dyn Injector>) -> io::Result<InputServer> {
    let (messages, inbox) = mpsc::channel(QUEUE);

    std::thread::Builder::new()
        .name("lbm-input-inject".to_string())
        .spawn(move || inject(inbox, injector))?;

    let (ready_tx, ready_rx) = std_mpsc::sync_channel(1);
    let accepted = messages.clone();
    let bound = endpoint.clone();
    std::thread::Builder::new()
        .name("lbm-input-ipc".to_string())
        .spawn(move || {
            let runtime = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(runtime) => runtime,
                Err(error) => {
                    let _ = ready_tx.send(Err(error));
                    return;
                }
            };
            runtime.block_on(async move {
                // Inside the runtime: Tokio's named-pipe constructor needs its reactor.
                let mut acceptor = match transport::Acceptor::bind(&bound) {
                    Ok(acceptor) => acceptor,
                    Err(error) => {
                        let _ = ready_tx.send(Err(error));
                        return;
                    }
                };
                let _ = ready_tx.send(Ok(()));
                accept(&mut acceptor, accepted).await;
            });
        })?;

    ready_rx
        .recv()
        .map_err(|_| io::Error::other("input endpoint thread exited before binding"))??;

    Ok(InputServer { messages, endpoint })
}

/// Hands every connection the endpoint, newest first.
///
/// The adoption is announced to the injector here, in accepting order, before the
/// connection can send anything: the injector then cannot see a batch of the new
/// client ahead of the news that it is the one to listen to.
async fn accept(acceptor: &mut transport::Acceptor, messages: mpsc::Sender<Message>) {
    let (current, _) = watch::channel(0);
    let mut next: ClientId = 1;
    while let Some(stream) = acceptor.accept().await {
        let id = next;
        next += 1;
        if messages.send(Message::Adopt(id)).await.is_err() {
            return;
        }
        current.send_replace(id);
        tokio::spawn(connection(
            stream,
            id,
            current.subscribe(),
            messages.clone(),
        ));
    }
}

/// One client's connection, until it ends, sends what is not a batch, or a newer one
/// takes the endpoint.
async fn connection<S>(
    stream: S,
    id: ClientId,
    mut current: watch::Receiver<ClientId>,
    messages: mpsc::Sender<Message>,
) where
    S: AsyncRead + AsyncWrite + Unpin,
{
    // The write half is the master's direction (lot 6); held so the client sees one
    // connection that lives and dies as a whole.
    let (mut reader, _writer) = tokio::io::split(stream);
    loop {
        let events = tokio::select! {
            read = read_batch(&mut reader) => match read.and_then(|batch| decode(&batch)) {
                Ok(events) => events,
                // The end of the stream, a frame too long to be a batch, or an event
                // that does not parse: none of them leaves a stream worth reading on.
                Err(_) => break,
            },
            // Only ever a newer client, since this receiver was made after this
            // connection's own adoption. Losing a half-read batch is fine: the
            // connection is over.
            _ = current.changed() => break,
        };
        if events.is_empty() {
            continue;
        }
        if messages.send(Message::Batch(id, events)).await.is_err() {
            break;
        }
    }
    let _ = messages.send(Message::End(id)).await;
}

/// The injector's thread: the session of the current client, and nothing else.
fn inject(mut inbox: mpsc::Receiver<Message>, mut injector: Box<dyn Injector>) {
    let mut session = Session::default();
    let mut current = None;
    let mut stopped = false;
    while let Some(message) = inbox.blocking_recv() {
        let injections = match message {
            Message::Stop(done) => {
                stopped = true;
                current = None;
                let injections = session.release_all();
                if !injections.is_empty() {
                    injector.inject(injections);
                }
                let _ = done.send(());
                continue;
            }
            _ if stopped => continue,
            Message::Adopt(id) => {
                current = Some(id);
                session.release_all()
            }
            Message::Batch(id, events) if current == Some(id) => session.apply(&events),
            Message::End(id) if current == Some(id) => {
                current = None;
                session.release_all()
            }
            // A replaced client's last words, or its end: already released.
            Message::Batch(..) | Message::End(_) => continue,
        };
        if !injections.is_empty() {
            injector.inject(injections);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    use lbm_ipc::input::{encode, write_batch};
    use tokio::io::AsyncReadExt;

    use crate::remote::Injection;
    use crate::testing::{connect, endpoint, Stream};

    const C: u16 = 46;
    const LEFT_CTRL: u16 = 29;
    const BTN_LEFT: u16 = 0x110;

    /// Everything the server would have put into the system, in order.
    #[derive(Clone, Default)]
    struct Recording(Arc<Mutex<Vec<Injection>>>);

    impl Injector for Recording {
        fn inject(&mut self, batch: &[Injection]) {
            self.0.lock().unwrap().extend_from_slice(batch);
        }
    }

    impl Recording {
        /// Waits until `count` injections were made, and returns them all.
        async fn until(&self, count: usize) -> Vec<Injection> {
            let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
            loop {
                let seen = self.0.lock().unwrap().clone();
                if seen.len() >= count {
                    return seen;
                }
                assert!(
                    tokio::time::Instant::now() < deadline,
                    "waited for {count} injections, saw {seen:?}"
                );
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        }
    }

    fn serve() -> (InputServer, String, Recording) {
        let recording = Recording::default();
        let endpoint = endpoint();
        let server = start(endpoint.clone(), Box::new(recording.clone())).expect("binds");
        (server, endpoint, recording)
    }

    async fn client(endpoint: &str) -> Box<dyn Stream> {
        tokio::time::timeout(Duration::from_secs(5), connect(endpoint))
            .await
            .expect("the endpoint accepts")
    }

    async fn send(stream: &mut Box<dyn Stream>, events: &[InputEvent]) {
        write_batch(stream, &encode(events)).await.expect("sent");
    }

    /// The server closed this client's connection.
    async fn closed(stream: &mut Box<dyn Stream>) {
        let mut byte = [0u8; 1];
        let read = tokio::time::timeout(Duration::from_secs(5), stream.read(&mut byte))
            .await
            .expect("the server closes the connection");
        assert!(matches!(read, Ok(0) | Err(_)), "read {read:?}");
    }

    fn key(code: u16, pressed: bool) -> Injection {
        Injection::Key { code, pressed }
    }

    #[tokio::test]
    async fn an_entered_client_moves_clicks_scrolls_and_types() {
        let (_server, endpoint, recording) = serve();
        let mut agent = client(&endpoint).await;
        send(
            &mut agent,
            &[
                InputEvent::Enter { x: -1920, y: 40 },
                InputEvent::Motion { x: -1900, y: 50 },
            ],
        )
        .await;
        send(
            &mut agent,
            &[
                InputEvent::Button {
                    code: BTN_LEFT,
                    pressed: true,
                },
                InputEvent::Button {
                    code: BTN_LEFT,
                    pressed: false,
                },
                InputEvent::Wheel {
                    horizontal: 0,
                    vertical: -120,
                },
                InputEvent::Key {
                    code: C,
                    pressed: true,
                },
            ],
        )
        .await;
        assert_eq!(
            recording.until(6).await,
            [
                Injection::MoveTo { x: -1920, y: 40 },
                Injection::MoveTo { x: -1900, y: 50 },
                Injection::Button {
                    code: BTN_LEFT,
                    pressed: true
                },
                Injection::Button {
                    code: BTN_LEFT,
                    pressed: false
                },
                Injection::Wheel {
                    horizontal: 0,
                    vertical: -120
                },
                key(C, true),
            ]
        );
    }

    #[tokio::test]
    async fn nothing_is_injected_before_an_enter() {
        let (_server, endpoint, recording) = serve();
        let mut agent = client(&endpoint).await;
        send(
            &mut agent,
            &[
                InputEvent::Motion { x: 1, y: 1 },
                InputEvent::Key {
                    code: C,
                    pressed: true,
                },
                InputEvent::Button {
                    code: BTN_LEFT,
                    pressed: true,
                },
                InputEvent::Wheel {
                    horizontal: 120,
                    vertical: 0,
                },
            ],
        )
        .await;
        // Same connection, so this arrives after the rest: if anything above had been
        // injected, it would come first.
        send(&mut agent, &[InputEvent::Enter { x: 7, y: 8 }]).await;
        assert_eq!(recording.until(1).await, [Injection::MoveTo { x: 7, y: 8 }]);
    }

    #[tokio::test]
    async fn a_leave_releases_what_the_stream_left_pressed_and_stops_the_rest() {
        let (_server, endpoint, recording) = serve();
        let mut agent = client(&endpoint).await;
        send(
            &mut agent,
            &[
                InputEvent::Enter { x: 0, y: 0 },
                InputEvent::Key {
                    code: LEFT_CTRL,
                    pressed: true,
                },
                InputEvent::Key {
                    code: C,
                    pressed: true,
                },
                InputEvent::Button {
                    code: BTN_LEFT,
                    pressed: true,
                },
            ],
        )
        .await;
        send(&mut agent, &[InputEvent::Leave]).await;
        send(
            &mut agent,
            &[
                InputEvent::Motion { x: 3, y: 3 },
                InputEvent::Enter { x: 9, y: 9 },
            ],
        )
        .await;
        assert_eq!(
            recording.until(8).await,
            [
                Injection::MoveTo { x: 0, y: 0 },
                key(LEFT_CTRL, true),
                key(C, true),
                Injection::Button {
                    code: BTN_LEFT,
                    pressed: true
                },
                Injection::Button {
                    code: BTN_LEFT,
                    pressed: false
                },
                key(C, false),
                key(LEFT_CTRL, false),
                Injection::MoveTo { x: 9, y: 9 },
            ]
        );
    }

    #[tokio::test]
    async fn a_client_that_goes_away_leaves_nothing_pressed() {
        let (_server, endpoint, recording) = serve();
        let mut agent = client(&endpoint).await;
        send(
            &mut agent,
            &[
                InputEvent::Enter { x: 0, y: 0 },
                InputEvent::Key {
                    code: C,
                    pressed: true,
                },
            ],
        )
        .await;
        recording.until(2).await;
        drop(agent);
        assert_eq!(recording.until(3).await[2], key(C, false));
    }

    #[tokio::test]
    async fn a_newer_client_takes_over_and_the_older_ones_keys_are_released() {
        let (_server, endpoint, recording) = serve();
        let mut first = client(&endpoint).await;
        send(
            &mut first,
            &[
                InputEvent::Enter { x: 0, y: 0 },
                InputEvent::Key {
                    code: C,
                    pressed: true,
                },
            ],
        )
        .await;
        recording.until(2).await;

        let mut second = client(&endpoint).await;
        assert_eq!(recording.until(3).await[2], key(C, false));
        closed(&mut first).await;

        // The newcomer starts undriven, like any client: its own Enter is needed.
        send(
            &mut second,
            &[
                InputEvent::Key {
                    code: LEFT_CTRL,
                    pressed: true,
                },
                InputEvent::Enter { x: 4, y: 5 },
            ],
        )
        .await;
        assert_eq!(
            recording.until(4).await,
            [
                Injection::MoveTo { x: 0, y: 0 },
                key(C, true),
                key(C, false),
                Injection::MoveTo { x: 4, y: 5 },
            ]
        );
    }

    #[tokio::test]
    async fn a_garbage_batch_closes_the_client_and_releases_its_keys() {
        let (_server, endpoint, recording) = serve();
        let mut agent = client(&endpoint).await;
        send(
            &mut agent,
            &[
                InputEvent::Enter { x: 0, y: 0 },
                InputEvent::Button {
                    code: BTN_LEFT,
                    pressed: true,
                },
            ],
        )
        .await;
        recording.until(2).await;
        // An unknown event tag.
        write_batch(&mut agent, &[200, 1, 2]).await.expect("sent");
        closed(&mut agent).await;
        assert_eq!(
            recording.until(3).await[2],
            Injection::Button {
                code: BTN_LEFT,
                pressed: false
            }
        );
    }

    #[tokio::test]
    async fn a_frame_too_long_to_be_a_batch_closes_the_client() {
        let (_server, endpoint, recording) = serve();
        let mut agent = client(&endpoint).await;
        send(
            &mut agent,
            &[
                InputEvent::Enter { x: 0, y: 0 },
                InputEvent::Key {
                    code: C,
                    pressed: true,
                },
            ],
        )
        .await;
        recording.until(2).await;
        use tokio::io::AsyncWriteExt;
        agent
            .write_all(&u32::MAX.to_le_bytes())
            .await
            .expect("sent");
        closed(&mut agent).await;
        assert_eq!(recording.until(3).await[2], key(C, false));
    }

    #[tokio::test]
    async fn stopping_the_endpoint_releases_what_is_pressed_and_ignores_the_rest() {
        let (server, endpoint, recording) = serve();
        let mut agent = client(&endpoint).await;
        send(
            &mut agent,
            &[
                InputEvent::Enter { x: 0, y: 0 },
                InputEvent::Key {
                    code: C,
                    pressed: true,
                },
            ],
        )
        .await;
        recording.until(2).await;
        tokio::task::spawn_blocking(move || server.stop())
            .await
            .expect("stops");
        assert_eq!(recording.until(3).await[2], key(C, false));

        send(&mut agent, &[InputEvent::Enter { x: 1, y: 1 }]).await;
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert_eq!(recording.until(3).await.len(), 3);
    }
}
