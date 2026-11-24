//! A connection between two nodes, from the TCP connect to the keepalive.
//!
//! It comes up in two steps, because trust is decided between them:
//!
//! 1. [`connect`] or a [`Listener`] yields a [`Pending`]: TLS is done, both sides have
//!    sent `Hello`, and the caller sees the peer's fingerprint and `Hello`. Nothing
//!    else has been said — an unknown machine gets `Hello` and nothing more.
//! 2. The caller [`accept`](Pending::accept)s (sends `Paired`, waits for the peer's)
//!    or [`refuse`](Pending::refuse)s (sends `Refused`, closes). Both sides accepting
//!    makes a [`Link`].
//!
//! A `Link` runs in a task of its own and is driven through channels, so that a
//! single-threaded `select!` loop can hold many: [`Link::next`] is cancel-safe, and
//! [`LinkSender::try_input`] never waits, so relaying input never stalls the caller
//! behind a slow network. The keepalive (`Ping` when idle, lost after three silent
//! intervals) runs inside the link; the caller only sees the link close.

use std::fmt;
use std::io;
use std::net::SocketAddr;
use std::time::Duration;

use tokio::io::{AsyncWriteExt, WriteHalf};
use tokio::net::{TcpListener, TcpStream, ToSocketAddrs};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio::time::{timeout, Instant};
use tokio_rustls::{TlsAcceptor, TlsConnector, TlsStream};

use crate::identity::Identity;
use crate::tls;
use crate::wire::{self, Control, Frame, Hello, PROTOCOL};

#[cfg(test)]
mod tests;

type Stream = TlsStream<TcpStream>;

/// The timings of a link. The defaults are the protocol's; tests shrink them.
#[derive(Debug, Clone)]
pub struct Options {
    /// How long a link may stay silent before a `Ping` is sent.
    pub keepalive: Duration,
    /// How many keepalive intervals of silence from the peer mean it is gone.
    pub missed: u32,
    /// TCP connect, TLS and the `Hello` exchange, together.
    pub handshake_timeout: Duration,
    /// How long [`Pending::accept`] waits for the other side to accept — which, for a
    /// first pairing, is a user reading a fingerprint on the other machine.
    pub pairing_timeout: Duration,
    /// Frames waiting to leave. Input beyond it is refused by
    /// [`LinkSender::try_input`] rather than queued behind a stalled network.
    pub queue: usize,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            keepalive: Duration::from_secs(1),
            missed: 3,
            handshake_timeout: Duration::from_secs(10),
            pairing_timeout: Duration::from_secs(300),
            queue: 256,
        }
    }
}

impl Options {
    fn dead_after(&self) -> Duration {
        self.keepalive * self.missed.max(1)
    }
}

/// Why a link could not be made.
#[derive(Debug)]
pub enum LinkError {
    Io(io::Error),
    /// A step took longer than the [`Options`] allow.
    Timeout(&'static str),
    /// The peer said something out of turn, or speaks another protocol version.
    Protocol(String),
    /// The peer refused this node, with its reason.
    Refused(String),
    /// The peer closed the connection before the link was up.
    Closed(String),
}

impl fmt::Display for LinkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LinkError::Io(error) => write!(f, "{error}"),
            LinkError::Timeout(step) => write!(f, "timed out during {step}"),
            LinkError::Protocol(what) => write!(f, "protocol error: {what}"),
            LinkError::Refused(reason) => write!(f, "refused by the peer: {reason}"),
            LinkError::Closed(reason) => write!(f, "the peer closed the connection: {reason}"),
        }
    }
}

impl std::error::Error for LinkError {}

impl From<io::Error> for LinkError {
    fn from(error: io::Error) -> Self {
        LinkError::Io(error)
    }
}

/// Who is at the other end.
#[derive(Debug, Clone, PartialEq)]
pub struct PeerInfo {
    /// The SHA-256 of the certificate it presented: its machine id.
    pub fingerprint: String,
    pub hello: Hello,
    pub address: SocketAddr,
}

/// A node accepting connections. Each one is handshaken in a task of its own, so a
/// slow or hostile client delays nobody else.
pub struct Listener {
    local_addr: SocketAddr,
    incoming: mpsc::Receiver<Result<Pending, LinkError>>,
    task: JoinHandle<()>,
}

