//! Version-1 local IPC framing: a four-byte little-endian UTF-8 payload length.

use std::io;

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

    #[tokio::test]
    async fn oversized_frame_is_rejected_before_allocation() {
        let prefix = (MAX_FRAME_SIZE as u32 + 1).to_le_bytes();
        let mut bytes = prefix.as_slice();
        let error = read_frame(&mut bytes).await.unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    }
}
