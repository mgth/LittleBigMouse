//! Version-1 local IPC framing: a four-byte little-endian UTF-8 payload length.
//!
//! In two shapes, because the two sides of the product are not built the same way. The
//! agent and the hook are async and read frames on a tokio runtime; a frontend drawing
//! at sixty frames a second has no runtime and no reason to grow one for a socket it
//! reads on a thread of its own. Both shapes are here rather than one of them being
//! rewritten somewhere else — the frame is one rule, and a second copy of it is how two
//! programs start to disagree about what a message is.

use std::io;
use std::io::{Read, Write};

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

pub const MAX_FRAME_SIZE: usize = 1024 * 1024;

pub async fn read_frame<R: AsyncRead + Unpin>(reader: &mut R) -> io::Result<String> {
    let length = reader.read_u32_le().await? as usize;
    if length > MAX_FRAME_SIZE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "local IPC frame exceeds 1 MiB",
        ));
    }
    let mut bytes = vec![0u8; length];
    reader.read_exact(&mut bytes).await?;
    String::from_utf8(bytes)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "local IPC frame is not UTF-8"))
}

pub async fn write_frame<W: AsyncWrite + Unpin>(writer: &mut W, message: &str) -> io::Result<()> {
    let bytes = message.as_bytes();
    if bytes.len() > MAX_FRAME_SIZE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "local IPC frame exceeds 1 MiB",
        ));
    }
    writer.write_u32_le(bytes.len() as u32).await?;
    writer.write_all(bytes).await?;
    writer.flush().await
}

/// [`read_frame`] for a caller with no runtime.
pub fn read_frame_blocking<R: Read>(reader: &mut R) -> io::Result<String> {
    let mut prefix = [0u8; 4];
    reader.read_exact(&mut prefix)?;
    let length = u32::from_le_bytes(prefix) as usize;
    if length > MAX_FRAME_SIZE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "local IPC frame exceeds 1 MiB",
        ));
    }
    let mut bytes = vec![0u8; length];
    reader.read_exact(&mut bytes)?;
    String::from_utf8(bytes)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "local IPC frame is not UTF-8"))
}

/// [`write_frame`] for a caller with no runtime.
///
/// One `write_all` for the whole frame, prefix included. The async twin writes the
/// prefix and the payload separately, which is safe there because one task owns the
/// writer; doing it in two here would leave a reader stranded on a length whose payload
/// never came if the caller ever wrote from two threads. The C# side learned that one
/// the hard way (#676).
pub fn write_frame_blocking<W: Write>(writer: &mut W, message: &str) -> io::Result<()> {
    let bytes = message.as_bytes();
    if bytes.len() > MAX_FRAME_SIZE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "local IPC frame exceeds 1 MiB",
        ));
    }
    let mut frame = Vec::with_capacity(4 + bytes.len());
    frame.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
    frame.extend_from_slice(bytes);
    writer.write_all(&frame)?;
    writer.flush()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn frame_round_trip_preserves_utf8() {
        let (mut client, mut server) = tokio::io::duplex(4096);
        let message = "<DaemonMessage><Payload>écran</Payload></DaemonMessage>";
        let send = write_frame(&mut client, message);
        let receive = read_frame(&mut server);
        let (sent, received) = tokio::join!(send, receive);
        sent.unwrap();
        assert_eq!(received.unwrap(), message);
    }

    /// The two shapes are one frame. A blocking writer's bytes have to be what the
    /// async reader expects, and the other way round — otherwise the frontend and the
    /// agent would each be right on their own and unable to talk.
    #[tokio::test]
    async fn what_one_shape_writes_the_other_reads() {
        let message = "{\"Method\":\"Hello\",\"Client\":\"écran\"}";

        // Blocking out, async in.
        let mut bytes = Vec::new();
        write_frame_blocking(&mut bytes, message).unwrap();
        assert_eq!(read_frame(&mut bytes.as_slice()).await.unwrap(), message);

        // Async out, blocking in.
        let mut other = Vec::new();
        write_frame(&mut other, message).await.unwrap();
        assert_eq!(read_frame_blocking(&mut other.as_slice()).unwrap(), message);

        // And the same bytes, not merely the same text.
        assert_eq!(bytes, other);
    }

    #[test]
    fn a_blocking_frame_is_written_in_one_go() {
        // A reader that gives up if it is asked to wait between the length and the
        // payload — what a second writer slipping in would cause.
        struct OneWrite(usize);
        impl std::io::Write for OneWrite {
            fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
                self.0 += 1;
                assert_eq!(self.0, 1, "the frame left in more than one write");
                Ok(buf.len())
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        write_frame_blocking(&mut OneWrite(0), "{}").unwrap();
    }

    #[test]
    fn an_oversized_blocking_frame_is_rejected_before_allocation() {
        let prefix = (MAX_FRAME_SIZE as u32 + 1).to_le_bytes();
        let error = read_frame_blocking(&mut prefix.as_slice()).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    }

    #[tokio::test]
    async fn oversized_frame_is_rejected_before_allocation() {
        let prefix = (MAX_FRAME_SIZE as u32 + 1).to_le_bytes();
        let mut bytes = prefix.as_slice();
        let error = read_frame(&mut bytes).await.unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    }
}
