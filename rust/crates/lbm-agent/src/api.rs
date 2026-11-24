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
//!
//! The multi-PC mode (capability `MultiPc`, additive to version 3; docs/multi-pc.md):
//!
//! ```text
//! -> {"Id": 9, "Method": "SetMultiPc", "Enabled": true, "Port": 24817, "Name": "desk",
//!     "PointerLockShortcut": "Ctrl+Alt+L"}                    // each optional but Enabled
//! <- {"Id": 9, "Result": {"Enabled": true, "Port": 24817, "Name": "desk",
//!                         "PointerLockShortcut": "Ctrl+Alt+L", "PointerLocked": false,
//!                         "MachineId": "<sha-256 hex>", "ListenAddress": "0.0.0.0:24817",
//!                         "Error": null, "Peers": [...]}}
//! -> {"Id": 10, "Method": "PeerConnect", "Address": "192.168.1.30"}
//! <- {"Id": 10, "Result": {"MachineId": "…", "Name": "tower", "Address": "192.168.1.30:24817",
//!                          "Connected": false, "Displays": [], "Role": "Remote",
//!                          "State": "Pending"}}
//! -> {"Id": 11, "Method": "PeerAccept", "MachineId": "…"}     // also PeerRefuse, PeerForget
//! -> {"Id": 12, "Method": "Peers"}                             // the state, as above
//! <- {"Event": "Peers", "Peers": {… the state, whenever it changes …}}
//! -> {"Id": 13, "Method": "SetActiveSource", "LayoutId": "…", "Monitor": "DEL4065_ABC123",
//!     "Source": "HDMI-1@a1b2c3d4"}                             // a shared panel's machine
//! <- {"Id": 13, "Result": null}
//! -> {"Id": 14, "Method": "PeerWallpapers", "MachineId": "…"}  // capability PeerWallpapers
//! <- {"Id": 14, "Result": {"MachineId": "…", "Revision": 3,     // its WallpaperRevision
//!                          "Wallpapers": [{"Display": "eDP-1", "Image": "<base64 JPEG>"},
//!                                         {"Display": "HDMI-1", "Color": "#203040"}]}}
//! -> {"Id": 15, "Method": "PeerOverlay", "MachineId": "…",     // capability PeerOverlay
//!     "Bands": [{… lbm_overlay::scene::Band, in that remote's desktop pixels …}]}
//! <- {"Id": 15, "Result": null}                                // "Bands": [] takes them down
//! ```
//!
//! The rulers on a remote are the frontend's that wrote them last: only its empty
//! `Bands` or the end of its connection takes them down.

use std::io;

use lbm_ipc::framing::{read_frame, write_frame};
use lbm_store::{GlobalOptionsDto, LayoutDocument};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

/// Bumped on any change a client must know about.
pub const PROTOCOL: u32 = 3;

