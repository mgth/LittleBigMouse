//! Two-dimensional geometry with the semantics of HLab.Geo, the C# library the
//! layout model was written against (a copy of WPF's `System.Windows` types).
//!
//! This is not `lbm-geom`, on purpose. `lbm-geom` ports the hook's C++
//! templates (integer pixels, `MAX` sentinels for "empty"); this module
//! reproduces HLab.Geo's doubles bit for bit, including where HLab.Geo departs
//! from WPF, because the domain oracle freezes what that code computes.
//!
//! Only the members the domain uses are ported. C# throws on a negative width
//! or height; here that is a debug assertion, and a release build keeps the
//! value (a negative width then reads as empty, like HLab.Geo's own test).

pub mod dotnet;
mod point;
mod rect;
mod size;
mod thickness;
mod vector;

pub use point::Point;
pub use rect::Rect;
pub use size::Size;
pub use thickness::Thickness;
pub use vector::Vector;
