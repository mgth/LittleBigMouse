//! The input endpoint of the multi-PC mode (docs/multi-pc.md): the second endpoint
//! this hook serves, beside its command endpoint (`lbm_ipc::input::input_endpoint_beside`).
//!
//! One client, the agent, and the newest connection wins, as on the command endpoint:
//! an agent that restarted must not wait for the system to notice its predecessor's
//! connection is dead. What travels are `lbm_ipc::input` batches — four-byte
//! little-endian length, then whole events — and on a remote they go one way, agent
//! to hook to system, through a [`Session`] and the platform's [`Injector`].
//!
//! The connection is a full duplex stream on purpose: on a master, the hook writes the
//! batches it captured while the pointer is on the remote to the agent over the same
//! one (lot 6). They reach the connection through the [`Outlet`]: the routing thread
//! queues an encoded batch with a `try_send` into a bounded queue, and a task of the
//! endpoint's runtime writes it. A queue that is full means the agent stopped reading;
//! the routing thread then gives up on that client ([`Driver::send`]), which closes its
//! connection, rather than wait or drop part of a stream that presses and releases.
//!
//! Two threads, neither of them the routing thread. The endpoint's own, with a small
//! runtime, reads, decodes and writes; the injector's, which owns the session, does the
//! system calls. A stalled injection holds back the reader — the stream is bounded —
//! and never the mouse this machine routes.

use std::io;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc as std_mpsc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use lbm_ipc::input::{decode, read_batch, write_batch, InputEvent};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::{mpsc, watch, Notify};

use crate::ipc::transport;
use crate::remote::{Injector, Session};

/// Batches waiting for the injector. A device report is one batch, so this is a few
/// dozen milliseconds of a fast mouse — enough to ride out a slow system call, short
/// enough that a stalled injector pushes back on the agent instead of piling up.
const QUEUE: usize = 64;

/// Batches waiting to be written to the agent, on a master. The same reasoning as
/// [`QUEUE`], from the other side: a quarter of a second of a 1 kHz mouse is more than
/// any healthy agent lets pile up, and an agent that does is one the pointer should
/// come home from rather than wait for.
pub const OUT_QUEUE: usize = 256;

/// The client a master's routing thread writes to: its queue, and the way to close
/// its connection.
#[derive(Clone)]
pub struct Client {
    id: ClientId,
    batches: mpsc::Sender<Vec<u8>>,
    abandon: Arc<Notify>,
}

/// Where the endpoint publishes its current client for the routing thread.
///
/// The routing thread must never block, so it does not read this on every report: it
/// compares [`Outlet::generation`], one atomic load, with the generation it last took a
/// copy at, and only takes the lock — with `try_lock` — when they differ.
#[derive(Default)]
pub struct Outlet {
    generation: AtomicU64,
    current: Mutex<Option<Client>>,
}

impl Outlet {
    fn adopt(&self, client: Client) {
        *self.current.lock().unwrap_or_else(|p| p.into_inner()) = Some(client);
        self.generation.fetch_add(1, Ordering::Release);
    }

    /// A connection ended: if it was the current one, there is no client any more. A
    /// replaced connection's end leaves its successor in place.
    fn end(&self, id: ClientId) {
        let mut current = self.current.lock().unwrap_or_else(|p| p.into_inner());
        if current.as_ref().is_some_and(|client| client.id == id) {
            *current = None;
            self.generation.fetch_add(1, Ordering::Release);
        }
    }
}

/// The routing thread's side of the [`Outlet`]: its own copy of the current client,
/// refreshed when the endpoint says it changed.
#[derive(Default)]
pub struct Driver {
    seen: Option<u64>,
    client: Option<Client>,
}

impl Driver {
    /// Catches up with the endpoint. An atomic load when nothing changed; a `try_lock`
    /// when something did, and on contention the next call tries again.
    pub fn refresh(&mut self, outlet: &Outlet) {
        let generation = outlet.generation.load(Ordering::Acquire);
        if self.seen == Some(generation) {
            return;
        }
        if let Ok(current) = outlet.current.try_lock() {
            self.client = current.clone();
            self.seen = Some(generation);
        }
    }

