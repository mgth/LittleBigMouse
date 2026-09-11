//! Windows display discovery — port of `HLab.Sys.Windows.Monitors`
//! (`MonitorDeviceHelper.GetDisplayDevices` and `DisplaySignature`, `DeviceFactory`,
//! `MonitorDevice`, `MonitorDeviceConnection`, `PhysicalAdapter`, `DisplayMode`,
//! `DeviceCaps`, `DeviceState`, `SystemMonitorsService`): the device tree
//! `EnumDisplayDevices` reports — adapter sources, the monitors below them — with each
//! monitor's EDID read through SetupAPI, the desktop state `EnumDisplayMonitors` and
//! `GetDpiForMonitor` add, and the CCD targets that give monitors their Settings number
//! and tell VR headsets apart.
//!
//! Only the Win32 calls are Windows-only (`sys`): they fill a [`RawDisplays`], and
//! everything C# decides from there — the tree, the ids, the order, the duplicates, the
//! numbers, the active connection — is [`DisplayTree::assemble`], pure, tested on any
//! platform. [`DisplayTree::layout_input`] is what `lbm_layout::windows` maps.
//!
//! Left out, each without effect on what the layout gets:
//! - The DDC/CI capabilities string `UpdateDpi` asks every monitor for: written to
//!   `PhysicalAdapter.CapabilitiesString`, which nothing reads, at up to 2 s a monitor.
//! - `GetScaleFactorForMonitor`, the work and monitor areas of `GetMonitorInfo` (their
//!   rectangles are still checked where HLab.Geo would throw), `DeviceKey`, the state
//!   flags but "attached", DEVMODE's bits per pixel, flags and fixed output, and
//!   `BITSPIXEL`/`ASPECTX`/`ASPECTY`: read by the Info view or the wallpaper only.
//! - `UpdateDevModes` (every mode of every device), and the device caps and current mode
//!   C# also asks for the root and for each monitor device: never read — the mapping
//!   reads the adapter's.
//! - The interface-path query C# also makes for each adapter source, whose answer
//!   `BuildDisplayDevice` drops: only monitors keep one.
//! - The quadratic scans. C# lists every monitor of the tree for each connection it
//!   adds, and walks every monitor device SetupAPI knows, opening their registry keys,
//!   for each new monitor. Here the monitors seen are indexed by id, and SetupAPI is
//!   walked once into a table ([`registry::EdidEntry`]) that answers each monitor with
//!   C#'s rule ([`registry::find_edid`]). The CCD configuration is queried once for the
//!   numbers and the specialization, each target's device path and specialization asked
//!   once. The answers are the same as long as the system does not change during one
//!   enumeration, which C# does not guard against either.
//!
//! Not ported yet: the desktop wallpaper (`UpdateWallpaper`, COM `IDesktopWallpaper`),
//! as on Linux.

pub mod ids;
pub mod registry;
#[cfg(windows)]
mod sys;
pub mod tree;

use lbm_layout::collation::invariant_compare;
use lbm_layout::model::DpiAwareness;
use serde_json::{json, Value};

pub use tree::{DiscoveryError, DisplayTree, RawDisplays, RawMonitorInfo};

/// C#'s `(DpiAwarenessKind)(int)GetAwarenessFromDpiAwarenessContext(...)`: -1 invalid,
/// 0 unaware, 1 system aware, 2 per-monitor aware. Windows defines no other value; one
/// would be an enum value C#'s DIP formulas throw on, and reads as invalid here.
pub fn dpi_awareness(value: i32) -> DpiAwareness {
    match value {
        0 => DpiAwareness::Unaware,
        1 => DpiAwareness::SystemAware,
        2 => DpiAwareness::PerMonitorAware,
        _ => DpiAwareness::Invalid,
    }
}

