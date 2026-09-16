//! The frontends' way in (decision D6 of the v6 plan: JSON between the frontend and the
//! agent) — what the C# UI becomes a client of in phase 4, and the egui frontend after
//! it; the tray drives the agent the same way.
//!
//! The transport is the hook's: length-prefixed UTF-8 frames (`lbm_ipc::framing`) over a
//! per-user endpoint — a 0600 Unix socket beside the instance lock, or on Windows a pipe
//! per logon session secured as the hook's (`winpipe`). The payloads are
//! JSON, PascalCase like the rest of the C# contracts:
//!
//! ```text
//! -> {"Id": 1, "Method": "Hello", "Client": "LittleBigMouse.Ui"}
//! <- {"Id": 1, "Result": {"Agent": "lbm-agent", "Version": "0.1.0", "Protocol": 1}}
//! -> {"Id": 2, "Method": "Subscribe"}
//! <- {"Id": 2, "Result": {"AgentVersion": …, "Engine": "Stopped", …}}
//! <- {"Event": "State", "State": {… the snapshot, whenever it changes …}}
//! <- {"Event": "Hook", "Hook": "Loaded", "Payload": "2 zones (2 main)"}
//! -> {"Id": 3, "Method": "Start", "KeepLayout": false}
//! <- {"Id": 3, "Result": null}
//! ```
//!
//! A subscriber also gets every hook event as the hook said it, under the names of C#'s
//! `LittleBigMouseEvent` (`Connected` and `Dead` when the connection comes and goes):
//! the frontends' trackers — the load outcome, the probe report, the rescue — read
//! them as they read the hook before.
//!
//! Version 1: `Hello`, `Snapshot`, `Subscribe`, `Start`, `Stop`, `Refresh`, `Quit`.
//! Version 2: the hook events, `Probe` (the report comes as a `Probed` event) and
//! `SeenProcesses` (the processes seen in the foreground this session).
//! Version 3: the agent becomes the only writer. A frontend sends what it would have
//! saved — a [`LayoutDocument`], the store's own documents in one object — for the
//! layout it edits (`LayoutId`, refused if the displays changed under it):
//!
//! ```text
//! -> {"Id": 4, "Method": "SaveLayout", "LayoutId": "…", "Document": {…}}
//! -> {"Id": 5, "Method": "Start", "LayoutId": "…", "Document": {…}}    // apply and start
//! -> {"Id": 6, "Method": "Preview", "LayoutId": "…", "Document": {…}}  // every tick
//! -> {"Id": 7, "Method": "EndPreview"}
//! -> {"Id": 8, "Method": "SaveOptions", "Options": {…}, "Excluded": ["…"]}
//! ```
//!
//! A preview is ended by the agent too (the user's Start or Stop, a rebuild, the
//! rescue, the hook going away): `Previewing` in the state says which is the case.
//! An unknown method is answered with an error, never guessed at.

use std::io;

use lbm_ipc::client::DaemonEvent;
use lbm_ipc::framing::{read_frame, write_frame};
use lbm_store::{GlobalOptionsDto, LayoutDocument};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

/// Bumped on any change a client must know about.
pub const PROTOCOL: u32 = 3;

//==================//
// The contract     //
//==================//

