//! A remote machine's screens in the multi-PC mode (docs/multi-pc.md): its report,
//! [`PeerDisplay`], read into the neutral [`RemoteMonitor`] the layout merges.
//!
//! Here because the EDID travels raw, and is decoded with the parser that reads this
//! machine's own: the panel key two machines compare is then computed by the same code
//! on both sides of the comparison.

use base64::Engine as _;
use lbm_ipc::peer::PeerDisplay;
use lbm_layout::model::Layout;
use lbm_layout::remote::{self, RemoteMonitor};

use crate::edid::{self, Edid};

/// The display's EDID, when it sent one that decodes. A block that does not decode is
/// treated as no EDID at all: it can identify nothing.
pub fn peer_edid(display: &PeerDisplay) -> Option<Edid> {
    let raw = display.edid.as_deref()?;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(raw.trim())
        .ok()?;
    Some(edid::parse(format!("peer:{}", display.id), &bytes))
}

/// The cross-machine key of the display's panel ([`lbm_layout::model::panel_key`]);
/// `None` without an EDID or without a serial in it.
pub fn panel_key(display: &PeerDisplay) -> Option<String> {
    peer_edid(display)?.panel_key()
}

/// The display as the layout's merge reads it.
pub fn remote_monitor(display: &PeerDisplay) -> RemoteMonitor {
    let edid = peer_edid(display);
    let non_empty = |s: &Option<String>| s.clone().filter(|s| !s.is_empty());
    RemoteMonitor {
        id: display.id.clone(),
        connector: display.connector.clone(),
        name: display.name.clone(),
        panel_key: edid.as_ref().and_then(Edid::panel_key),
        pnp_code: edid.as_ref().and_then(|e| {
            Some(format!(
                "{}{}",
                non_empty(&e.manufacturer_code)?,
                e.product_code.as_deref().unwrap_or("")
            ))
        }),
        model_name: edid.as_ref().and_then(|e| non_empty(&e.model)),
        video_interface: edid.as_ref().and_then(|e| e.video_interface.clone()),
        edid_size_mm: edid
            .as_ref()
            .map(|e| (e.physical_width, e.physical_height))
            .filter(|&(w, h)| w > 0.0 && h > 0.0),
        x: display.x,
        y: display.y,
        width: display.width,
        height: display.height,
        scale: display.scale,
        effective_dpi: display.effective_dpi,
        orientation: i32::from(display.orientation),
        frequency: display.frequency.round() as i32,
        attached: display.attached,
        width_mm: display.width_mm,
        height_mm: display.height_mm,
        input_source: display.input_source,
    }
}

/// Merges a remote machine's displays into `layout` ([`remote::merge_remote`]): a panel
/// this layout already has gains the remote's source, any other display becomes a
/// monitor of its own. `machine_id` is the machine's certificate fingerprint,
/// `machine_name` what the map shows it as.
pub fn merge_peer(
    layout: &mut Layout,
    machine_id: &str,
    machine_name: &str,
    displays: &[PeerDisplay],
) {
    let remotes: Vec<RemoteMonitor> = displays.iter().map(remote_monitor).collect();
    remote::merge_remote(layout, machine_id, machine_name, &remotes);
}
