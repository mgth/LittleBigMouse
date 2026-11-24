//! The input stream of the multi-PC mode (docs/multi-pc.md): what a master's hook
//! captures and a remote's hook injects.
//!
//! One vocabulary for the three hops it crosses — master hook → master agent, agent →
//! agent over the network, remote agent → remote hook — so that the agents relay
//! batches without ever reading them. It is binary, unlike the rest of this crate: a
//! mouse reports a thousand times a second, and these frames are the only ones on the
//! product's hot path that leave the process.
//!
//! A frame is a four-byte little-endian length and that many bytes, holding whole
//! events back to back: everything one device report produced travels, and is
//! injected, together. Codes are Linux input event codes (`BTN_LEFT` = 0x110,
//! `KEY_A` = 30, …) whatever the two systems are: they name physical keys, which is
//! what a remote keyboard layout has to be applied to.

use std::borrow::Cow;
use std::io;

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

/// The version of this vocabulary, announced on each hop's handshake.
pub const PROTOCOL: u32 = 1;

/// A batch is one device report: far below this. A longer frame is not ours.
pub const MAX_BATCH_SIZE: usize = 64 * 1024;

/// The Linux socket the hook serves the stream on, beside its command socket.
pub const INPUT_SOCKET_NAME: &str = "littlebigmouse-input-v1.sock";

/// The Windows pipe the hook serves the stream on, per logon session.
pub fn input_pipe_name(session: u32) -> String {
    format!(r"\\.\pipe\LittleBigMouse-Input-v1-session-{session}")
}

/// The input endpoint that goes with a hook command endpoint: the default names side
/// by side, and for an overridden endpoint (tests, side-by-side instances) the same
/// name with `-input` appended — so one override moves both.
pub fn input_endpoint_beside(command_endpoint: &str) -> String {
    if let Some(prefix) = command_endpoint.strip_suffix(crate::endpoint::SOCKET_NAME) {
        return format!("{prefix}{INPUT_SOCKET_NAME}");
    }
    if let Some(session) = command_endpoint.strip_prefix(r"\\.\pipe\LittleBigMouse-v1-session-") {
        if let Ok(session) = session.parse() {
            return input_pipe_name(session);
        }
    }
    format!("{command_endpoint}-input")
}

/// The capability (`lbm_net::wire::Hello::capabilities`) of a node whose hook reads
/// [`InputEvent::Locks`]. A hook that does not fails the whole batch on it, so a master
/// sends it only to a remote that says this ([`without_locks`]).
pub const LOCKS_CAPABILITY: &str = "InputLocks";

/// The lock keys of the sender, each `None` when it cannot tell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Locks {
    pub caps: Option<bool>,
    pub num: Option<bool>,
    pub scroll: Option<bool>,
}

const CAPS_LOCK: u8 = 1;
const NUM_LOCK: u8 = 2;
const SCROLL_LOCK: u8 = 4;

impl Locks {
    /// Two bytes: which locks are known, then which of those are on.
    fn bits(self) -> [u8; 2] {
        let mut known = 0;
        let mut on = 0;
        for (lock, bit) in [
            (self.caps, CAPS_LOCK),
            (self.num, NUM_LOCK),
            (self.scroll, SCROLL_LOCK),
        ] {
            if let Some(state) = lock {
                known |= bit;
                if state {
                    on |= bit;
                }
            }
        }
        [known, on]
    }

