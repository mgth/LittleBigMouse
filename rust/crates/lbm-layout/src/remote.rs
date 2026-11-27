//! How a remote machine's displays join this machine's layout in the multi-PC mode
//! (docs/multi-pc.md, "Model"). Pure, like [`crate::linux`] and [`crate::windows`]:
//! decoding what the remote sent (its EDIDs among it) is the caller's business, and
//! [`RemoteMonitor`] is the neutral description it hands over.
//!
//! A remote display whose panel this layout already knows ([`crate::model::panel_key`])
//! becomes a second source of that monitor; any other becomes a monitor of its own with
//! one source, named as this machine would name the panel ([`remote_monitor_id`]). Either
//! way the source carries the machine id as its host. Nothing is ever removed: a
//! machine that left is dropped by rebuilding the layout without it, as a display that
//! left is.

use crate::geo::{Point, Rect, Size};
use crate::model::{
    DisplaySize, DisplaySource, Layout, Monitor, MonitorModel, PhysicalSource, Ratio,
};

/// One display of a remote machine, as the master's mapping reads it: the remote's
/// report (`lbm_ipc::peer::PeerDisplay`) with its EDID already decoded.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RemoteMonitor {
    /// The monitor id on the remote: stable there, opaque here.
    pub id: String,
    /// The connector or device name the remote addresses it by.
    pub connector: String,
    pub name: String,
    /// [`crate::model::panel_key`] of its EDID.
    pub panel_key: Option<String>,
    /// [`crate::model::panel_identity`] of its EDID.
    pub panel_identity: Option<String>,
    /// The id this machine would give the panel if it showed it itself, when that is
    /// not its panel key: on a Windows master, the Windows monitor id computed from
    /// the EDID the remote sent (`lbm_display::peer`). `None` elsewhere, and without
    /// an EDID.
    pub local_id: Option<String>,
    /// EDID manufacturer and product codes, when there is an EDID: the model the
    /// monitor shares with every panel of that make.
    pub pnp_code: Option<String>,
    /// The EDID's monitor name descriptor.
    pub model_name: Option<String>,
    pub video_interface: Option<String>,
    /// The EDID's image size, in mm, never rotated; `None` when it gives none.
    pub edid_size_mm: Option<(f64, f64)>,
    /// The rectangle on the remote's desktop, in its pixels.
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub scale: f64,
    /// 0 when the remote did not say: then 96 per scale unit, as on Linux.
    pub effective_dpi: f64,
    /// Quarter turns.
    pub orientation: i32,
    pub frequency: i32,
    pub attached: bool,
    /// The oriented physical size the remote reports, 0 when unknown.
    pub width_mm: f64,
    pub height_mm: f64,
    pub input_source: Option<u16>,
}

/// How many characters of the machine id the ids of a remote's monitors and sources
/// carry: enough to tell a handful of machines apart, short enough to keep a layout id
/// (every monitor id, joined) under the store's 255 characters as long as it can.
const HOST_ID_LENGTH: usize = 8;

/// The source id a remote display gets here: its id on the remote, then `@` and the
/// start of the machine id — the way an id already taken on Linux gets `@` and its
/// connector. Stable as long as the remote's id is, and never one of this machine's ids.
/// It is also the id of the monitor made for a display without a panel key
/// ([`remote_monitor_id`]).
pub fn remote_source_id(machine_id: &str, display_id: &str) -> String {
    let short: String = machine_id.chars().take(HOST_ID_LENGTH).collect();
    format!("{display_id}@{short}")
}

