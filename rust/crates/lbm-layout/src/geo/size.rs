use std::ops::{Add, Sub};

use super::Thickness;

/// `HLab.Geo.Size`. The empty size is the only negative one: both dimensions
/// negative infinity.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Size {
    width: f64,
    height: f64,
}

impl Size {
    pub const EMPTY: Size = Size {
        width: f64::NEG_INFINITY,
        height: f64::NEG_INFINITY,
    };

    pub fn new(width: f64, height: f64) -> Self {
        debug_assert!(
            !(width < 0.0 || height < 0.0),
            "HLab.Geo rejects a negative size ({width} x {height})"
        );
        Self { width, height }
    }

    pub fn is_empty(self) -> bool {
        self.width < 0.0
    }

    pub fn width(self) -> f64 {
        self.width
    }

    pub fn height(self) -> f64 {
        self.height
    }

    pub fn transpose(self) -> Size {
        Size::new(self.height, self.width)
    }
}

impl Default for Size {
    fn default() -> Self {
        Size::new(0.0, 0.0)
    }
}

/// `size.Width + thickness.Left + thickness.Right`, left to right.
impl Add<Thickness> for Size {
    type Output = Size;
    fn add(self, t: Thickness) -> Size {
        Size::new(
            self.width + t.left + t.right,
            self.height + t.top + t.bottom,
        )
    }
}

/// `size.Width - (thickness.Left + thickness.Right)`: the borders are summed first.
impl Sub<Thickness> for Size {
    type Output = Size;
    fn sub(self, t: Thickness) -> Size {
        Size::new(
            self.width - (t.left + t.right),
            self.height - (t.top + t.bottom),
        )
    }
}