    fn from_bits([known, on]: [u8; 2]) -> Locks {
        let lock = |bit: u8| (known & bit != 0).then_some(on & bit != 0);
        Locks {
            caps: lock(CAPS_LOCK),
            num: lock(NUM_LOCK),
            scroll: lock(SCROLL_LOCK),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputEvent {
    /// The receiver is now driven: its pointer goes to this pixel of its own desktop.
    Enter { x: i32, y: i32 },
    /// The receiver is no longer driven. It releases every key and button the stream
    /// left pressed; the end of the stream means the same.
    Leave,
    /// The pointer, in pixels of the receiver's desktop. Absolute: the sender owns the
    /// position, as the evdev backend already does locally.
    Motion { x: i32, y: i32 },
    /// A mouse button, as a Linux `BTN_*` code.
    Button { code: u16, pressed: bool },
    /// Wheel travel in 1/120ths of a notch, positive away from the user and to the
    /// right.
    Wheel { horizontal: i32, vertical: i32 },
    /// A key, as a Linux `KEY_*` code.
    Key { code: u16, pressed: bool },
    /// What the sender's lock keys are: the receiver toggles its own to match, those
    /// both know. Sent after an `Enter`, and only to a receiver that reads it
    /// ([`LOCKS_CAPABILITY`]).
    Locks(Locks),
}

const ENTER: u8 = 1;
const LEAVE: u8 = 2;
const MOTION: u8 = 3;
const BUTTON: u8 = 4;
const WHEEL: u8 = 5;
const KEY: u8 = 6;
const LOCKS: u8 = 7;

/// The bytes after an event's tag; `None` for a tag that is not one.
fn payload_length(tag: u8) -> Option<usize> {
    Some(match tag {
        ENTER | MOTION | WHEEL => 8,
        LEAVE => 0,
        BUTTON | KEY => 3,
        LOCKS => 2,
        _ => return None,
    })
}

impl InputEvent {
    /// Appends this event to a batch.
    pub fn encode(&self, batch: &mut Vec<u8>) {
        let two = |batch: &mut Vec<u8>, tag: u8, a: i32, b: i32| {
            batch.push(tag);
            batch.extend_from_slice(&a.to_le_bytes());
            batch.extend_from_slice(&b.to_le_bytes());
        };
        let switch = |batch: &mut Vec<u8>, tag: u8, code: u16, pressed: bool| {
            batch.push(tag);
            batch.extend_from_slice(&code.to_le_bytes());
            batch.push(pressed as u8);
        };
        match *self {
            InputEvent::Enter { x, y } => two(batch, ENTER, x, y),
            InputEvent::Leave => batch.push(LEAVE),
            InputEvent::Motion { x, y } => two(batch, MOTION, x, y),
            InputEvent::Button { code, pressed } => switch(batch, BUTTON, code, pressed),
            InputEvent::Wheel {
                horizontal,
                vertical,
            } => two(batch, WHEEL, horizontal, vertical),
            InputEvent::Key { code, pressed } => switch(batch, KEY, code, pressed),
            InputEvent::Locks(locks) => {
                batch.push(LOCKS);
                batch.extend_from_slice(&locks.bits());
            }
        }
    }
}

/// One batch of events as the bytes of a frame's payload.
pub fn encode(events: &[InputEvent]) -> Vec<u8> {
    let mut batch = Vec::with_capacity(events.len() * 9);
    for event in events {
        event.encode(&mut batch);
    }
    batch
}

/// The events of a batch. An unknown tag or a cut-off event fails the whole batch:
/// there is no telling where the next event starts, and half a report is not input
/// anyone typed.
pub fn decode(mut batch: &[u8]) -> io::Result<Vec<InputEvent>> {
    fn take<'a>(batch: &mut &'a [u8], count: usize) -> io::Result<&'a [u8]> {
        if batch.len() < count {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "input batch ends inside an event",
            ));
        }
        let (head, tail) = batch.split_at(count);
        *batch = tail;
        Ok(head)
    }
    fn two(batch: &mut &[u8]) -> io::Result<(i32, i32)> {
        let bytes = take(batch, 8)?;
        Ok((
            i32::from_le_bytes(bytes[..4].try_into().unwrap()),
            i32::from_le_bytes(bytes[4..].try_into().unwrap()),
        ))
    }
    fn switch(batch: &mut &[u8]) -> io::Result<(u16, bool)> {
        let bytes = take(batch, 3)?;
        Ok((u16::from_le_bytes([bytes[0], bytes[1]]), bytes[2] != 0))
    }

    let mut events = Vec::new();
    while let Some((&tag, rest)) = batch.split_first() {
        batch = rest;
        events.push(match tag {
            ENTER => {
                let (x, y) = two(&mut batch)?;
                InputEvent::Enter { x, y }
            }
            LEAVE => InputEvent::Leave,
            MOTION => {
                let (x, y) = two(&mut batch)?;
                InputEvent::Motion { x, y }
            }
            BUTTON => {
                let (code, pressed) = switch(&mut batch)?;
                InputEvent::Button { code, pressed }
            }
            WHEEL => {
                let (horizontal, vertical) = two(&mut batch)?;
                InputEvent::Wheel {
                    horizontal,
                    vertical,
                }
            }
            KEY => {
                let (code, pressed) = switch(&mut batch)?;
                InputEvent::Key { code, pressed }
            }
            LOCKS => {
                let bytes = take(&mut batch, 2)?;
                InputEvent::Locks(Locks::from_bits([bytes[0], bytes[1]]))
            }
            other => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("unknown input event tag {other}"),
                ))
            }
        });
    }
    Ok(events)
}

