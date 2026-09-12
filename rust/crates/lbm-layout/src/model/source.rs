use super::{DisplaySize, Ratio};

/// `WallpaperStyle`: how the desktop picture fills a screen.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum WallpaperStyle {
    #[default]
    Fill,
    Fit,
    Stretch,
    Tile,
    Center,
    Span,
}

/// A source as the operating system reports it: C#'s `DisplaySource`.
///
/// Strings are `None` where C# leaves them null. The three DPI ratios default
/// to 96 like C#'s `new DisplayRatioValue(96)`, and `in_pixel` to the empty
/// origin rectangle of `new DisplaySizeInPixels(new Rect())`.
#[derive(Clone, Debug, PartialEq)]
pub struct DisplaySource {
    pub id: String,
    pub primary: bool,
    pub device_name: Option<String>,
    pub source_name: Option<String>,
    pub display_name: Option<String>,
    pub interface_path: Option<String>,
    pub source_number: Option<String>,
    pub display_frequency: i32,
    /// Pixels, without borders (`DisplaySizeInPixels`).
    pub in_pixel: DisplaySize,
    /// Quarter turns: 0 = 0°, 1 = 90°, 2 = 180°, 3 = 270°.
    pub orientation: i32,
    pub wallpaper_path: Option<String>,
    pub wallpaper_style: WallpaperStyle,
    pub interface_name: Option<String>,
    pub interface_logo: Option<String>,
    pub attached_to_desktop: bool,
    pub raw_dpi: Ratio,
    pub effective_dpi: Ratio,
    pub dpi_aware_angular_dpi: Ratio,
}

impl DisplaySource {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            primary: false,
            device_name: None,
            source_name: None,
            display_name: None,
            interface_path: None,
            source_number: None,
            display_frequency: 0,
            in_pixel: DisplaySize::from_rect(crate::geo::Rect::default()),
            orientation: 0,
            wallpaper_path: None,
            wallpaper_style: WallpaperStyle::default(),
            interface_name: None,
            interface_logo: None,
            attached_to_desktop: false,
            raw_dpi: Ratio::uniform(96.0),
            effective_dpi: Ratio::uniform(96.0),
            dpi_aware_angular_dpi: Ratio::uniform(96.0),
        }
    }
}

/// A source seen through the monitor it belongs to: C#'s `PhysicalSource`.
#[derive(Clone, Debug, PartialEq)]
pub struct PhysicalSource {
    /// The device key sources are ordered by (a connector name on Linux).
    pub device_id: String,
    /// The owning monitor's id.
    pub monitor: String,
    pub source: DisplaySource,
    /// C#'s `SavableReactiveModel.Saved`: false until the store marks it.
    pub saved: bool,
    /// Whether the source is in the layout's source cache
    /// (`AddOrUpdatePhysicalSource`). A source can belong to its monitor before
    /// that: C#'s `AddMonitor` hands the monitor to the layout first, and the
    /// monitor's geometry already reads its active source.
    pub(crate) registered: bool,
    /// The primary source the layout had published when this source joined
    /// it. C# builds `DipToPixelRatio` from the primary observed through
    /// `Monitor.Layout.PrimarySource` at the source's construction, and the
    /// layout publishes every later primary with change notifications
    /// suppressed: a source created before any primary never gets the ratio,
    /// one created after keeps observing the primary it saw.
    pub(crate) observed_primary: Option<String>,
}

impl PhysicalSource {
    pub fn new(
        device_id: impl Into<String>,
        monitor: impl Into<String>,
        source: DisplaySource,
    ) -> Self {
        Self {
            device_id: device_id.into(),
            monitor: monitor.into(),
            source,
            saved: false,
            registered: false,
            observed_primary: None,
        }
    }
}