/// Starts listening. `hello` is what this node says to everyone who connects.
pub async fn listen(
    address: impl ToSocketAddrs,
    identity: &Identity,
    hello: Hello,
    options: Options,
) -> io::Result<Listener> {
    let acceptor = TlsAcceptor::from(tls::server_config(identity)?);
    let socket = TcpListener::bind(address).await?;
    let local_addr = socket.local_addr()?;
    let (sender, incoming) = mpsc::channel(16);
    let task = tokio::spawn(async move {
        loop {
            let (tcp, address) = match socket.accept().await {
                Ok(accepted) => accepted,
                // Out of descriptors and the like: pausing keeps this from spinning
                // until whatever it is passes.
                Err(_) => {
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    continue;
                }
            };
            let acceptor = acceptor.clone();
            let hello = hello.clone();
            let options = options.clone();
            let sender = sender.clone();
            tokio::spawn(async move {
                let pending = timeout(options.handshake_timeout, async {
                    tcp.set_nodelay(true)?;
                    let stream = acceptor.accept(tcp).await?;
                    handshake(TlsStream::Server(stream), address, &hello, options.clone()).await
                })
                .await
                .unwrap_or(Err(LinkError::Timeout("the handshake")));
                let _ = sender.send(pending).await;
            });
        }
    });
    Ok(Listener {
        local_addr,
        incoming,
        task,
    })
}

impl Listener {
    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    /// The next connection that made it through TLS and `Hello`, or why one did not
    /// (for the log: a stranger knocking is not this node's failure). Cancel-safe.
    pub async fn next(&mut self) -> Option<Result<Pending, LinkError>> {
        self.incoming.recv().await
    }
}

impl Drop for Listener {
    fn drop(&mut self) {
        self.task.abort();
    }
}

/// Connects to a node. `hello` is what this node says.
pub async fn connect(
    address: impl ToSocketAddrs,
    identity: &Identity,
    hello: Hello,
    options: Options,
) -> Result<Pending, LinkError> {
    let connector = TlsConnector::from(tls::client_config(identity)?);
    let name = rustls::pki_types::ServerName::try_from(tls::SERVER_NAME)
        .map_err(|error| LinkError::Io(io::Error::other(error)))?;
    timeout(options.handshake_timeout, async {
        let tcp = TcpStream::connect(address).await?;
        tcp.set_nodelay(true)?;
        let peer = tcp.peer_addr()?;
        let stream = connector.connect(name, tcp).await?;
        handshake(TlsStream::Client(stream), peer, &hello, options.clone()).await
    })
    .await
    .unwrap_or(Err(LinkError::Timeout("the handshake")))
}

/// Both sides say `Hello` at once and read the other's: neither waits on the other to
/// speak first, so there is no order to get wrong.
async fn handshake(
    mut stream: Stream,
    address: SocketAddr,
    hello: &Hello,
    options: Options,
) -> Result<Pending, LinkError> {
    let fingerprint = tls::peer_fingerprint(stream.get_ref().1)
        .ok_or_else(|| LinkError::Protocol("the peer presented no certificate".into()))?;
    send(&mut stream, &Control::Hello(hello.clone())).await?;
    let theirs = match wire::read_frame(&mut stream).await? {
        Frame::Control(Control::Hello(theirs)) => theirs,
        Frame::Control(Control::Refused { reason }) => return Err(LinkError::Refused(reason)),
        other => {
            return Err(LinkError::Protocol(format!(
                "expected Hello, got {}",
                describe(&other)
            )))
        }
    };
    if theirs.protocol != PROTOCOL {
        let reason = format!(
            "this node speaks protocol {PROTOCOL}, the peer protocol {}",
            theirs.protocol
        );
        let _ = send(
            &mut stream,
            &Control::Refused {
                reason: reason.clone(),
            },
        )
        .await;
        let _ = stream.shutdown().await;
        return Err(LinkError::Protocol(reason));
    }
    Ok(Pending {
        peer: PeerInfo {
            fingerprint,
            hello: theirs,
            address,
        },
        stream,
        options,
    })
}

