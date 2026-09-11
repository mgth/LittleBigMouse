//! The LittleBigMouse layout model, ported from the C# domain core
//! (`LittleBigMouse.DisplayLayout`, `LittleBigMouse.Zoning`).
//!
//! The port is checked against `domain-oracle/` at the repository root, which
//! records what the C# code computes on a corpus of scenarios. Where the C#
//! does something surprising, the Rust does it too, and says so: the oracle is
//! the specification until the C# is gone.

pub mod collation;
pub mod geo;
pub mod linux;
pub mod model;
pub mod solve;
pub mod wallpaper;
pub mod zoning;
