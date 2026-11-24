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
use std::time::{Duration, Instant};

use lbm_ipc::input::{decode, read_batch, write_batch, InputEvent};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::{mpsc, watch, Notify};

use crate::ipc::transport;
use crate::remote::{deliver, Injector, Session};

/// Batches waiting for the injector. A device report is one batch, so this is a few
/// dozen milliseconds of a fast mouse — enough to ride out a slow system call, short
/// enough that a stalled injector pushes back on the agent instead of piling up.
const QUEUE: usize = 64;

/// How often an injector holding input back is looked at while nothing arrives.
const CATCH_UP_POLL: Duration = Duration::from_millis(5);

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

    /// Which client there is to drive, if any. Two connections never share a value:
    /// a crossing begun with one client is not continued with its successor, which
    /// never read the `Enter`.
    pub fn client(&self) -> Option<u64> {
        self.client
            .as_ref()
            .filter(|client| !client.batches.is_closed())
            .map(|client| client.id)
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
    loop {
        // Input held back by the injector is put in when it is due, whether or not
        // more arrives; until then the stream is still read, never left to back up.
        // Once stopped, nothing held back is ever put in: it was discarded.
        let due = if stopped { None } else { injector.due() };
        let message = match due {
            None => match inbox.blocking_recv() {
                Some(message) => message,
                None => return,
            },
            Some(due) => match inbox.try_recv() {
                Ok(message) => message,
                Err(mpsc::error::TryRecvError::Empty) => {
                    let now = Instant::now();
                    if now >= due {
                        injector.catch_up();
                    } else {
                        std::thread::sleep((due - now).min(CATCH_UP_POLL));
                    }
                    continue;
                }
                Err(mpsc::error::TryRecvError::Disconnected) => return,
            },
        };
        let injections = match message {
            Message::Stop(done) => {
                stopped = true;
                current = None;
                deliver(injector.as_mut(), session.release_all());
                // What was held back never reached the system — releases included,
                // when the presses are held back with them — and must not reach it
                // after the caller is told injection stopped.
                injector.discard();
                // The hook is leaving: whatever the session's end parked must not stay
                // hidden after it.
                injector.unpark();
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
        deliver(injector.as_mut(), injections);
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

    /// Everything the server would have put into the system, in order, its parkings
    /// and unparkings included at their place.
    #[derive(Clone, Default)]
    struct Recording(Arc<Mutex<Vec<Injection>>>);

    impl Injector for Recording {
        fn inject(&mut self, batch: &[Injection]) {
            assert!(
                !batch
                    .iter()
                    .any(|i| matches!(i, Injection::Park | Injection::Unpark)),
                "a parking reached inject: {batch:?}"
            );
            self.0.lock().unwrap().extend_from_slice(batch);
        }
        fn park(&mut self) {
            self.0.lock().unwrap().push(Injection::Park);
        }
        fn unpark(&mut self) {
            self.0.lock().unwrap().push(Injection::Unpark);
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

        /// Waits until `count` injections were made, lets the server run on a little,
        /// and returns them all: anything made after the first `count` is there too.
        async fn settled(&self, count: usize) -> Vec<Injection> {
            self.until(count).await;
            tokio::time::sleep(Duration::from_millis(100)).await;
            self.0.lock().unwrap().clone()
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
            recording.until(7).await,
            [
                Injection::Unpark,
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
        assert_eq!(
            recording.until(2).await,
            [Injection::Unpark, Injection::MoveTo { x: 7, y: 8 }]
        );
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
            recording.until(11).await,
            [
                Injection::Unpark,
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
                Injection::Park,
                Injection::Unpark,
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
        recording.until(3).await;
        drop(agent);
        assert_eq!(
            recording.settled(5).await[3..],
            [key(C, false), Injection::Park]
        );
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
        recording.until(3).await;

        let mut second = client(&endpoint).await;
        assert_eq!(
            recording.until(5).await[3..],
            [key(C, false), Injection::Park]
        );
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
        // Parked once: the replaced client's own end, which follows, parks nothing.
        assert_eq!(
            recording.settled(7).await,
            [
                Injection::Unpark,
                Injection::MoveTo { x: 0, y: 0 },
                key(C, true),
                key(C, false),
                Injection::Park,
                Injection::Unpark,
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
        recording.until(3).await;
        // An unknown event tag.
        write_batch(&mut agent, &[200, 1, 2]).await.expect("sent");
        closed(&mut agent).await;
        assert_eq!(
            recording.settled(5).await[3..],
            [
                Injection::Button {
                    code: BTN_LEFT,
                    pressed: false
                },
                Injection::Park,
            ]
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
        recording.until(3).await;
        use tokio::io::AsyncWriteExt;
        agent
            .write_all(&u32::MAX.to_le_bytes())
            .await
            .expect("sent");
        closed(&mut agent).await;
        assert_eq!(
            recording.settled(5).await[3..],
            [key(C, false), Injection::Park]
        );
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
        recording.until(3).await;
        tokio::task::spawn_blocking(move || server.stop())
            .await
            .expect("stops");
        // Parked as any session's end is, then given back: the hook is leaving, and
        // nothing it hid may stay hidden.
        assert_eq!(
            recording.until(6).await[3..],
            [key(C, false), Injection::Park, Injection::Unpark]
        );

        send(&mut agent, &[InputEvent::Enter { x: 1, y: 1 }]).await;
        assert_eq!(recording.settled(6).await.len(), 6);
    }

    #[tokio::test]
    async fn a_client_that_goes_away_after_its_leave_is_parked_only_once() {
        let (_server, endpoint, recording) = serve();
        let mut agent = client(&endpoint).await;
        send(
            &mut agent,
            &[InputEvent::Enter { x: 0, y: 0 }, InputEvent::Leave],
        )
        .await;
        recording.until(3).await;
        drop(agent);
        assert_eq!(
            recording.settled(3).await,
            [
                Injection::Unpark,
                Injection::MoveTo { x: 0, y: 0 },
                Injection::Park
            ]
        );
    }

    #[tokio::test]
    async fn a_client_that_never_entered_is_never_parked() {
        let (server, endpoint, recording) = serve();
        let agent = client(&endpoint).await;
        let second = client(&endpoint).await;
        drop(agent);
        drop(second);
        tokio::task::spawn_blocking(move || server.stop())
            .await
            .expect("stops");
        // Only the unpark of the hook's way out, harmless with nothing parked.
        assert_eq!(recording.settled(1).await, [Injection::Unpark]);
    }

    /// An injector whose devices are too young: it holds everything back until a
    /// deadline set at its first batch, and counts the batches it was handed.
    #[derive(Clone)]
    struct Holding {
        hold: Duration,
        state: Arc<Mutex<HoldingState>>,
    }

    #[derive(Default)]
    struct HoldingState {
        until: Option<Instant>,
        held: Vec<Injection>,
        handed: usize,
        written: Vec<Injection>,
        written_at: Option<Instant>,
    }

    impl Injector for Holding {
        fn inject(&mut self, batch: &[Injection]) {
            let mut state = self.state.lock().unwrap();
            state.handed += 1;
            let until = *state.until.get_or_insert(Instant::now() + self.hold);
            if Instant::now() < until {
                state.held.extend_from_slice(batch);
            } else {
                let held = std::mem::take(&mut state.held);
                state.written.extend(held);
                state.written.extend_from_slice(batch);
            }
        }
        fn park(&mut self) {}
        fn unpark(&mut self) {}
        fn due(&self) -> Option<Instant> {
            let state = self.state.lock().unwrap();
            (!state.held.is_empty()).then(|| state.until.unwrap())
        }
        fn catch_up(&mut self) {
            let mut state = self.state.lock().unwrap();
            let held = std::mem::take(&mut state.held);
            state.written.extend(held);
            state.written_at.get_or_insert_with(Instant::now);
        }
        fn discard(&mut self) {
            self.state.lock().unwrap().held.clear();
        }
    }

    impl Holding {
        fn new(hold: Duration) -> Self {
            Self {
                hold,
                state: Arc::default(),
            }
        }

        async fn until_handed(&self, count: usize) {
            let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
            while self.state.lock().unwrap().handed < count {
                assert!(tokio::time::Instant::now() < deadline, "not handed {count}");
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        }
    }

    fn press_left() -> InputEvent {
        InputEvent::Button {
            code: BTN_LEFT,
            pressed: true,
        }
    }

    fn serve_holding(hold: Duration) -> (InputServer, String, Holding) {
        let holding = Holding::new(hold);
        let endpoint = endpoint();
        let server = start(
            endpoint.clone(),
            Box::new(holding.clone()),
            Arc::new(Outlet::default()),
        )
        .expect("binds");
        (server, endpoint, holding)
    }

    /// Stopped while the devices settle: what was held back is dropped before the
    /// caller hears injection stopped, and nothing is put in at the deadline either —
    /// the press never reached the system, so neither may its release.
    #[tokio::test]
    async fn stopping_while_input_is_held_back_drops_it_for_good() {
        let hold = Duration::from_millis(400);
        let (server, endpoint, holding) = serve_holding(hold);
        let mut agent = client(&endpoint).await;
        send(
            &mut agent,
            &[InputEvent::Enter { x: 0, y: 0 }, press_left()],
        )
        .await;
        holding.until_handed(1).await;
        tokio::task::spawn_blocking(move || server.stop())
            .await
            .expect("stops");
        assert!(holding.state.lock().unwrap().written.is_empty());
        tokio::time::sleep(hold + Duration::from_millis(200)).await;
        let state = holding.state.lock().unwrap();
        assert!(
            state.written.is_empty(),
            "put in after the stop: {:?}",
            state.written
        );
        assert!(state.held.is_empty());
    }

    /// Stopped with settled devices: the press reached the system, so its release does,
    /// before the caller hears injection stopped.
    #[tokio::test]
    async fn stopping_with_settled_devices_releases_before_it_returns() {
        let (server, endpoint, holding) = serve_holding(Duration::ZERO);
        let mut agent = client(&endpoint).await;
        send(
            &mut agent,
            &[InputEvent::Enter { x: 0, y: 0 }, press_left()],
        )
        .await;
        holding.until_handed(1).await;
        tokio::task::spawn_blocking(move || server.stop())
            .await
            .expect("stops");
        assert_eq!(
            holding.state.lock().unwrap().written,
            [
                Injection::MoveTo { x: 0, y: 0 },
                Injection::Button {
                    code: BTN_LEFT,
                    pressed: true
                },
                Injection::Button {
                    code: BTN_LEFT,
                    pressed: false
                },
            ]
        );
    }

    /// A master's first crossing at 1 kHz while the receiving devices settle: every
    /// batch is read as it comes — far past the endpoint's queue — and what was held
    /// back is put in when due, with nothing more arriving to prompt it.
    #[tokio::test]
    async fn input_held_back_by_the_injector_neither_backs_the_stream_up_nor_waits_for_more() {
        const MOTIONS: i32 = 4 * QUEUE as i32;
        let holding = Holding::new(Duration::from_millis(1500));
        let endpoint = endpoint();
        let _server = start(
            endpoint.clone(),
            Box::new(holding.clone()),
            Arc::new(Outlet::default()),
        )
        .expect("binds");
        let mut agent = client(&endpoint).await;
        send(&mut agent, &[InputEvent::Enter { x: 0, y: 0 }]).await;
        for i in 1..=MOTIONS {
            send(&mut agent, &[InputEvent::Motion { x: i, y: i }]).await;
            if i % 16 == 0 {
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
        }
        send(
            &mut agent,
            &[
                InputEvent::Key {
                    code: C,
                    pressed: true,
                },
                InputEvent::Key {
                    code: C,
                    pressed: false,
                },
            ],
        )
        .await;

        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        while holding.state.lock().unwrap().handed < MOTIONS as usize + 2 {
            assert!(
                tokio::time::Instant::now() < deadline,
                "the stream backed up"
            );
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        {
            let state = holding.state.lock().unwrap();
            assert!(
                Instant::now() < state.until.unwrap(),
                "every batch was read while the injector held them back"
            );
            assert!(state.written.is_empty(), "nothing put in early");
        }

        while holding.state.lock().unwrap().written_at.is_none() {
            assert!(tokio::time::Instant::now() < deadline, "never caught up");
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        let state = holding.state.lock().unwrap();
        assert!(state.written_at.unwrap() >= state.until.unwrap());
        assert_eq!(state.written.len(), MOTIONS as usize + 3);
        assert_eq!(state.written[0], Injection::MoveTo { x: 0, y: 0 });
        assert_eq!(
            state.written[state.written.len() - 2..],
            [key(C, true), key(C, false)]
        );
    }

    /// Which of the session calls an injector was asked for, in order.
    #[derive(Clone, Default)]
    struct SessionCalls(Arc<Mutex<Vec<&'static str>>>);

    impl Injector for SessionCalls {
        fn inject(&mut self, _batch: &[Injection]) {}
        fn park(&mut self) {
            self.0.lock().unwrap().push("park");
        }
        fn unpark(&mut self) {
            self.0.lock().unwrap().push("unpark");
        }
        fn begin(&mut self) {
            self.0.lock().unwrap().push("begin");
        }
    }

    /// Preparing a session (on Linux, building virtual devices) is for a session that
    /// starts, never for the hook's way out, which only gives back what was hidden.
    #[tokio::test]
    async fn a_session_is_prepared_when_it_starts_and_never_on_the_way_out() {
        let calls = SessionCalls::default();
        let endpoint = endpoint();
        let server = start(
            endpoint.clone(),
            Box::new(calls.clone()),
            Arc::new(Outlet::default()),
        )
        .expect("binds");
        let mut agent = client(&endpoint).await;
        send(
            &mut agent,
            &[InputEvent::Enter { x: 0, y: 0 }, InputEvent::Leave],
        )
        .await;
        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        while calls.0.lock().unwrap().len() < 2 {
            assert!(tokio::time::Instant::now() < deadline);
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        tokio::task::spawn_blocking(move || server.stop())
            .await
            .expect("stops");
        assert_eq!(*calls.0.lock().unwrap(), ["begin", "park", "unpark"]);
    }
}