async fn send(stream: &mut Stream, message: &Control) -> io::Result<()> {
    wire::write_frame(stream, &wire::encode_control(message)?).await
}

fn describe(frame: &Frame) -> String {
    match frame {
        Frame::Control(message) => format!("{message:?}"),
        Frame::Input(batch) => format!("an input batch of {} bytes", batch.len()),
    }
}

/// A connection whose peer has said who it is, waiting for this side to decide
/// whether it trusts it. Dropping it closes the connection without a word.
pub struct Pending {
    peer: PeerInfo,
    stream: Stream,
    options: Options,
}

impl fmt::Debug for Pending {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Pending").field("peer", &self.peer).finish()
    }
}

impl Pending {
    pub fn peer(&self) -> &PeerInfo {
        &self.peer
    }

    /// Trusts the peer: says `Paired`, then waits for the peer to say it too.
    ///
    /// That wait lasts as long as the user on the other machine takes, up to
    /// [`Options::pairing_timeout`], so a caller with an event loop spawns this rather
    /// than awaiting it inline.
    pub async fn accept(mut self) -> Result<Link, LinkError> {
        if let Err(error) = send(&mut self.stream, &Control::Paired).await {
            // The peer may have refused this node and closed already: its reason, if it
            // is still there to read, says more than the broken connection does.
            let said = timeout(
                self.options.handshake_timeout,
                wire::read_frame(&mut self.stream),
            )
            .await;
            return Err(match said {
                Ok(Ok(Frame::Control(Control::Refused { reason }))) => LinkError::Refused(reason),
                _ => LinkError::Io(error),
            });
        }
        let wait = async {
            loop {
                match wire::read_frame(&mut self.stream).await {
                    Ok(Frame::Control(Control::Paired)) => return Ok(()),
                    Ok(Frame::Control(Control::Refused { reason })) => {
                        return Err(LinkError::Refused(reason))
                    }
                    Ok(Frame::Control(Control::Bye { reason })) => {
                        return Err(LinkError::Closed(reason))
                    }
                    // A newer peer may say things this one does not know; a keepalive
                    // is harmless.
                    Ok(Frame::Control(Control::Unknown | Control::Ping | Control::Pong)) => {}
                    Ok(other) => {
                        return Err(LinkError::Protocol(format!(
                            "{} before Paired",
                            describe(&other)
                        )))
                    }
                    Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => {
                        return Err(LinkError::Closed("connection closed".into()))
                    }
                    Err(error) => return Err(LinkError::Io(error)),
                }
            }
        };
        timeout(self.options.pairing_timeout, wait)
            .await
            .unwrap_or(Err(LinkError::Timeout("pairing")))?;
        Ok(Link::start(self.peer, self.stream, self.options))
    }

    /// Does not trust the peer: says `Refused` with `reason` and closes.
    pub async fn refuse(mut self, reason: &str) {
        let refused = Control::Refused {
            reason: reason.to_owned(),
        };
        let _ = timeout(self.options.handshake_timeout, async {
            let _ = send(&mut self.stream, &refused).await;
            let _ = self.stream.shutdown().await;
            // Read what the peer still sends (its `Paired`, crossing the refusal) until it
            // closes: a connection closed with data unread is reset, and a reset can
            // throw away the `Refused` before the peer has read it — Windows does, and
            // the peer then knows only that the connection was aborted.
            while wire::read_frame(&mut self.stream).await.is_ok() {}
        })
        .await;
    }
}

/// What a link delivers.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// A control message. `Ping`, `Pong`, `Bye`, a late `Hello` or `Paired` and types
    /// this build does not know are handled inside the link and never show up here.
    Control(Control),
    /// An input batch, still encoded, to relay.
    Input(Vec<u8>),
    /// The link is over; nothing follows.
    Closed(CloseReason),
}

/// How a link ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CloseReason {
    /// The peer said `Bye`.
    Bye(String),
    /// The peer withdrew its trust (`Refused` on a live link).
    Refused(String),
    /// The peer fell silent for [`Options::missed`] keepalive intervals, or stopped
    /// taking what was sent to it.
    Lost,
    /// The connection broke or the peer sent something that is not a frame of ours.
    Failed(String),
    /// This side closed it (`LinkSender::close`, or every handle dropped).
    Local(String),
}