/// What this agent does that an agent of the same [`PROTOCOL`] may not, announced in
/// `Hello` as `Capabilities`. A frontend updated before the agent it talks to (the
/// resident survives an update until restarted) checks for one before relying on it,
/// rather than the protocol being bumped and every older agent refused outright.
///
/// - `IntrinsicSize`: a model size marked `IntrinsicSize` in a document is applied as
///   stored, whatever the detected shape (#728, #730). An agent without it swaps a
///   size whose landscape/portrait shape contradicts the detected panel.
/// - `MultiPc`: the multi-PC requests (`Peers`, `SetMultiPc`, `PeerConnect`,
///   `PeerAccept`, `PeerRefuse`, `PeerForget`, `SetActiveSource`) and the `Peers`
///   event, which also carry the remote machines' screens (docs/multi-pc.md). The
///   layout of a master holds its remotes' screens: its `LayoutId` is that of every
///   monitor, theirs included.
/// - `PeerWallpapers`: the request of that name, and `WallpaperRevision` on every machine
///   of the `Peers` state: what a remote's screens show, for the master's map.
/// - `PeerOverlay`: the request of that name — the rulers the editor shows, drawn on a
///   remote's screens too — and `OverlayError` on a machine of the `Peers` state.
pub const CAPABILITIES: &[&str] = &["IntrinsicSize", "MultiPc", "PeerWallpapers", "PeerOverlay"];

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
    /// Connected outputs, including disabled ones. Unsupported backends return an error.
    DisplayOutputs,
    /// Native Samsung/Hisense remote, independent from DDC/CI.
    Remote {
        #[serde(rename = "LayoutId")]
        layout_id: String,
        #[serde(rename = "LayoutRevision")]
        layout_revision: u64,
        #[serde(rename = "Monitor")]
        monitor: String,
        #[serde(rename = "Command")]
        command: lbm_ipc::remote::Command,
    },
    /// Read a monitor's VCP controls, or write one previously probed value.
    Vcp {
        #[serde(rename = "LayoutId")]
        layout_id: String,
        #[serde(rename = "LayoutRevision")]
        layout_revision: u64,
        #[serde(rename = "Monitor")]
        monitor: String,
        #[serde(rename = "Write", default)]
        write: Option<lbm_ipc::vcp::Write>,
    },
    /// Change one system output after checking the editor's revision and fresh inventory.
    ChangeOutput {
        #[serde(rename = "LayoutId")]
        layout_id: String,
        #[serde(rename = "LayoutRevision")]
        layout_revision: u64,
        #[serde(rename = "Signature")]
        signature: String,
        #[serde(rename = "Connector")]
        connector: String,
        #[serde(rename = "Change")]
        change: lbm_ipc::displays::Change,
    },
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
    /// Apply the layout to the **system**: the screens really move.
    ///
    /// The document is saved first, as the C# does and for the reason it gives: the
    /// system change triggers a rebuild that re-imports the system layout and then loads
    /// the saved one, so the physical layout survives only if it was saved.
    ///
    /// It goes through the agent rather than being done by the frontend — which the plan
    /// otherwise lets do topology — because applying means closing the engine's 1px gaps
    /// first, and the gap guard is the agent's (D7). Two processes writing the topology
    /// would fight over it.
    ApplyTopology {
        #[serde(rename = "LayoutId")]
        layout_id: String,
        /// Required by the Windows placement path. Optional for older Linux clients.
        #[serde(
            rename = "LayoutRevision",
            default,
            skip_serializing_if = "Option::is_none"
        )]
        layout_revision: Option<u64>,
        #[serde(rename = "Document")]
        document: Box<LayoutDocument>,
        /// Ask the compositor for the per-output scales that make every monitor's
        /// logical pitch the primary's. Linux only; the C# offers it in the dialog.
        #[serde(rename = "AdjustScale", default)]
        adjust_scale: bool,
        /// Change nothing: answer with the command lines this would have run.
        ///
        /// Defaulted to false, and the default is the dangerous one — which is on
        /// purpose. A frontend that forgot the field gets the behaviour the method is
        /// named after; one that wants a preview has to say so.
        #[serde(rename = "DryRun", default)]
        dry_run: bool,
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
        options: Option<Box<GlobalOptionsDto>>,
        #[serde(rename = "Excluded", default, skip_serializing_if = "Option::is_none")]
        excluded: Option<Vec<String>>,
        #[serde(
            rename = "LoadAtStartup",
            default,
            skip_serializing_if = "Option::is_none"
        )]
        load_at_startup: Option<bool>,
    },
    /// A frontend's wallpaper settings for one layout: recorded, then shown. The agent
    /// writes `wallpaper.json` and paints the desktop; the frontends only edit.
    SaveWallpaper {
        #[serde(rename = "LayoutId")]
        layout_id: String,
        #[serde(rename = "Settings")]
        settings: Box<lbm_store::wallpaper_settings::LayoutWallpaperSettings>,
    },
    WallpaperSettings {
        #[serde(rename = "LayoutId")]
        layout_id: String,
        #[serde(rename = "LayoutRevision")]
        layout_revision: u64,
    },
    ApplyWallpaper {
        #[serde(rename = "LayoutId")]
        layout_id: String,
        #[serde(rename = "LayoutRevision")]
        layout_revision: u64,
        #[serde(rename = "Settings")]
        settings: Box<lbm_store::wallpaper_settings::LayoutWallpaperSettings>,
    },
    /// The user's Stop.
    Stop,
    /// Rebuild the layout the automatic detection missed (#443).
    Refresh,
    /// Leave: the hook, then the agent.
    Quit,
    /// Probe the agent's current/previewed zones (an XML `Probed` event).
    Probe,
    /// Sweep an editor's document on a private copy: no save, preview or hook command.
    /// The result is a JSON lbm_ipc::probe::Report under this request's Id, never a
    /// broadcast. Legacy Probe/Probed keeps its existing XML contract.
    ProbeLayout {
        #[serde(rename = "LayoutId")]
        layout_id: String,
        #[serde(rename = "LayoutRevision")]
        layout_revision: u64,
        #[serde(rename = "Document")]
        document: Box<LayoutDocument>,
    },
    /// The processes seen in the foreground this session, oldest first — what the
    /// exclusion list is picked from.
    SeenProcesses,
    /// The multi-PC mode (`MultiPc`): its settings, this machine's id, and every machine
    /// paired or waiting to be, with the screens of those linked as remotes.
    Peers,
    /// Turns the mode on or off; `Port` and `Name` change only when given. Answered
    /// with the state once the listener is up (or could not be) or everything is closed.
    SetMultiPc {
        #[serde(rename = "Enabled")]
        enabled: bool,
        #[serde(rename = "Port", default, skip_serializing_if = "Option::is_none")]
        port: Option<u16>,
        #[serde(rename = "Name", default, skip_serializing_if = "Option::is_none")]
        name: Option<String>,
        /// The shortcut that locks the pointer on the machine it is on; empty takes it
        /// away. Changing it alone leaves the links up.
        #[serde(
            rename = "PointerLockShortcut",
            default,
            skip_serializing_if = "Option::is_none"
        )]
        pointer_lock_shortcut: Option<String>,
    },
    /// Connects to a machine as its master (`host`, `host:port`; the default port
    /// otherwise). Answered once it said who it is: the machine, `Pending` until the
    /// user accepts it, `Paired` if it already was.
    PeerConnect {
        #[serde(rename = "Address")]
        address: String,
    },
    /// The user trusts a machine waiting to be paired.
    PeerAccept {
        #[serde(rename = "MachineId")]
        machine_id: String,
    },
    /// The user does not trust a machine waiting to be paired.
    PeerRefuse {
        #[serde(rename = "MachineId")]
        machine_id: String,
    },
    /// Unpairs a machine (or drops one waiting): its link, if any, is closed.
    PeerForget {
        #[serde(rename = "MachineId")]
        machine_id: String,
    },
    /// Which machine a panel two machines know shows (`MultiPc`): `Source` is one of
    /// the sources of `Monitor` in the current layout, on its machine's desktop. Saved,
    /// and handed to a running engine. Refused for another layout than the current one,
    /// as an edit is.
    SetActiveSource {
        #[serde(rename = "LayoutId")]
        layout_id: String,
        #[serde(rename = "Monitor")]
        monitor: String,
        #[serde(rename = "Source")]
        source: String,
    },
    /// What the screens of a remote of this machine show (`PeerWallpapers`): the pictures
    /// it last sent, asked for when its `WallpaperRevision` in the `Peers` state changes.
    PeerWallpapers {
        #[serde(rename = "MachineId")]
        machine_id: String,
    },
    /// The rulers to draw on the screens of a remote of this machine (`PeerOverlay`):
    /// `lbm_overlay::scene::Band`s in that remote's own desktop pixels, which the remote
    /// checks against its screens; none takes them down. Owned by the frontend that asked:
    /// taken down when it goes.
    PeerOverlay {
        #[serde(rename = "MachineId")]
        machine_id: String,
        #[serde(rename = "Bands", default)]
        bands: Vec<Value>,
    },
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
    /// Changes after every completed rebuild, even when the same monitors remain
    /// attached (resolution, rotation, scale or placement changed). Additive in v3.
    #[serde(default)]
    pub layout_revision: u64,
    /// Configuration used for that build. Lets a reconnect distinguish an unchanged
    /// desktop from a revision counter restarting on different display geometry.
    #[serde(default)]
    pub display_signature: String,
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
    /// The current rescue shortcut, when the hook said it could not arm it. In the state
    /// because the hook says it once, often before any window is open. Additive.
    #[serde(default)]
    pub shortcut_unavailable: Option<String>,
    /// The panels whose last input switch failed (multi-PC, VCP 0x60), and why: until
    /// the panel is switched or leaves the layout. In the state rather than an event
    /// because the switch is carried out after `SetActiveSource` has been answered, and
    /// a window opened later must still see it. Additive; absent when empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub input_switch_failures: Vec<crate::input_switch::Failure>,
    /// The other machine that receives the mouse and the keyboard now, by the name it
    /// goes by here (multi-PC mode, this machine its master); `None` when this machine
    /// does. Follows the pointer across the border. Additive.
    #[serde(default)]
    pub driven_machine: Option<String>,
    /// The pointer is locked on the machine it is on — [`Self::driven_machine`], or
    /// this one — by the multi-PC pointer lock's shortcut. Additive.
    #[serde(default)]
    pub pointer_locked: bool,
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

