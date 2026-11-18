//! Minimal MQTT 3.1.1 transport used by the VIDAA reference implementation.
use super::{network, store::Saved};
use std::{sync::Arc, time::Duration};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWriteExt},
    sync::{broadcast, Mutex},
};

trait Io: AsyncRead + tokio::io::AsyncWrite + Unpin + Send {}
impl<T: AsyncRead + tokio::io::AsyncWrite + Unpin + Send> Io for T {}
type Stream = Box<dyn Io>;
type Writer = Arc<Mutex<tokio::io::WriteHalf<Stream>>>;
#[derive(Clone, Debug)]
pub struct Published {
    pub topic: String,
    pub payload: String,
    pub retained: bool,
}
pub struct Client {
    writer: Writer,
    events: broadcast::Sender<Result<Published, String>>,
    reader: tokio::task::JoinHandle<()>,
    heartbeat: tokio::task::JoinHandle<()>,
}
impl Drop for Client {
    fn drop(&mut self) {
        self.reader.abort();
        self.heartbeat.abort();
    }
}
impl Client {
    pub fn alive(&self) -> bool {
        !self.reader.is_finished() && !self.heartbeat.is_finished()
    }
    pub fn subscribe_events(&self) -> broadcast::Receiver<Result<Published, String>> {
        self.events.subscribe()
    }
    pub async fn connect(
        c: &Saved,
        id: &str,
        user: &str,
        pass: &str,
        topics: &[String],
    ) -> Result<Self, String> {
        let stream = network::tls(c, 36669, true).await?;
        Self::open(Box::new(stream), id, user, pass, topics).await
    }
    async fn open(
        mut stream: Stream,
        id: &str,
        user: &str,
        pass: &str,
        topics: &[String],
    ) -> Result<Self, String> {
        let mut body = vec![];
        string(&mut body, "MQTT")?;
        body.extend([4, 0xc2, 0, 30]);
        string(&mut body, id)?;
        string(&mut body, user)?;
        string(&mut body, pass)?;
        stream
            .write_all(&packet(0x10, &body)?)
            .await
            .map_err(|e| e.to_string())?;
        let (header, body) = read_packet(&mut stream).await?;
        if header != 0x20 || body.len() != 2 {
            return Err("Invalid VIDAA MQTT response.".into());
        }
        if body[1] != 0 {
            return Err(format!(
                "VIDAA rejected the MQTT credentials (code {}).",
                body[1]
            ));
        }
        // Wait for each SUBACK before publishing pairing commands, so an immediate
        // response cannot precede our subscription.
        for (i, topic) in topics.iter().enumerate() {
            let id = (i + 1) as u16;
            let mut b = id.to_be_bytes().to_vec();
            string(&mut b, topic)?;
            b.push(0);
            stream
                .write_all(&packet(0x82, &b)?)
                .await
                .map_err(|e| e.to_string())?;
            loop {
                let (h, b) = read_packet(&mut stream).await?;
                if h == 0x90 && b.len() >= 3 && b[..2] == id.to_be_bytes() {
                    if b[2] == 0x80 {
                        return Err("VIDAA refused the response subscription.".into());
                    }
                    break;
                }
            }
        }
        let (mut read, write) = tokio::io::split(stream);
        let writer = Arc::new(Mutex::new(write));
        let (events, _) = broadcast::channel(128);
        let sink = events.clone();
        let pong_writer = writer.clone();
        let reader = tokio::spawn(async move {
            loop {
                match read_packet(&mut read).await {
                    Ok((h, b)) if h >> 4 == 3 => {
                        if let Some((published, ack)) = parse_publish(h, &b) {
                            if let Some(id) = ack {
                                let _ = pong_writer
                                    .lock()
                                    .await
                                    .write_all(&[0x40, 2, id[0], id[1]])
                                    .await;
                            }
                            let _ = sink.send(Ok(published));
                        }
                    }
                    Ok(_) => {}
                    Err(e) => {
                        let _ = sink.send(Err(e));
                        break;
                    }
                }
            }
        });
        let heartbeat_writer = writer.clone();
        let heartbeat = tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(10)).await;
                if heartbeat_writer
                    .lock()
                    .await
                    .write_all(&[0xc0, 0])
                    .await
                    .is_err()
                {
                    break;
                }
            }
        });
        Ok(Self {
            writer,
            events,
            reader,
            heartbeat,
        })
    }
    pub async fn publish(&self, topic: &str, payload: &str) -> Result<(), String> {
        if !self.alive() {
            return Err("VIDAA disconnected; retry the command.".into());
        }
        let mut b = vec![];
        string(&mut b, topic)?;
        b.extend(payload.as_bytes());
        self.writer
            .lock()
            .await
            .write_all(&packet(0x30, &b)?)
            .await
            .map_err(|e| e.to_string())
    }
}
fn string(b: &mut Vec<u8>, s: &str) -> Result<(), String> {
    let n = u16::try_from(s.len()).map_err(|_| "MQTT field is too long")?;
    b.extend(n.to_be_bytes());
    b.extend(s.as_bytes());
    Ok(())
}
fn packet(header: u8, body: &[u8]) -> Result<Vec<u8>, String> {
    if body.len() > 1024 * 1024 {
        return Err("MQTT packet is too large.".into());
    }
    let mut b = vec![header];
    let mut n = body.len();
    loop {
        let digit = (n % 128) as u8;
        n /= 128;
        b.push(digit | if n > 0 { 128 } else { 0 });
        if n == 0 {
            break;
        }
    }
    b.extend(body);
    Ok(b)
}
async fn read_packet(r: &mut (impl AsyncRead + Unpin)) -> Result<(u8, Vec<u8>), String> {
    let header = r.read_u8().await.map_err(|e| e.to_string())?;
    let mut n = 0usize;
    for i in 0..4 {
        let b = r.read_u8().await.map_err(|e| e.to_string())?;
        n += usize::from(b & 127) << (7 * i);
        if n > 1024 * 1024 {
            return Err("MQTT response is too large.".into());
        }
        if b & 128 == 0 {
            let mut body = vec![0; n];
            r.read_exact(&mut body).await.map_err(|e| e.to_string())?;
            return Ok((header, body));
        }
    }
    Err("Invalid MQTT packet length.".into())
}
fn parse_publish(h: u8, b: &[u8]) -> Option<(Published, Option<[u8; 2]>)> {
    let n = usize::from(u16::from_be_bytes(b.get(..2)?.try_into().ok()?));
    let topic = std::str::from_utf8(b.get(2..2 + n)?).ok()?.to_string();
    let qos = (h >> 1) & 3;
    if qos > 1 {
        return None;
    }
    let mut offset = 2 + n;
    let ack = if qos == 1 {
        let id = b.get(offset..offset + 2)?.try_into().ok()?;
        offset += 2;
        Some(id)
    } else {
        None
    };
    Some((
        Published {
            topic,
            payload: std::str::from_utf8(b.get(offset..)?).ok()?.into(),
            retained: h & 1 != 0,
        },
        ack,
    ))
}
pub async fn answer<T>(
    rx: &mut broadcast::Receiver<Result<Published, String>>,
    mut parse: impl FnMut(&Published) -> Option<Result<T, String>>,
) -> Result<T, String> {
    tokio::time::timeout(Duration::from_secs(12), async {
        loop {
            let event = rx
                .recv()
                .await
                .map_err(|_| "VIDAA response stream ended.".to_string())??;
            if !event.retained {
                if let Some(answer) = parse(&event) {
                    return answer;
                }
            }
        }
    })
    .await
    .map_err(|_| "VIDAA did not acknowledge this request.".to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn native_mqtt_subscribes_then_correlates_a_reply_without_accepting_retained_state() {
        let (client, mut server) = tokio::io::duplex(8192);
        let tv = tokio::spawn(async move {
            let (h, b) = read_packet(&mut server).await.unwrap();
            assert_eq!(h, 0x10);
            assert_eq!(&b[2..6], b"MQTT");
            server.write_all(&[0x20, 2, 0, 0]).await.unwrap();
            let (h, b) = read_packet(&mut server).await.unwrap();
            assert_eq!(h, 0x82);
            server.write_all(&[0x90, 3, b[0], b[1], 0]).await.unwrap();
            let (h, b) = read_packet(&mut server).await.unwrap();
            let (message, _) = parse_publish(h, &b).unwrap();
            assert_eq!(
                message.topic,
                "/remoteapp/tv/platform_service/device/actions/getvolume"
            );
            assert_eq!(message.payload, "0");
            let mut body = vec![];
            string(
                &mut body,
                "/remoteapp/mobile/broadcast/platform_service/actions/volumechange",
            )
            .unwrap();
            body.extend(b"{\"volume_value\":37}");
            server
                .write_all(&packet(0x31, &body).unwrap())
                .await
                .unwrap();
            server
                .write_all(&packet(0x30, &body).unwrap())
                .await
                .unwrap();
            // Keep the transport alive until the test has collected its response.
            let _ = server.read_u8().await;
        });
        let remote = Client::open(
            Box::new(client),
            "test-client",
            "user",
            "password",
            &["/remoteapp/mobile/broadcast/#".into()],
        )
        .await
        .unwrap();
        let mut rx = remote.subscribe_events();
        remote
            .publish(
                "/remoteapp/tv/platform_service/device/actions/getvolume",
                "0",
            )
            .await
            .unwrap();
        let value = answer(&mut rx, |m| {
            Some(Ok(serde_json::from_str::<serde_json::Value>(&m.payload)
                .unwrap()["volume_value"]
                .as_i64()
                .unwrap()))
        })
        .await
        .unwrap();
        assert_eq!(value, 37);
        drop(remote);
        tv.abort();
    }
    #[tokio::test]
    async fn malformed_lengths_are_bounded() {
        let bytes = &[0x30, 0xff, 0xff, 0xff, 0x7f][..];
        assert!(read_packet(&mut &*bytes).await.is_err());
        assert!(parse_publish(0x30, &[0, 50, 1]).is_none());
    }
}
