//! The frontends' way in (decision D6 of the v6 plan: JSON between the frontend and the
//! agent) — what the C# UI becomes a client of in phase 4, and the egui frontend after
//! it; the tray drives the agent the same way.
//!
//! The transport is the hook's: length-prefixed UTF-8 frames (`lbm_ipc::framing`) over a
//! per-user endpoint (a 0600 Unix socket beside the instance lock). The payloads are
//! JSON, PascalCase like the rest of the C# contracts:
//!
//! ```text
//! -> {"Id": 1, "Method": "Hello", "Client": "LittleBigMouse.Ui"}
//! <- {"Id": 1, "Result": {"Agent": "lbm-agent", "Version": "0.1.0", "Protocol": 1}}
//! -> {"Id": 2, "Method": "Subscribe"}
//! <- {"Id": 2, "Result": {"AgentVersion": …, "Engine": "Stopped", …}}
//! <- {"Event": "State", "State": {… the snapshot, whenever it changes …}}
//! -> {"Id": 3, "Method": "Start", "KeepLayout": false}
//! <- {"Id": 3, "Result": null}
//! ```
//!
//! Version 1: `Hello`, `Snapshot`, `Subscribe`, `Start`, `Stop`, `Refresh`, `Quit`. The
//! plan's `SaveLayout`, `SaveOptions`, `Preview`/`EndPreview`, `Probe` and
//! `SeenProcesses` come next. An unknown method is answered with an error, never
//! guessed at.

#[cfg(unix)]
use std::io;

#[cfg(unix)]
use lbm_ipc::framing::{read_frame, write_frame};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
#[cfg(unix)]
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

/// Bumped on any change a client must know about.
pub const PROTOCOL: u32 = 1;

//==================//
// The contract     //
//==================//

/// What a frontend asks.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "Method")]
pub enum Request {
    /// Who is there: answered with the agent's name, version and protocol.
    Hello {
        #[serde(rename = "Client", default)]
        client: String,
    },
    /// The agent's state now.
    Snapshot,
    /// The state now, then a `State` event whenever it changes.
    Subscribe,
    /// The user's Start; `KeepLayout` from an editor's "apply and start".
    Start {
        #[serde(rename = "KeepLayout", default)]
        keep_layout: bool,
    },
    /// The user's Stop.
    Stop,
    /// Rebuild the layout the automatic detection missed (#443).
    Refresh,
    /// Leave: the hook, then the agent.
    Quit,
}

/// A request with the id its answer carries back.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RequestFrame {
    #[serde(rename = "Id")]
    pub id: u64,
    #[serde(flatten)]
    pub request: Request,
}

/// What a frontend sees of the agent.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Snapshot {
    pub agent_version: String,
    /// A hook answers at the endpoint.
    pub hook_connected: bool,
    /// `Running`, `Stopped`, `Paused` or `Dead`, as the hook reports it.
    pub engine: String,
    /// The display is off.
    pub suspended: bool,
    /// The current layout's id (the store key of its profile).
    pub layout_id: Option<String>,
    /// Whether the user wants the engine on this layout.
    pub enabled: Option<bool>,
}

/// An answer to request `id`.
pub fn answer(id: u64, result: Result<Value, String>) -> String {
    match result {
        Ok(value) => json!({ "Id": id, "Result": value }),
        Err(error) => json!({ "Id": id, "Error": error }),
    }
    .to_string()
}

/// The `State` event.
pub fn state_event(snapshot: &Snapshot) -> String {
    json!({ "Event": "State", "State": snapshot }).to_string()
}

//==================//
// The server       //
//==================//

/// A frontend's connection, to answer it and send it events.
#[derive(Clone, Debug)]
pub struct Client {
    out: mpsc::UnboundedSender<String>,
}

impl Client {
    /// Sends a frame; `false` once the frontend is gone.
    pub fn send(&self, frame: String) -> bool {
        self.out.send(frame).is_ok()
    }
}

/// A request, with who asked.
#[derive(Debug)]
pub struct Call {
    pub id: u64,
    pub request: Request,
    pub client: Client,
}

/// The endpoint, listening until dropped.
pub struct Listener {
    accepting: JoinHandle<()>,
    #[cfg(unix)]
    path: std::path::PathBuf,
}

impl Drop for Listener {
    fn drop(&mut self) {
        self.accepting.abort();
        #[cfg(unix)]
        let _ = std::fs::remove_file(&self.path);
    }
}

/// Listens at `endpoint` (a socket path): the requests come out of the receiver, in the
/// order each frontend sent them. The caller holds the instance lock, so a socket left
/// at that path is a dead agent's and is replaced.
#[cfg(unix)]
pub fn listen(endpoint: &str) -> io::Result<(mpsc::UnboundedReceiver<Call>, Listener)> {
    use std::os::unix::fs::PermissionsExt;

    let _ = std::fs::remove_file(endpoint);
    let listener = tokio::net::UnixListener::bind(endpoint)?;
    std::fs::set_permissions(endpoint, std::fs::Permissions::from_mode(0o600))?;
    let (calls, calls_rx) = mpsc::unbounded_channel();
    let accepting = tokio::spawn(async move {
        while let Ok((stream, _)) = listener.accept().await {
            tokio::spawn(connection(stream, calls.clone()));
        }
    });
    Ok((
        calls_rx,
        Listener {
            accepting,
            path: endpoint.into(),
        },
    ))
}

// Unix only until the Windows endpoint (a per-session pipe with the hook's DACL) exists.
#[cfg(unix)]
async fn connection<S>(stream: S, calls: mpsc::UnboundedSender<Call>)
where
    S: AsyncRead + AsyncWrite + Send + 'static,
{
    let (mut reader, mut writer) = tokio::io::split(stream);
    let (out, mut out_rx) = mpsc::unbounded_channel::<String>();
    let writing = tokio::spawn(async move {
        while let Some(frame) = out_rx.recv().await {
            if write_frame(&mut writer, &frame).await.is_err() {
                break;
            }
        }
    });
    let client = Client { out };

    while let Ok(frame) = read_frame(&mut reader).await {
        match serde_json::from_str::<RequestFrame>(&frame) {
            Ok(RequestFrame { id, request }) => {
                if calls
                    .send(Call {
                        id,
                        request,
                        client: client.clone(),
                    })
                    .is_err()
                {
                    break;
                }
            }
            Err(error) => {
                // The id, if the frame at least has one, so the client can match it.
                let id = serde_json::from_str::<Value>(&frame)
                    .ok()
                    .and_then(|v| v.get("Id").and_then(Value::as_u64))
                    .unwrap_or(0);
                client.send(answer(id, Err(format!("bad request: {error}"))));
            }
        }
    }
    // Subscribers hold clones of this client: stop the writer outright, and their next
    // send fails, which is how the agent learns the frontend left.
    writing.abort();
}