/// What a frontend asks.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
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
    /// The user's Start; `KeepLayout` from an editor's "apply and start", which sends
    /// its edit along (`LayoutId` and `Document`): applied, saved, started.
    Start {
        #[serde(rename = "KeepLayout", default)]
        keep_layout: bool,
        #[serde(rename = "LayoutId", default, skip_serializing_if = "Option::is_none")]
        layout_id: Option<String>,
        #[serde(rename = "Document", default, skip_serializing_if = "Option::is_none")]
        document: Option<Box<LayoutDocument>>,
    },
    /// Apply an edit to the current layout and save it (C#: the Save button).
    SaveLayout {
        #[serde(rename = "LayoutId")]
        layout_id: String,
        #[serde(rename = "Document")]
        document: Box<LayoutDocument>,
    },
    /// A live-preview tick: the hook runs the edit, nothing is saved.
    Preview {
        #[serde(rename = "LayoutId")]
        layout_id: String,
        #[serde(rename = "Document")]
        document: Box<LayoutDocument>,
    },
    /// The preview is over: the hook goes back to the current layout.
    EndPreview,
    /// The app-level options and the excluded list, saved at once (C#: `SaveLive`).
    /// `LoadAtStartup` is not one of the stored options: it *is* the session autostart
    /// (the XDG entry, the scheduled task), which the agent aligns.
    SaveOptions {
        #[serde(rename = "Options", default, skip_serializing_if = "Option::is_none")]
        options: Option<GlobalOptionsDto>,
        #[serde(rename = "Excluded", default, skip_serializing_if = "Option::is_none")]
        excluded: Option<Vec<String>>,
        #[serde(
            rename = "LoadAtStartup",
            default,
            skip_serializing_if = "Option::is_none"
        )]
        load_at_startup: Option<bool>,
    },
    /// The user's Stop.
    Stop,
    /// Rebuild the layout the automatic detection missed (#443).
    Refresh,
    /// Leave: the hook, then the agent.
    Quit,
    /// Ask the hook for its edge report on the loaded layout (a `Probed` event).
    Probe,
    /// The processes seen in the foreground this session, oldest first — what the
    /// exclusion list is picked from.
    SeenProcesses,
}

/// A request with the id its answer carries back.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
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
    /// Nothing edited since the layout was last saved.
    pub saved: Option<bool>,
    /// The session starts the agent (the autostart entry, the scheduled task).
    pub load_at_startup: Option<bool>,
    /// The user asked for no tray icon.
    pub hide_tray_icon: Option<bool>,
    /// A frontend's live preview is what the hook runs.
    pub previewing: bool,
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

/// The `Hook` event: what the hook said, forwarded.
pub fn hook_event(name: &str, payload: &str) -> String {
    json!({ "Event": "Hook", "Hook": name, "Payload": payload }).to_string()
}

/// A hook event's name on this API: C#'s `LittleBigMouseEvent`.
pub fn hook_event_name(event: DaemonEvent) -> &'static str {
    match event {
        DaemonEvent::Running => "Running",
        DaemonEvent::Stopped => "Stopped",
        DaemonEvent::Paused => "Paused",
        DaemonEvent::Dead => "Dead",
        DaemonEvent::SettingsChanged => "SettingsChanged",
        DaemonEvent::DisplayChanged => "DisplayChanged",
        DaemonEvent::DesktopChanged => "DesktopChanged",
        DaemonEvent::FocusChanged => "FocusChanged",
        DaemonEvent::Suspended => "Suspended",
        DaemonEvent::Resumed => "Resumed",
        DaemonEvent::Loaded => "Loaded",
        DaemonEvent::LoadFailed => "LoadFailed",
        DaemonEvent::Probed => "Probed",
        DaemonEvent::Rescued => "Rescued",
        DaemonEvent::ShortcutUnavailable => "ShortcutUnavailable",
        DaemonEvent::RunRefused => "RunRefused",
    }
}

/// C# `ProcessesCollector`: the processes seen in the foreground, in order, each once.
#[derive(Debug, Default)]
pub struct SeenProcesses(Vec<String>);

impl SeenProcesses {
    /// C# `AddProcess`: nothing for an empty name, nor for one a seen entry already
    /// contains. Returns whether it was added.
    pub fn add(&mut self, process: &str) -> bool {
        if process.is_empty() || self.0.iter().any(|seen| seen.contains(process)) {
            return false;
        }
        self.0.push(process.to_owned());
        true
    }

    pub fn list(&self) -> &[String] {
        &self.0
    }
}

//==================//
// The server       //
//==================//

