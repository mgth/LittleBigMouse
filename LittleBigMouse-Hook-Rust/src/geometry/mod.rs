//! Geometry primitives — port of the header-only `geo::` templates
//! (`Geometry/Point.h`, `Rect.h`, `Line.h`, `Segment.h`, `Sides.h`).
//!
//! The C++ uses templates instantiated with `long` (pixels) and `double`
//! (millimetres). Here the types are generic over a small [`Coord`] trait
//! implemented for `i32` (Win32 `long`) and `f64`, so the port stays a faithful
//! 1:1 translation while all mixed int/float math routes through `f64`.

pub mod line;
pub mod point;
pub mod rect;
pub mod segment;
pub mod sides;

pub use line::Line;
pub use point::Point;
pub use rect::Rect;
pub use segment::Segment;
pub use sides::Side;

use std::ops::{Add, Mul, Sub};

/// A coordinate scalar: `i32` for pixels, `f64` for millimetres.
pub trait Coord:
    Copy + PartialOrd + Add<Output = Self> + Sub<Output = Self> + Mul<Output = Self>
{
    /// `std::numeric_limits<T>::max()` — the "empty" sentinel.
    const MAX: Self;
    const ZERO: Self;
    fn to_f64(self) -> f64;
    fn from_f64(v: f64) -> Self;
}

impl Coord for i32 {
    const MAX: Self = i32::MAX;
    const ZERO: Self = 0;
    fn to_f64(self) -> f64 {
        self as f64
    }
    fn from_f64(v: f64) -> Self {
        v as i32
    }
}

impl Coord for f64 {
    const MAX: Self = f64::MAX;
    const ZERO: Self = 0.0;
    fn to_f64(self) -> f64 {
        self
    }
    fn from_f64(v: f64) -> Self {
        v
    }
}

/// `min` over `PartialOrd` values (C++ `min` macro), returning the first on ties.
pub(crate) fn cmin<T: Coord>(a: T, b: T) -> T {
    if b < a {
        b
    } else {
        a
    }
}

/// `max` over `PartialOrd` values (C++ `max` macro), returning the first on ties.
pub(crate) fn cmax<T: Coord>(a: T, b: T) -> T {
    if a < b {
        b
    } else {
        a
    }
}