/// A live link to a trusted peer.
pub struct Link {
    peer: PeerInfo,
    sender: LinkSender,
    events: mpsc::Receiver<Event>,
}

impl fmt::Debug for Link {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Link").field("peer", &self.peer).finish()
    }
}

impl Link {
    fn start(peer: PeerInfo, stream: Stream, options: Options) -> Link {
        let (outgoing, queue) = mpsc::channel(options.queue.max(1));
        let (events_sender, events) = mpsc::channel(options.queue.max(1));
        tokio::spawn(drive(stream, options, queue, events_sender));
        Link {
            peer,
            sender: LinkSender { outgoing },
            events,
        }
    }

    pub fn peer(&self) -> &PeerInfo {
        &self.peer
    }

    /// A handle to send on this link; clone it freely.
    pub fn sender(&self) -> LinkSender {
        self.sender.clone()
    }

    /// The next thing the peer sent, or how the link ended; `None` after that.
    /// Cancel-safe.
    pub async fn next(&mut self) -> Option<Event> {
        self.events.recv().await
    }

    /// The parts, for a caller that merges the events of several links itself.
    pub fn into_parts(self) -> (PeerInfo, LinkSender, mpsc::Receiver<Event>) {
        (self.peer, self.sender, self.events)
    }
}

/// Why something could not be sent.
#[derive(Debug)]
pub enum SendError {
    /// The link is over.
    Closed,
    /// The queue is full: the network is not keeping up. Only [`LinkSender::try_input`]
    /// says this.
    Full,
    /// Not something a frame can carry (too large).
    Invalid(io::Error),
}

