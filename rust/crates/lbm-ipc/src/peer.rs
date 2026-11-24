//! What a machine tells another about its screens in the multi-PC mode
//! (docs/multi-pc.md).
//!
//! Here rather than in the network crate because three parties read it: the agent
//! that receives it from the network, the layout code that merges it, and the
//! frontend, which detects displays by itself and so has to be handed the remote
//! ones by the agent.

use serde::{Deserialize, Serialize};

/// One output of a remote machine, as its own detection reports it. OS-neutral: the
/// raw EDID travels so that the receiver identifies the panel with the same parser
/// it uses on its own.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct PeerDisplay {
    /// The monitor id on the remote machine: stable there, opaque here.
    pub id: String,
    /// The connector (Linux) or device name (Windows) the remote addresses it by.
    #[serde(default)]
    pub connector: String,
    #[serde(default)]
    pub name: String,
    /// The raw EDID block, base64. Absent when the remote could not read one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edid: Option<String>,
    /// The pixel rectangle on the remote's desktop — the space `InputEvent::Motion`
    /// is expressed in for that machine.
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    #[serde(default = "one")]
    pub scale: f64,
    #[serde(default)]
    pub effective_dpi: f64,
    /// Quarter turns clockwise, 0 to 3.
    #[serde(default)]
    pub orientation: u8,
    #[serde(default)]
    pub frequency: f64,
    #[serde(default)]
    pub primary: bool,
    /// Whether the output is part of the remote's desktop right now.
    #[serde(default)]
    pub attached: bool,
    /// The oriented physical size the remote reports, in millimetres; 0 when unknown.
    #[serde(default)]
    pub width_mm: f64,
    #[serde(default)]
    pub height_mm: f64,
    /// The monitor's current VCP 0x60 value, when the remote could read it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_source: Option<u16>,
}

fn one() -> f64 {
    1.0
}

/// A remote machine and its screens, as the agent hands them to a frontend.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Peer {
    /// The SHA-256 fingerprint of the machine's certificate, lowercase hex.
    pub machine_id: String,
    pub name: String,
    #[serde(default)]
    pub address: String,
    #[serde(default)]
    pub connected: bool,
    #[serde(default)]
    pub displays: Vec<PeerDisplay>,
    /// Changes whenever the master holds other wallpapers for this machine's screens;
    /// `0` while it holds none. The pictures themselves are asked for (`PeerWallpapers`):
    /// they are too large for a state sent to every frontend at every change.
    #[serde(default)]
    pub wallpaper_revision: u64,
}

/// What one screen of a remote shows, drawn small for its master's map: either a picture
/// or a solid colour. A remote that could not read a screen's wallpaper sends nothing for
/// it, and the map draws that screen as it would without.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct PeerWallpaper {
    /// The display's id, as in its machine's `Displays`.
    pub display: String,
    /// A JPEG or PNG, base64 (standard alphabet, padded), at most 384 pixels wide.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    /// `#RRGGBB`: a solid background, or what is left of a picture too large to send.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
}

/// The capability (`lbm_net::wire::Hello::capabilities`) of a node that tells, and
/// hears, when Windows keeps a remote's injected input from reaching its desktop
/// ([`InjectionBlock`]). A remote says it only to a master that announced it; an older
/// master would ignore the message anyway, but the remote would be talking to nobody.
pub const INJECTION_BLOCKED_CAPABILITY: &str = "InjectionBlocked";

/// Why a remote's injected input does not reach its desktop: the window in the
/// foreground belongs to a process of a higher integrity level than the hook's (an
/// elevated program, the on-screen keyboard, Task Manager), and UIPI drops everything
/// a lower one injects while it has the focus — the pointer's motion included
/// (docs/multi-pc-etat.md, "Fenêtres élevées"). Only Windows has this.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct InjectionBlock {
    /// The file name of the process whose window has the focus (`osk.exe`); empty when
    /// it could not be read.
    #[serde(default)]
    pub process: String,
}
