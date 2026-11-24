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

/// The bytes of an EDID's base block: what a Windows monitor id is computed from.
const EDID_BASE_BLOCK: usize = 128;

/// How this machine names its own monitors: what a remote's panel is named after, so
/// that a panel a KVM moves between the two machines keeps one id
/// ([`lbm_layout::remote::remote_monitor_id`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IdForm {
    /// `{pnp}_{serial}`: the panel key itself.
    Linux,
    /// `{pnp}{serial string}_{week}_{year}_{checksum}` (`crate::windows::ids`).
    Windows,
}

impl IdForm {
    /// The form of this machine's ids.
    pub const HERE: IdForm = if cfg!(windows) {
        IdForm::Windows
    } else {
        IdForm::Linux
    };
}

/// The display as the layout's merge reads it, on this machine.
pub fn remote_monitor(display: &PeerDisplay) -> RemoteMonitor {
    remote_monitor_as(display, IdForm::HERE)
}

/// The display as the layout's merge reads it, on a machine whose monitor ids have
/// the form `form`.
pub fn remote_monitor_as(display: &PeerDisplay, form: IdForm) -> RemoteMonitor {
    let edid = peer_edid(display);
    let non_empty = |s: &Option<String>| s.clone().filter(|s| !s.is_empty());
    let pnp_code = edid.as_ref().and_then(|e| {
        Some(format!(
            "{}{}",
            non_empty(&e.manufacturer_code)?,
            e.product_code.as_deref().unwrap_or("")
        ))
    });
    // Windows' own id needs the EDID only, and its PnP code is the EDID's manufacturer
    // and product: `MONITOR\SAME035\...` for manufacturer SAM, product E035.
    // Only from a whole base block: a shorter one already gives the manufacturer, while
    // the product, the week, the year and the checksum would be defaults — an id the
    // panel loses the day the remote sends all of it.
    let local_id = match form {
        IdForm::Linux => None,
        IdForm::Windows => edid
            .as_ref()
            .filter(|e| e.raw.len() >= EDID_BASE_BLOCK && e.product_code.is_some())
            .zip(pnp_code.as_deref())
            .map(|(e, pnp)| crate::windows::ids::source_id_of(pnp, e)),
    };
    RemoteMonitor {
        id: display.id.clone(),
        connector: display.connector.clone(),
        name: display.name.clone(),
        panel_key: edid.as_ref().and_then(Edid::panel_key),
        local_id,
        pnp_code,
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
    merge_peer_as(layout, machine_id, machine_name, displays, IdForm::HERE);
}

/// [`merge_peer`] on a machine whose monitor ids have the form `form`.
pub fn merge_peer_as(
    layout: &mut Layout,
    machine_id: &str,
    machine_name: &str,
    displays: &[PeerDisplay],
    form: IdForm,
) {
    let remotes: Vec<RemoteMonitor> = displays
        .iter()
        .map(|display| remote_monitor_as(display, form))
        .collect();
    remote::merge_remote(layout, machine_id, machine_name, &remotes);
}