/// The `Peers` event: the multi-PC state, whenever it changes.
pub fn peers_event(state: &crate::peers::PeersState) -> String {
    json!({ "Event": "Peers", "Peers": state }).to_string()
}

/// The `Hook` event: what the hook said, forwarded.
pub fn hook_event(name: &str, payload: &str) -> String {
    json!({ "Event": "Hook", "Hook": name, "Payload": payload }).to_string()
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

    /// The same connection as `other` — not merely another frontend that looks like it.
    ///
    /// Two clones of one connection answer `true`; two separate frontends answer `false`.
    /// It is what lets the agent tell *whose* preview ended when a connection drops.
    pub fn is(&self, other: &Client) -> bool {
        self.out.same_channel(&other.out)
    }
}

/// What reaches the agent from a frontend.
///
/// A departure is one of the two, because some of what a frontend asks for **belongs to
/// its connection** and has to end with it — a live preview above all: it is a `Load` and
/// a `Run`, so a frontend that goes away without ending it leaves the engine driving an
/// arrangement nobody saved.
// A departure is far smaller than a request, and clippy would have the big one boxed.
// Nothing is gained: these are channel messages sent one at a time, never collected, and
// the channel already carried a whole `Request` before departures existed. Boxing would
// buy back a few bytes on the rarest message the agent receives, at the price of an
// indirection in the shape every caller reads.
#[allow(clippy::large_enum_variant)]
#[derive(Debug)]
pub enum Call {
    /// A request, with who asked.
    Asked {
        id: u64,
        request: Request,
        client: Client,
    },
    /// A frontend's connection ended, cleanly or not.
    Gone(Client),
}

impl Call {
    /// The request and its id, when this call is one — `None` for a departure.
    ///
    /// For a caller that only deals in requests, which is most of them: the tray's tests,
    /// and anything reading a channel it filled itself.
    pub fn asked(self) -> Option<(u64, Request)> {
        match self {
            Call::Asked { id, request, .. } => Some((id, request)),
            Call::Gone(_) => None,
        }
    }
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
                    .send(Call::Asked {
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
    // send fails, which is how the agent learns the frontend left. That is enough for a
    // subscriber list, which is pruned whenever something is broadcast — and not enough
    // for a preview, which an agent with nothing to say would go on running for ever. So
    // the departure is announced.
    writing.abort();
    let _ = calls.send(Call::Gone(client));
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
        let event: Value = serde_json::from_str(&hook_event("Probed", "<ProbeReport />")).unwrap();
        assert_eq!(
            event,
            json!({ "Event": "Hook", "Hook": "Probed", "Payload": "<ProbeReport />" })
        );
    }
}