/// The id of the monitor made for a remote display that no monitor of the layout
/// already shows: the id this machine gives the panel when it shows it itself, when no
/// monitor holds that id yet, else its source id.
///
/// That id, because a hardware KVM moves a panel from one machine to the other: this
/// machine stops detecting it and the remote starts publishing it. Named as this
/// machine names it, the panel keeps one id whichever machine shows it, and with it the
/// layout id, the profile and everything stored for it (placement, borders,
/// resistance). Linux names a local monitor `{pnp}_{serial}`, which is exactly the
/// panel key. Windows names it `{pnp}{serial string}_{week}_{year}_{checksum}`: that
/// is `local_id`, computed from the EDID the remote sent, the same block this machine
/// reads when the KVM points here. A Windows master named it by its panel key before:
/// the profile saved for the panel on the remote under that name is left behind, and
/// the one of the panel shown here is used both ways from then on.
///
/// An id already taken falls back to the next, then to the source id, which no local
/// monitor can have: a twin of a panel already in the layout (two monitors reporting
/// the same serial, the case Linux tells apart with `@{connector}`, and on Windows two
/// panels of one model, week and checksum without a serial string), or a coincidence.
/// Which twin gets the id then follows the remote's order, as it follows the
/// connectors' on Linux.
pub fn remote_monitor_id(layout: &Layout, source_id: &str, display: &RemoteMonitor) -> String {
    [display.local_id.as_deref(), display.panel_key.as_deref()]
        .into_iter()
        .flatten()
        .find(|id| layout.monitor(id).is_none())
        .unwrap_or(source_id)
        .to_owned()
}

/// Merges one remote machine's displays into `layout`: a shared panel gains a source,
/// any other display becomes a monitor of its own; then the active source of every
/// shared panel is resolved ([`Layout::resolve_active_sources`]).
///
/// Call it after this machine's displays are mapped and before the id is computed and
/// the store loaded (the order of `linux::populate`), so that the profile is the one of
/// this set of monitors and the stored remote monitors are restored like any other.
/// Merging a machine a second time updates its sources in place.
///
/// A remote's `primary` flag is not carried over: the primary is this machine's.
pub fn merge_remote(
    layout: &mut Layout,
    machine_id: &str,
    machine_name: &str,
    displays: &[RemoteMonitor],
) {
    for display in displays {
        add_remote_monitor(layout, machine_id, machine_name, display);
    }
    layout.resolve_active_sources();
}

/// Whether the remote display is this monitor's panel: the same
/// [`crate::model::panel_key`], or the same [`crate::model::panel_identity`] for a
/// monitor whose inputs carry different product codes. Never two missing keys.
fn is_same_panel(monitor: &Monitor, display: &RemoteMonitor) -> bool {
    let same = |a: &Option<String>, b: &Option<String>| a.is_some() && a == b;
    same(&monitor.panel_key, &display.panel_key)
        || same(&monitor.panel_identity, &display.panel_identity)
}

fn add_remote_monitor(
    layout: &mut Layout,
    machine_id: &str,
    machine_name: &str,
    display: &RemoteMonitor,
) {
    let source_id = remote_source_id(machine_id, &display.id);
    let source = PhysicalSource::new(
        source_id.clone(),
        String::new(),
        display_source(machine_id, machine_name, &source_id, display),
    );

    // A panel already known, and not through this machine: a second source of it.
    let shared = layout
        .monitors()
        .iter()
        .find(|m| {
            is_same_panel(m, display)
                && !m.sources.iter().any(|s| {
                    s != &source_id
                        && layout
                            .source(s)
                            .is_some_and(|s| s.source.host.as_deref() == Some(machine_id))
                })
        })
        .map(|m| m.id.clone());
    if let Some(monitor_id) = shared {
        let source = PhysicalSource {
            monitor: monitor_id.clone(),
            ..source
        };
        layout.attach_source(source.clone());
        if let Some(i) = layout.monitor_index(&monitor_id) {
            if !layout.monitors[i].sources.contains(&source_id) {
                layout.monitors[i].sources.push(source_id);
            }
        }
        layout.add_or_update_source(source);
        return;
    }

    // Merged before: the monitor this source already has, whatever it is named.
    let monitor_id = layout
        .monitors()
        .iter()
        .find(|m| m.sources.contains(&source_id))
        .map_or_else(
            || remote_monitor_id(layout, &source_id, display),
            |m| m.id.clone(),
        );
    let pnp_code = display
        .pnp_code
        .clone()
        .unwrap_or_else(|| monitor_id.clone());
    let model = layout
        .get_or_add_model(&pnp_code, |code| remote_model(code, display))
        .clone();
    let mut monitor = Monitor::new(monitor_id.clone(), &model);
    monitor.device_id = Some(source_id.clone());
    monitor.panel_key = display.panel_key.clone();
    monitor.panel_identity = display.panel_identity.clone();
    // A monitor merged before keeps what was placed, chosen or edited on it.
    if let Some(previous) = layout.monitor(&monitor_id) {
        monitor = Monitor {
            sources: Vec::new(),
            active_source: None,
            ..previous.clone()
        };
    }
    monitor.active_source = Some(source_id.clone());
    monitor.sources.push(source_id);
    let source = PhysicalSource {
        monitor: monitor_id,
        ..source
    };
    layout.attach_source(source.clone());
    layout.add_or_update_monitor(monitor);
    layout.add_or_update_source(source);
}

