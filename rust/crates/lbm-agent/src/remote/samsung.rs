use super::{network, store::Saved};
use base64::{engine::general_purpose::STANDARD, Engine};
use futures_util::{SinkExt, StreamExt};
use tokio::sync::{mpsc, oneshot};
use tokio_tungstenite::tungstenite::Message;

pub struct Client {
    pub ip: String,
    tx: mpsc::Sender<(String, oneshot::Sender<Result<(), String>>)>,
    task: tokio::task::JoinHandle<()>,
}
impl Drop for Client {
    fn drop(&mut self) {
        self.task.abort();
    }
}
impl Client {
    pub fn alive(&self) -> bool {
        !self.task.is_finished()
    }
    pub async fn connect(c: &mut Saved) -> Result<Self, String> {
        let tls = network::tls(c, 8002, false).await?;
        Self::channel(c, tls).await
    }
    async fn channel(
        c: &mut Saved,
        tls: impl tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static,
    ) -> Result<Self, String> {
        let mut url = url::Url::parse(&format!(
            "wss://{}:8002/api/v2/channels/samsung.remote.control",
            network::address(&c.ip_address)?
        ))
        .map_err(|e| e.to_string())?;
        url.query_pairs_mut()
            .append_pair("name", &STANDARD.encode("LittleBigMouse"));
        if !c.token.is_empty() {
            url.query_pairs_mut().append_pair("token", &c.token);
        }
        let (mut socket, _) = tokio_tungstenite::client_async(url.as_str(), tls)
            .await
            .map_err(|_| {
                "Cannot open the Samsung remote channel. Check the address and pairing.".to_string()
            })?;
        loop {
            let message = socket
                .next()
                .await
                .ok_or("Samsung closed the pairing channel.")?
                .map_err(|e| e.to_string())?;
            if let Message::Text(text) = message {
                let root: serde_json::Value =
                    serde_json::from_str(&text).map_err(|e| e.to_string())?;
                match root["event"].as_str().unwrap_or("") {
                    "ms.channel.connect" => { if let Some(token) = token(&root) { c.token = token; } break; }
                    "ms.channel.unauthorized" | "ms.channel.timeOut" | "ms.channel.clientDisconnect" => return Err("Samsung rejected the connection. Pair again and accept LittleBigMouse on the screen.".into()),
                    _ => {},
                }
            } else if matches!(message, Message::Close(_)) {
                return Err("Samsung closed the pairing channel.".into());
            }
        }
        let (tx, mut rx) = mpsc::channel::<(String, oneshot::Sender<Result<(), String>>)>(16);
        let task = tokio::spawn(async move {
            loop {
                tokio::select! {
                    command = rx.recv() => {
                        let Some((key, answer)) = command else { break };
                        let payload = payload(&key);
                        let result = socket.send(Message::Text(payload.into())).await.map_err(|e| e.to_string());
                        let failed = result.is_err(); let _ = answer.send(result);
                        if failed { break; }
                    }
                    message = socket.next() => {
                        match message {
                            Some(Ok(Message::Ping(_))) => { if socket.flush().await.is_err() { break; } }
                            Some(Ok(Message::Close(_))) | Some(Err(_)) | None => break,
                            _ => {}
                        }
                    }
                }
            }
        });
        Ok(Self {
            ip: c.ip_address.clone(),
            tx,
            task,
        })
    }
    pub async fn key(&self, key: String) -> Result<(), String> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send((key, tx))
            .await
            .map_err(|_| "Samsung disconnected; try again.")?;
        rx.await
            .map_err(|_| "Samsung disconnected before confirming the command was sent.")?
    }
}
fn token(root: &serde_json::Value) -> Option<String> {
    if let Some(s) = root.get("token").and_then(|v| v.as_str()) {
        return Some(s.into());
    }
    match root {
        serde_json::Value::Object(o) => o.values().find_map(token),
        serde_json::Value::Array(a) => a.iter().find_map(token),
        _ => None,
    }
}
fn payload(key: &str) -> String {
    serde_json::json!({"method":"ms.remote.control", "params":{"Cmd":"Click", "DataOfCmd":key, "Option":"false", "TypeOfRemote":"SendRemoteKey"}}).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn native_websocket_pairs_and_sends_exact_remote_payloads() {
        let (client, server) = tokio::io::duplex(8192);
        let tv = tokio::spawn(async move {
            let mut socket = tokio_tungstenite::accept_async(server).await.unwrap();
            socket.send(Message::Text(r#"{"event":"ms.channel.connect","data":{"clients":[{"attributes":{"token":"test-token"}}]}}"#.into())).await.unwrap();
            let frame = socket.next().await.unwrap().unwrap().into_text().unwrap();
            let value: serde_json::Value = serde_json::from_str(&frame).unwrap();
            assert_eq!(value["method"], "ms.remote.control");
            assert_eq!(value["params"]["Cmd"], "Click");
            assert_eq!(value["params"]["DataOfCmd"], "KEY_HOME");
        });
        let mut config = Saved {
            ip_address: "127.0.0.1".into(),
            ..Default::default()
        };
        let remote = Client::channel(&mut config, client).await.unwrap();
        assert_eq!(config.token, "test-token");
        remote.key("KEY_HOME".into()).await.unwrap();
        tv.await.unwrap();
    }
}
