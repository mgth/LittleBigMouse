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
}

const ENTER: u8 = 1;
const LEAVE: u8 = 2;
const MOTION: u8 = 3;
const BUTTON: u8 = 4;
const WHEEL: u8 = 5;
const KEY: u8 = 6;

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

    const EVERY: [InputEvent; 6] = [
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