fn display_source(
    machine_id: &str,
    machine_name: &str,
    source_id: &str,
    display: &RemoteMonitor,
) -> DisplaySource {
    let connector = display.connector.as_str();
    let mut source = DisplaySource::new(source_id);
    source.host = Some(machine_id.to_owned());
    source.input_source = display.input_source;
    source.device_name = Some(connector.to_owned());
    source.interface_path = Some(connector.to_owned());
    source.display_name = Some(if display.name.is_empty() {
        connector.to_owned()
    } else {
        display.name.clone()
    });
    source.source_name = Some(format!(
        "{}:{connector}",
        display.video_interface.as_deref().unwrap_or("Unknown")
    ));
    // Where the map says which machine a source is on.
    source.interface_name = Some(machine_name.to_owned());
    source.attached_to_desktop = display.attached;
    source.orientation = display.orientation;
    source.display_frequency = display.frequency;
    source.in_pixel = DisplaySize::from_rect(Rect::from_location_size(
        Point::new(f64::from(display.x), f64::from(display.y)),
        Size::new(f64::from(display.width), f64::from(display.height)),
    ));
    let effective_dpi = if display.effective_dpi > 0.0 {
        display.effective_dpi
    } else {
        96.0 * display.scale
    };
    source.effective_dpi = Ratio::uniform(effective_dpi);
    source.dpi_aware_angular_dpi = Ratio::uniform(effective_dpi);
    // The remote's pixels over the oriented millimetres, as the Linux mapping does.
    let raw = |pixels: i32, mm: f64| {
        if mm > 0.0 {
            f64::from(pixels) * 25.4 / mm
        } else {
            effective_dpi
        }
    };
    source.raw_dpi = Ratio::new(
        raw(display.width, display.width_mm),
        raw(display.height, display.height_mm),
    );
    source
}

/// The model of a remote panel no local monitor shares: its size is the EDID's, else
/// the remote's report turned back to the panel's own orientation.
fn remote_model(code: &str, display: &RemoteMonitor) -> MonitorModel {
    let mut model = MonitorModel::new(code);
    model.pnp_device_name = Some(
        display
            .model_name
            .clone()
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| {
                if display.name.is_empty() {
                    display.connector.clone()
                } else {
                    display.name.clone()
                }
            }),
    );
    let (width_mm, height_mm) = match display.edid_size_mm {
        Some((w, h)) if w > 0.0 && h > 0.0 => (w, h),
        _ if display.orientation % 2 != 0 => (display.height_mm, display.width_mm),
        _ => (display.width_mm, display.height_mm),
    };
    if width_mm > 0.0 && height_mm > 0.0 {
        let size = &mut model.physical_size;
        let fixed = size.fixed_aspect_ratio();
        size.set_fixed_aspect_ratio(false);
        size.set_width(width_mm);
        size.set_height(height_mm);
        size.set_fixed_aspect_ratio(fixed);
    }
    if let Some(manufacturer) = display.pnp_code.as_deref().and_then(|c| c.get(..3)) {
        model.logo = Some(format!("icon/Pnp/{manufacturer}?icon/Pnp/LBM"));
    }
    model
}