impl fmt::Display for SendError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SendError::Closed => write!(f, "the link is closed"),
            SendError::Full => write!(f, "the link's queue is full"),
            SendError::Invalid(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for SendError {}

enum Outgoing {
    Frame(Vec<u8>),
    Close(String),
}

/// The sending side of a [`Link`]. Frames leave in the order they were queued,
/// whichever method queued them.
#[derive(Clone, Debug)]
pub struct LinkSender {
    outgoing: mpsc::Sender<Outgoing>,
}

impl LinkSender {
    /// Queues a control message, waiting for room if the queue is full.
    pub async fn control(&self, message: &Control) -> Result<(), SendError> {
        let frame = wire::encode_control(message).map_err(SendError::Invalid)?;
        self.outgoing
            .send(Outgoing::Frame(frame))
            .await
            .map_err(|_| SendError::Closed)
    }

    /// Queues an input batch, waiting for room if the queue is full.
    pub async fn input(&self, batch: &[u8]) -> Result<(), SendError> {
        let frame = wire::encode_input(batch).map_err(SendError::Invalid)?;
        self.outgoing
            .send(Outgoing::Frame(frame))
            .await
            .map_err(|_| SendError::Closed)
    }

    /// Queues an input batch if there is room, without waiting: what the relay of a
    /// thousand reports a second uses, so that a congested link costs input on that
    /// link only and never the caller's loop.
    pub fn try_input(&self, batch: &[u8]) -> Result<(), SendError> {
        let frame = wire::encode_input(batch).map_err(SendError::Invalid)?;
        self.outgoing
            .try_send(Outgoing::Frame(frame))
            .map_err(|error| match error {
                mpsc::error::TrySendError::Full(_) => SendError::Full,
                mpsc::error::TrySendError::Closed(_) => SendError::Closed,
            })
    }

    /// Says `Bye` with `reason` after what is already queued, and closes.
    pub async fn close(&self, reason: &str) {
        let _ = self.outgoing.send(Outgoing::Close(reason.to_owned())).await;
    }

    pub fn is_closed(&self) -> bool {
        self.outgoing.is_closed()
    }
}

/// The link's task: reads in a task of its own (a frame read halfway cannot be
/// abandoned in a `select!`), writes, keeps alive, and reports how it ended.
async fn drive(
    stream: Stream,
    options: Options,
    mut outgoing: mpsc::Receiver<Outgoing>,
    events: mpsc::Sender<Event>,
) {
    let (mut reader, mut writer) = tokio::io::split(stream);
    let (frames, mut incoming) = mpsc::channel(32);
    let reading = tokio::spawn(async move {
        loop {
            let frame = wire::read_frame(&mut reader).await;
            let failed = frame.is_err();
            if frames.send(frame).await.is_err() || failed {
                break;
            }
        }
    });

    let reason = run(&mut writer, &options, &mut outgoing, &mut incoming, &events).await;
    reading.abort();
    let _ = timeout(options.keepalive, writer.shutdown()).await;
    let _ = events.send(Event::Closed(reason)).await;
}

/// A write that cannot finish is a peer that stopped reading: as gone as one that
/// stopped talking, and the same patience applies.
async fn write(
    writer: &mut WriteHalf<Stream>,
    frame: Vec<u8>,
    dead_after: Duration,
) -> Result<(), CloseReason> {
    match timeout(dead_after, wire::write_frame(writer, &frame)).await {
        Ok(Ok(())) => Ok(()),
        Ok(Err(error)) => Err(CloseReason::Failed(error.to_string())),
        Err(_) => Err(CloseReason::Lost),
    }
}

async fn run(
    writer: &mut WriteHalf<Stream>,
    options: &Options,
    outgoing: &mut mpsc::Receiver<Outgoing>,
    incoming: &mut mpsc::Receiver<io::Result<Frame>>,
    events: &mpsc::Sender<Event>,
) -> CloseReason {
    let dead_after = options.dead_after();
    let control = |message: Control| wire::encode_control(&message).unwrap_or_default();

    // Ticking at half the interval keeps the peer's longest silence near one and a half
    // intervals, well inside the three it allows.
    let mut tick = tokio::time::interval(options.keepalive / 2);
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let mut last_received = Instant::now();
    let mut last_sent = Instant::now();
    loop {
        tokio::select! {
            frame = incoming.recv() => {
                let frame = match frame {
                    Some(Ok(frame)) => frame,
                    Some(Err(error)) if error.kind() == io::ErrorKind::UnexpectedEof => {
                        return CloseReason::Failed("the peer closed the connection".into())
                    }
                    Some(Err(error)) => return CloseReason::Failed(error.to_string()),
                    None => return CloseReason::Failed("the reader stopped".into()),
                };
                last_received = Instant::now();
                let event = match frame {
                    Frame::Input(batch) => Event::Input(batch),
                    Frame::Control(Control::Ping) => {
                        if let Err(reason) = write(writer, control(Control::Pong), dead_after).await {
                            return reason;
                        }
                        last_sent = Instant::now();
                        continue;
                    }
                    Frame::Control(Control::Bye { reason }) => return CloseReason::Bye(reason),
                    Frame::Control(Control::Refused { reason }) => {
                        return CloseReason::Refused(reason)
                    }
                    Frame::Control(
                        Control::Pong | Control::Unknown | Control::Hello(_) | Control::Paired,
                    ) => continue,
                    Frame::Control(message) => Event::Control(message),
                };
                if events.send(event).await.is_err() {
                    return CloseReason::Local("nobody listens to this link".into());
                }
            }
            queued = outgoing.recv() => {
                let (frame, closing) = match queued {
                    Some(Outgoing::Frame(frame)) => (frame, None),
                    Some(Outgoing::Close(reason)) => {
                        (control(Control::Bye { reason: reason.clone() }), Some(reason))
                    }
                    None => {
                        let reason = "closed".to_owned();
                        (control(Control::Bye { reason: reason.clone() }), Some(reason))
                    }
                };
                let written = write(writer, frame, dead_after).await;
                if let Some(reason) = closing {
                    return CloseReason::Local(reason);
                }
                if let Err(reason) = written {
                    return reason;
                }
                last_sent = Instant::now();
            }
            _ = events.closed() => {
                let _ = write(writer, control(Control::Bye { reason: "closed".into() }), dead_after).await;
                return CloseReason::Local("nobody listens to this link".into());
            }
            _ = tick.tick() => {
                if last_received.elapsed() >= dead_after {
                    return CloseReason::Lost;
                }
                if last_sent.elapsed() >= options.keepalive {
                    if let Err(reason) = write(writer, control(Control::Ping), dead_after).await {
                        return reason;
                    }
                    last_sent = Instant::now();
                }
            }
        }
    }
}