/// A frontend's connection, to answer it and send it events.
#[derive(Clone, Debug)]
pub struct Client {
    out: mpsc::UnboundedSender<String>,
}

/// A frontend in the agent's own process (the tray): a client, and the frames sent to it.
pub fn in_process() -> (Client, mpsc::UnboundedReceiver<String>) {
    let (out, frames) = mpsc::unbounded_channel();
    (Client { out }, frames)
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
    let (calls, calls_rx) = mpsc::unbounded_channel();
    Ok((calls_rx, listen_into(endpoint, calls)?))
}

/// [`listen`], the requests going into `calls` — which the in-process frontends (the
/// tray) send into too.
#[cfg(unix)]
pub fn listen_into(endpoint: &str, calls: mpsc::UnboundedSender<Call>) -> io::Result<Listener> {
    use std::os::unix::fs::PermissionsExt;

    let _ = std::fs::remove_file(endpoint);
    let listener = tokio::net::UnixListener::bind(endpoint)?;
    std::fs::set_permissions(endpoint, std::fs::Permissions::from_mode(0o600))?;
    let accepting = tokio::spawn(async move {
        while let Ok((stream, _)) = listener.accept().await {
            tokio::spawn(connection(stream, calls.clone()));
        }
    });
    Ok(Listener {
        accepting,
        path: endpoint.into(),
    })
}

/// Listens on the agent's pipe `name` (`winpipe::agent_pipe` for this session), as
/// [`listen`] does on a socket. The first instance claims the name: the caller holds
/// the instance lock, so a refusal means someone else holds it — an error, not a retry.
#[cfg(windows)]
pub fn listen_pipe(name: &str) -> io::Result<(mpsc::UnboundedReceiver<Call>, Listener)> {
    let (calls, calls_rx) = mpsc::unbounded_channel();
    Ok((calls_rx, listen_pipe_into(name, calls)?))
}

/// [`listen_pipe`], the requests going into `calls` — which the in-process frontends (the
/// tray) send into too.
#[cfg(windows)]
pub fn listen_pipe_into(name: &str, calls: mpsc::UnboundedSender<Call>) -> io::Result<Listener> {
    use crate::winpipe;

    let mut pipe = winpipe::create_pipe(name, true)?;
    let name = name.to_owned();
    let accepting = tokio::spawn(async move {
        loop {
            if pipe.connect().await.is_err() {
                return;
            }
            // The next instance before this one is handed over: a frontend connecting
            // meanwhile finds the pipe waiting.
            let next = loop {
                match winpipe::create_pipe(&name, false) {
                    Ok(next) => break next,
                    Err(error) => {
                        eprintln!("[lbm-agent] frontend pipe: {error}; retrying");
                        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                    }
                }
            };
            let connected = std::mem::replace(&mut pipe, next);
            if winpipe::client_is_current_session(&connected) {
                tokio::spawn(connection(connected, calls.clone()));
            }
        }
    });
    Ok(Listener { accepting })
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_process_is_seen_once_and_never_empty() {
        let mut seen = SeenProcesses::default();
        assert!(!seen.add(""));
        assert!(seen.add("/usr/bin/firefox"));
        assert!(seen.add("/usr/bin/kate"));
        assert!(!seen.add("/usr/bin/firefox"));
        // C#: `Contains`, so a name a seen entry holds is taken as seen.
        assert!(!seen.add("firefox"));
        assert_eq!(seen.list(), ["/usr/bin/firefox", "/usr/bin/kate"]);
    }

    #[test]
    fn a_hook_event_keeps_its_csharp_name_and_payload() {
        let event: Value = serde_json::from_str(&hook_event(
            hook_event_name(DaemonEvent::Probed),
            "<ProbeReport />",
        ))
        .unwrap();
        assert_eq!(
            event,
            json!({ "Event": "Hook", "Hook": "Probed", "Payload": "<ProbeReport />" })
        );
    }
}
