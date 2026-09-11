//! The layout model: monitors, their sources, and the geometry C# derives from
//! them through reactive chains, here as plain values and functions.

mod border_resistance;
mod distance;
mod layout;
mod monitor;
mod options;
mod ratio;
mod size;
mod source;

pub use border_resistance::{BorderResistance, BorderSection, BorderSide, ResistanceValues};
pub use distance::{distance, distance_hv};
pub use layout::{DpiAwareness, Layout, LayoutSource};
pub use monitor::{Monitor, MonitorModel};
pub use options::{LayoutOptions, PER_MODEL, PER_MONITOR};
pub use ratio::{inverse_of, Ratio};
pub use size::{DisplaySize, MmSize};
pub use source::{DisplaySource, PhysicalSource, WallpaperStyle};
