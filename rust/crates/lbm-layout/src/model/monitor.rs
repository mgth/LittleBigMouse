use crate::geo::{Point, Thickness};

use super::{BorderResistance, MmSize, Ratio};

/// A monitor make and model, shared by every monitor reporting the same PnP
/// code: C#'s `PhysicalMonitorModel`.
#[derive(Clone, Debug, PartialEq)]
pub struct MonitorModel {
    pub pnp_code: String,
    pub pnp_device_name: Option<String>,
    /// Brand logo icon path.
    pub logo: Option<String>,
    /// The panel's intrinsic (unrotated) size and bezel borders.
    pub physical_size: MmSize,
    /// C# `PhysicalMonitorModel.DetectedSize`: the size the platform detected, captured
    /// by the first load that applies a store over it; `None` until then. The store
    /// migrations compare a stored size with this, never with the current size: a
    /// reload of the same model (Undo, an agent applying a document over the profile it
    /// loaded) would otherwise take the profile's own size for a detection. Not
    /// persisted.
    pub detected_size: Option<(f64, f64)>,
    /// C# `PhysicalMonitorModel.UnresolvedStoredSize`: the stored size as read, while
    /// its pre-5.4.1 orientation migration could not be decided because the detected
    /// size had no shape yet; `None` otherwise. Neither persisted nor tracked for
    /// `saved`: it only tells the next save not to mark this size intrinsic unless the
    /// user has changed it since.
    pub unresolved_stored_size: Option<(f64, f64)>,
}

impl MonitorModel {
    pub fn new(pnp_code: impl Into<String>) -> Self {
        Self {
            pnp_code: pnp_code.into(),
            pnp_device_name: None,
            logo: None,
            physical_size: MmSize::default(),
            detected_size: None,
            unresolved_stored_size: None,
        }
    }
}

/// One physical monitor: C#'s `PhysicalMonitor`.
///
/// The geometry is not stored: the layout derives it from the model size, the
/// active source's orientation, the depth ratio and [`Monitor::location`]
/// (see `Layout::depth_projection`). The location is the one piece of state
/// the C# `DisplayLocate` node carries.
#[derive(Clone, Debug, PartialEq)]
pub struct Monitor {
    pub id: String,
    /// PnP code of the shared [`MonitorModel`].
    pub model: String,
    /// Ids of this monitor's sources, in insertion order.
    pub sources: Vec<String>,
    /// Id of the source currently displayed.
    pub active_source: Option<String>,
    pub device_id: Option<String>,
    pub split_sources: bool,
    /// Kept out of the mouse layout: attached, but no zone (#504).
    pub excluded_from_layout: bool,
    pub serial_number: Option<String>,
    /// Placement set by the user or by automatic placement.
    pub placed: bool,
    /// This monitor's own bezel borders (C#'s `DisplayBorders`). While
    /// `borders_customized` is false they mirror the model's; see
    /// `Layout::effective_physical_size`.
    pub(crate) borders: Thickness,
    pub(crate) borders_customized: bool,
    pub depth_ratio: Ratio,
    /// Top-left of the panel in the layout, in mm: `DepthProjection.X/Y`.
    pub(crate) location: Point,
    pub border_resistance: BorderResistance,
}

impl Monitor {
    /// A monitor whose own borders start as a copy of the model's, as
    /// `MonitorBorderPolicy` seeds them.
    pub fn new(id: impl Into<String>, model: &MonitorModel) -> Self {
        Self {
            id: id.into(),
            model: model.pnp_code.clone(),
            sources: Vec::new(),
            active_source: None,
            device_id: None,
            split_sources: false,
            excluded_from_layout: false,
            serial_number: None,
            placed: false,
            borders: model.physical_size.borders(),
            borders_customized: false,
            depth_ratio: Ratio::uniform(1.0),
            location: Point::new(0.0, 0.0),
            border_resistance: BorderResistance::default(),
        }
    }

    pub fn borders(&self) -> Thickness {
        self.borders
    }

    pub fn borders_customized(&self) -> bool {
        self.borders_customized
    }

    pub fn location(&self) -> Point {
        self.location
    }
}