    /// Whether there is a client to drive. A connection that ended is noticed here
    /// even before the endpoint's news is: its queue is closed.
    pub fn connected(&self) -> bool {
        self.client
            .as_ref()
            .is_some_and(|client| !client.batches.is_closed())
    }

    /// Queues one encoded batch without waiting. `false` when the batch will never be
    /// written: no client, a connection that ended, or a queue that is full. A full
    /// queue closes the connection — the stream already lost an event and cannot be
    /// trusted to release what it pressed — and the client is forgotten until the
    /// endpoint names another.
    pub fn send(&mut self, batch: Vec<u8>) -> bool {
        let Some(client) = &self.client else {
            return false;
        };
        match client.batches.try_send(batch) {
            Ok(()) => true,
            Err(_) => {
                self.abandon();
                false
            }
        }
    }

    /// Closes the current client's connection and forgets it.
    pub fn abandon(&mut self) {
        if let Some(client) = self.client.take() {
            client.abandon.notify_one();
        }
    }
}

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

/// Serves the input endpoint at `endpoint`, injecting through `injector` and publishing
/// its current client in `outlet`. Binds before returning, so a client may connect as
/// soon as this has returned.
pub fn start(
    endpoint: String,
    injector: Box<dyn Injector>,
    outlet: Arc<Outlet>,
) -> io::Result<InputServer> {
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
                accept(&mut acceptor, accepted, outlet).await;
            });
        })?;

    ready_rx
        .recv()
        .map_err(|_| io::Error::other("input endpoint thread exited before binding"))??;

    Ok(InputServer { messages, endpoint })
}

/// Hands every connection the endpoint, newest first.
///
/// The adoption is announced to the injector and published in the outlet here, in
/// accepting order, before the connection can send anything: the injector then cannot
/// see a batch of the new client ahead of the news that it is the one to listen to, and
/// the routing thread cannot be handed an older client after a newer one.
async fn accept(
    acceptor: &mut transport::Acceptor,
    messages: mpsc::Sender<Message>,
    outlet: Arc<Outlet>,
) {
    let (current, _) = watch::channel(0);
    let mut next: ClientId = 1;
    while let Some(stream) = acceptor.accept().await {
        let id = next;
        next += 1;
        if messages.send(Message::Adopt(id)).await.is_err() {
            return;
        }
        current.send_replace(id);
        let (batches, outgoing) = mpsc::channel(OUT_QUEUE);
        let abandon = Arc::new(Notify::new());
        outlet.adopt(Client {
            id,
            batches,
            abandon: abandon.clone(),
        });
        tokio::spawn(connection(
            stream,
            id,
            current.subscribe(),
            messages.clone(),
            Outgoing {
                batches: outgoing,
                abandon,
                outlet: outlet.clone(),
            },
        ));
    }
}

/// What a connection writes, and how it is told to stop.
struct Outgoing {
    batches: mpsc::Receiver<Vec<u8>>,
    abandon: Arc<Notify>,
    outlet: Arc<Outlet>,
}