/// C# `MonitorDeviceHelper.DisplaySignature`, over `EnumDisplayMonitors`' answers: each
/// monitor's source, virtual-screen rectangle, primary flag and effective DPI (0 when
/// the query failed), sorted — with the invariant culture's comparer, C# uses the
/// current one; the signature is only compared with itself — and joined with `|`. Cheap
/// next to [`discover`]: it coalesces the `WM_DISPLAYCHANGE` burst and tells when the
/// system has settled. A rectangle is printed as Windows gives it, where C# would throw
/// on an inverted one (HLab.Geo's `Rect`), which `GetMonitorInfo` does not produce.
pub fn display_signature(infos: &[RawMonitorInfo]) -> String {
    let mut parts: Vec<String> = infos
        .iter()
        .map(|info| {
            let r = info.monitor;
            format!(
                "{}[{},{} {}x{}]{}d{}",
                info.device_name,
                r.left,
                r.top,
                r.width(),
                r.height(),
                if info.flags == 1 { "*" } else { "" },
                if info.effective_dpi.succeeded {
                    info.effective_dpi.x
                } else {
                    0
                },
            )
        })
        .collect();
    parts.sort_by(|a, b| invariant_compare(a, b));
    parts.join("|")
}

/// A monitor of the tree as `--dump-displays` prints it, and `DisplayDump.Windows` on
/// the C# side: its ids, its EDID, and its active connection with the adapter source it
/// is on.
pub fn display_json(tree: &DisplayTree, m: &tree::MonitorDevice) -> Value {
    json!({
        "Id": m.id,
        "PnpCode": m.pnp_code,
        "PhysicalId": m.physical_id,
        "SourceId": m.source_id,
        "InterfacePath": m.interface_path,
        "MonitorNumber": m.monitor_number,
        "IsSpecialized": m.is_specialized,
        "Edid": m.edid.as_ref().map(|e| json!({
            "ManufacturerCode": e.manufacturer_code,
            "ProductCode": e.product_code,
            "Serial": e.serial,
            "SerialNumber": e.serial_number,
            "Model": e.model,
            "Week": e.week,
            "Year": e.year,
            "Checksum": e.checksum,
            "PhysicalWidth": e.physical_width,
            "PhysicalHeight": e.physical_height,
            "VideoInterface": e.video_interface,
        })),
        "ActiveConnection": tree.active_connection(m).map(|(adapter, connection)| {
            let a = &adapter.adapter;
            let caps = &a.capabilities;
            json!({
                "DeviceName": connection.device.device_name,
                "DeviceString": connection.device.device_string,
                "AttachedToDesktop": connection.attached_to_desktop(),
                "Adapter": {
                    "DeviceName": a.device_name,
                    "DeviceString": a.device_string,
                    "Primary": a.primary,
                    "EffectiveDpi": { "X": a.effective_dpi.x, "Y": a.effective_dpi.y },
                    "AngularDpi": { "X": a.angular_dpi.x, "Y": a.angular_dpi.y },
                    "RawDpi": { "X": a.raw_dpi.x, "Y": a.raw_dpi.y },
                    "CurrentMode": a.current_mode.map(|mode| json!({
                        "X": mode.position.x,
                        "Y": mode.position.y,
                        "Width": mode.pels.width(),
                        "Height": mode.pels.height(),
                        "Orientation": mode.display_orientation,
                        "Frequency": mode.display_frequency,
                    })),
                    "Capabilities": {
                        "Width": caps.size.width(),
                        "Height": caps.size.height(),
                        "ResolutionWidth": caps.resolution.width(),
                        "ResolutionHeight": caps.resolution.height(),
                        "LogPixelsX": caps.log_pixels.width(),
                        "LogPixelsY": caps.log_pixels.height(),
                    },
                },
            })
        }),
    })
}

/// `SystemMonitorsService.Root` (`MonitorDeviceHelper.GetDisplayDevices`): the device
/// tree of this machine now. Fails only where C# throws.
#[cfg(windows)]
pub fn discover() -> Result<DisplayTree, DiscoveryError> {
    DisplayTree::assemble(sys::read())
}

/// [`display_signature`] of this machine now.
#[cfg(windows)]
pub fn current_display_signature() -> String {
    display_signature(&sys::monitor_infos())
}

/// The calling thread's DPI awareness, which C#'s builder stamps on the layout before
/// mapping (`GetAwarenessFromDpiAwarenessContext(GetThreadDpiAwarenessContext())`).
#[cfg(windows)]
pub fn thread_dpi_awareness() -> DpiAwareness {
    sys::thread_dpi_awareness()
}

/// Makes the process per-monitor DPI aware (v2), as the C# UI's manifest does. Without
/// it Windows virtualizes what the enumeration reads (positions, modes, DPIs) for an
/// unaware process. Does nothing when the awareness is already set.
#[cfg(windows)]
pub fn set_process_per_monitor_dpi_aware() {
    sys::set_process_per_monitor_dpi_aware();
}