/// Whether a batch starts or ends a crossing, read without decoding it: `Some(true)`
/// when its last `Enter` or `Leave` is an `Enter`, `Some(false)` when it is a `Leave`,
/// `None` when it holds neither — or is not a batch at all, which says nothing either.
/// What a master's agent, which relays batches unread, learns of which machine the
/// stream drives; a walk over the tags, allocating nothing.
pub fn crossing(batch: &[u8]) -> Option<bool> {
    let mut seen = None;
    let mut rest = batch;
    while let Some((&tag, tail)) = rest.split_first() {
        match tag {
            ENTER => seen = Some(true),
            LEAVE => seen = Some(false),
            _ => {}
        }
        rest = tail.get(payload_length(tag)?..)?;
    }
    seen
}

/// The batch without its `Locks` events, for a receiver that would fail it on them
/// ([`LOCKS_CAPABILITY`]): the same bytes, borrowed, when there are none — the case of
/// every batch but a crossing's. A batch that does not walk is left as it is: what to
/// do about it is the receiver's business.
pub fn without_locks(batch: &[u8]) -> Cow<'_, [u8]> {
    let mut rest = batch;
    let mut cut: Option<Vec<u8>> = None;
    while let Some(&tag) = rest.first() {
        let Some(event) = payload_length(tag).and_then(|length| rest.get(..1 + length)) else {
            return Cow::Borrowed(batch);
        };
        if tag == LOCKS {
            cut.get_or_insert_with(|| batch[..batch.len() - rest.len()].to_vec());
        } else if let Some(cut) = &mut cut {
            cut.extend_from_slice(event);
        }
        rest = &rest[event.len()..];
    }
    match cut {
        Some(cut) => Cow::Owned(cut),
        None => Cow::Borrowed(batch),
    }
}

/// Reads one batch's bytes, still encoded: an agent relays them as they are.
pub async fn read_batch<R: AsyncRead + Unpin>(reader: &mut R) -> io::Result<Vec<u8>> {
    let length = reader.read_u32_le().await? as usize;
    if length > MAX_BATCH_SIZE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "input batch exceeds 64 KiB",
        ));
    }
    let mut bytes = vec![0u8; length];
    reader.read_exact(&mut bytes).await?;
    Ok(bytes)
}

/// Writes one batch, prefix and payload in a single write.
pub async fn write_batch<W: AsyncWrite + Unpin>(writer: &mut W, batch: &[u8]) -> io::Result<()> {
    if batch.len() > MAX_BATCH_SIZE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "input batch exceeds 64 KiB",
        ));
    }
    let mut frame = Vec::with_capacity(4 + batch.len());
    frame.extend_from_slice(&(batch.len() as u32).to_le_bytes());
    frame.extend_from_slice(batch);
    writer.write_all(&frame).await?;
    writer.flush().await
}

#[cfg(test)]
mod tests {
    use super::*;

    const LOCKED: Locks = Locks {
        caps: Some(true),
        num: Some(false),
        scroll: None,
    };

    const EVERY: [InputEvent; 7] = [
        InputEvent::Enter { x: -1920, y: 40 },
        InputEvent::Motion { x: 5759, y: -2160 },
        InputEvent::Button {
            code: 0x110,
            pressed: true,
        },
        InputEvent::Wheel {
            horizontal: -15,
            vertical: 120,
        },
        InputEvent::Key {
            code: 30,
            pressed: false,
        },
        InputEvent::Locks(LOCKED),
        InputEvent::Leave,
    ];