/// One client's connection, until it ends, sends what is not a batch, cannot be written
/// to, is abandoned by the routing thread, or a newer one takes the endpoint.
///
/// Reading and writing are two futures of one task, raced against each other and
/// against the two reasons to stop: whichever ends first ends the connection as a
/// whole, so the client never sees half of one. Neither is cancelled while the other
/// lives — `read_batch` is not cancel-safe, and losing half a frame mid-stream would
/// desynchronise everything read after it.
async fn connection<S>(
    stream: S,
    id: ClientId,
    mut current: watch::Receiver<ClientId>,
    messages: mpsc::Sender<Message>,
    outgoing: Outgoing,
) where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let Outgoing {
        mut batches,
        abandon,
        outlet,
    } = outgoing;
    let (mut reader, mut writer) = tokio::io::split(stream);
    let reading = async {
        loop {
            let events = match read_batch(&mut reader)
                .await
                .and_then(|batch| decode(&batch))
            {
                Ok(events) => events,
                // The end of the stream, a frame too long to be a batch, or an event
                // that does not parse: none of them leaves a stream worth reading on.
                Err(_) => break,
            };
            if events.is_empty() {
                continue;
            }
            if messages.send(Message::Batch(id, events)).await.is_err() {
                break;
            }
        }
    };
    let writing = async {
        while let Some(batch) = batches.recv().await {
            if write_batch(&mut writer, &batch).await.is_err() {
                break;
            }
        }
    };
    tokio::select! {
        () = reading => {}
        () = writing => {}
        // Only ever a newer client, since this receiver was made after this
        // connection's own adoption. Losing a half-read batch is fine: the connection
        // is over.
        _ = current.changed() => {}
        // The routing thread gave up on this client: its queue was full.
        () = abandon.notified() => {}
    }
    // Before the injector hears of it: the routing thread is to stop writing to a
    // client that is gone as early as it can know.
    outlet.end(id);
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
        let server = start(
            endpoint.clone(),
            Box::new(recording.clone()),
            Arc::new(Outlet::default()),
        )
        .expect("binds");
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

    /// A driver that has caught up with `outlet`'s current client, or the test fails.
    async fn driving(outlet: &Outlet) -> Driver {
        let mut driver = Driver::default();
        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        while !driver.connected() {
            assert!(
                tokio::time::Instant::now() < deadline,
                "the client was never published"
            );
            tokio::time::sleep(Duration::from_millis(5)).await;
            driver.refresh(outlet);
        }
        driver
    }

    fn serve_master() -> (InputServer, String, Arc<Outlet>) {
        let outlet = Arc::new(Outlet::default());
        let endpoint = endpoint();
        let server = start(
            endpoint.clone(),
            Box::new(Recording::default()),
            outlet.clone(),
        )
        .expect("binds");
        (server, endpoint, outlet)
    }

    #[tokio::test]
    async fn a_master_writes_its_batches_to_the_client_and_stops_when_it_goes() {
        let (_server, endpoint, outlet) = serve_master();
        let mut agent = client(&endpoint).await;
        let mut driver = driving(&outlet).await;

        let batch = encode(&[InputEvent::Enter { x: 1, y: 2 }]);
        assert!(driver.send(batch.clone()));
        let read = tokio::time::timeout(Duration::from_secs(5), read_batch(&mut agent))
            .await
            .expect("the batch arrives")
            .expect("read");
        assert_eq!(read, batch);

        drop(agent);
        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        while driver.connected() {
            assert!(
                tokio::time::Instant::now() < deadline,
                "the client's end was never noticed"
            );
            tokio::time::sleep(Duration::from_millis(5)).await;
            driver.refresh(&outlet);
        }
        assert!(!driver.send(batch), "nobody to write to");
    }

    #[tokio::test]
    async fn a_newer_client_is_the_one_a_master_writes_to() {
        let (_server, endpoint, outlet) = serve_master();
        let mut first = client(&endpoint).await;
        let mut driver = driving(&outlet).await;
        let mut second = client(&endpoint).await;
        closed(&mut first).await;

        driver.refresh(&outlet);
        let batch = encode(&[InputEvent::Leave]);
        assert!(driver.send(batch.clone()));
        let read = tokio::time::timeout(Duration::from_secs(5), read_batch(&mut second))
            .await
            .expect("the batch arrives at the newer client")
            .expect("read");
        assert_eq!(read, batch);
    }

    #[tokio::test]
    async fn a_full_queue_gives_the_client_up_and_closes_its_connection() {
        let (batches, mut outgoing) = mpsc::channel(1);
        let abandon = Arc::new(Notify::new());
        let outlet = Outlet::default();
        outlet.adopt(Client {
            id: 7,
            batches,
            abandon: abandon.clone(),
        });
        let mut driver = Driver::default();
        driver.refresh(&outlet);
        assert!(driver.connected());

        assert!(driver.send(vec![2]));
        assert!(!driver.send(vec![2]), "the queue holds one batch");
        assert!(!driver.connected(), "the client is given up");
        tokio::time::timeout(Duration::from_secs(5), abandon.notified())
            .await
            .expect("its connection is told to close");
        // Given up, not lost from the endpoint's view: the queued batch is still there.
        assert_eq!(outgoing.recv().await, Some(vec![2]));
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
