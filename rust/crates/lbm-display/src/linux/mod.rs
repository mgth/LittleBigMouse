//! Linux display discovery — port of the enumeration half of `LittleBigMouse.Platform.Linux`:
//! the best available output source (`KScreenMonitorSource` on KDE, `XRandRMonitorSource`
//! anywhere else, in that order), enriched with EDID identity and physical size from
//! sysfs (`DrmEdidReader`).
//!
//! What comes out is the neutral output list the layout model maps
//! (`lbm_layout::linux::populate`), the same records the domain oracle's scenarios hold.

pub mod desktop;
pub mod drm;
pub mod kscreen;
pub mod xrandr;

use lbm_layout::geo::dotnet::format_double;
use lbm_layout::linux::LinuxMonitor;

use desktop::DesktopEnvironment;
pub use kscreen::KScreenError;

/// An output source (C# `ILinuxMonitorSource`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Backend {
    KScreen,
    XRandR,
}

impl Backend {
    /// C# `ILinuxMonitorSource.Name`.
    pub fn name(self) -> &'static str {
        match self {
            Backend::KScreen => "kscreen",
            Backend::XRandR => "xrandr",
        }
    }

    /// The first available source, KScreen first (C#: the `LinuxLayoutFactory`
    /// constructor); `None` when neither answers.
    pub fn detect() -> Option<Backend> {
        if kscreen::is_available(&DesktopEnvironment::current()) {
            Some(Backend::KScreen)
        } else if xrandr::is_available() {
            Some(Backend::XRandR)
        } else {
            None
        }
    }

    /// C# `ILinuxMonitorSource.Query`: the outputs now, EDIDs read from sysfs. A source
    /// that stopped answering gives none.
    pub fn query(self) -> Result<Vec<LinuxMonitor>, KScreenError> {
        match self {
            Backend::KScreen => match kscreen::run_kscreen_doctor() {
                Some(json) => kscreen::parse(&json, &drm::read_all()),
                None => Ok(Vec::new()),
            },
            Backend::XRandR => Ok(xrandr::run_xrandr()
                .map(|stdout| xrandr::parse(&stdout, &drm::read_all()))
                .unwrap_or_default()),
        }
    }
}

/// C# `LinuxLayoutFactory.DisplaySignature`, over a query's outputs: what the
/// compositor shows, in connector order — position, mode, scale, primary, enabled and
/// reported size. Numbers are written in the invariant culture (C# uses the current
/// one; the signature is only ever compared with itself).
pub fn display_signature(monitors: &[LinuxMonitor]) -> String {
    let mut sorted: Vec<&LinuxMonitor> = monitors.iter().collect();
    sorted.sort_by(|a, b| a.connector_name.cmp(&b.connector_name));
    sorted
        .iter()
        .map(|m| {
            format!(
                "{}[{},{} {}x{}@{}]{}{}d{}x{}",
                m.connector_name,
                format_double(m.logical_x),
                format_double(m.logical_y),
                m.pixel_width,
                m.pixel_height,
                format_double(m.scale),
                if m.primary { "*" } else { "" },
                if m.enabled { "" } else { "-" },
                format_double(m.width_mm),
                format_double(m.height_mm),
            )
        })
        .collect::<Vec<_>>()
        .join("|")
}

/// An output as the domain oracle's `input.json` holds it (C# `OracleInput`, and
/// `DisplayDump.Display` on the C# side of `--dump-displays`).
pub fn display_json(m: &LinuxMonitor) -> serde_json::Value {
    serde_json::json!({
        "ConnectorName": m.connector_name,
        "LogicalX": m.logical_x,
        "LogicalY": m.logical_y,
        "LogicalWidth": m.logical_width,
        "LogicalHeight": m.logical_height,
        "PixelWidth": m.pixel_width,
        "PixelHeight": m.pixel_height,
        "Scale": m.scale,
        "WidthMm": m.width_mm,
        "HeightMm": m.height_mm,
        "Primary": m.primary,
        "Enabled": m.enabled,
        "Orientation": m.orientation,
        "Frequency": m.frequency,
        "Edid": m.edid.as_ref().map(|e| serde_json::json!({
            "ManufacturerCode": e.manufacturer_code,
            "ProductCode": e.product_code,
            "Serial": e.serial,
            "SerialNumber": e.serial_number,
            "Model": e.model,
            "PhysicalWidth": e.physical_width,
            "PhysicalHeight": e.physical_height,
            "VideoInterface": e.video_interface,
        })),
    })
}