    #[test]
    fn a_batch_decodes_to_what_was_encoded() {
        assert_eq!(decode(&encode(&EVERY)).unwrap(), EVERY);
        assert_eq!(decode(&[]).unwrap(), []);
    }

    #[test]
    fn a_cut_off_or_unknown_event_fails_the_batch() {
        let batch = encode(&EVERY);
        // Cut inside the last multi-byte event (the trailing Leave is one byte).
        assert!(decode(&batch[..batch.len() - 2]).is_err());
        assert!(decode(&[0]).is_err());
        assert!(decode(&[200, 1, 2]).is_err());
    }

    #[test]
    fn a_crossing_is_read_without_decoding() {
        assert_eq!(crossing(&encode(&EVERY)), Some(false), "the last one wins");
        assert_eq!(crossing(&encode(&EVERY[..6])), Some(true));
        assert_eq!(crossing(&encode(&EVERY[1..6])), None);
        assert_eq!(crossing(&[]), None);
        // An Enter whose coordinates are the tag of a Leave is still an Enter.
        let enter = encode(&[InputEvent::Enter { x: 2, y: 2 }]);
        assert_eq!(crossing(&enter), Some(true));
        // Not a batch: nothing is said, not even the Enter before the damage.
        let mut damaged = enter.clone();
        damaged.push(200);
        assert_eq!(crossing(&damaged), None);
        assert_eq!(crossing(&enter[..5]), None);
    }

    #[test]
    fn every_lock_state_survives_the_wire() {
        let states = [None, Some(false), Some(true)];
        for caps in states {
            for num in states {
                for scroll in states {
                    let locks = InputEvent::Locks(Locks { caps, num, scroll });
                    assert_eq!(decode(&encode(&[locks])).unwrap(), [locks]);
                }
            }
        }
    }

    #[test]
    fn the_locks_are_cut_out_for_a_receiver_that_does_not_read_them() {
        let enter = InputEvent::Enter { x: 7, y: 7 };
        let crossing = encode(&[
            enter,
            InputEvent::Locks(LOCKED),
            InputEvent::Motion { x: 7, y: 8 },
        ]);
        assert_eq!(
            decode(&without_locks(&crossing)).unwrap(),
            [enter, InputEvent::Motion { x: 7, y: 8 }]
        );
        let alone = encode(&[InputEvent::Locks(LOCKED)]);
        assert_eq!(&*without_locks(&alone), &[] as &[u8]);
        // Without locks, the very same bytes, not a copy.
        let plain = encode(&EVERY[..5]);
        assert!(matches!(without_locks(&plain), Cow::Borrowed(_)));
        // Not a batch: left for the receiver to refuse.
        assert!(matches!(without_locks(&[LOCKS, 1]), Cow::Borrowed(_)));
        assert!(matches!(without_locks(&[200]), Cow::Borrowed(_)));
    }

    #[tokio::test]
    async fn a_batch_crosses_a_stream_as_one_frame() {
        let (mut near, mut far) = tokio::io::duplex(4096);
        let batch = encode(&EVERY);
        write_batch(&mut near, &batch).await.unwrap();
        write_batch(&mut near, &[]).await.unwrap();
        assert_eq!(read_batch(&mut far).await.unwrap(), batch);
        assert_eq!(read_batch(&mut far).await.unwrap(), Vec::<u8>::new());
    }

    #[tokio::test]
    async fn an_oversized_batch_is_rejected_before_allocation() {
        let prefix = (MAX_BATCH_SIZE as u32 + 1).to_le_bytes();
        let error = read_batch(&mut prefix.as_slice()).await.unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn the_input_endpoint_follows_the_command_endpoint() {
        assert_eq!(
            input_endpoint_beside("/run/user/1000/littlebigmouse-v1.sock"),
            "/run/user/1000/littlebigmouse-input-v1.sock"
        );
        assert_eq!(
            input_endpoint_beside(r"\\.\pipe\LittleBigMouse-v1-session-2"),
            r"\\.\pipe\LittleBigMouse-Input-v1-session-2"
        );
        assert_eq!(
            input_endpoint_beside("/tmp/x/hook.sock"),
            "/tmp/x/hook.sock-input"
        );
    }
}
