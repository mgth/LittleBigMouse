use std::ops::{Add, Mul, Sub};

/// `HLab.Geo.Thickness`: four border widths.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Thickness {
    pub left: f64,
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
}

impl Thickness {
    pub const fn new(left: f64, top: f64, right: f64, bottom: f64) -> Self {
        Self {
            left,
            top,
            right,
            bottom,
        }
    }

    pub const fn uniform(length: f64) -> Self {
        Self::new(length, length, length, length)
    }

    pub fn is_uniform(self) -> bool {
        self.left == self.right && self.top == self.bottom && self.right == self.bottom
    }
}

impl Add for Thickness {
    type Output = Thickness;
    fn add(self, b: Thickness) -> Thickness {
        Thickness::new(
            self.left + b.left,
            self.top + b.top,
            self.right + b.right,
            self.bottom + b.bottom,
        )
    }
}

impl Sub for Thickness {
    type Output = Thickness;
    fn sub(self, b: Thickness) -> Thickness {
        Thickness::new(
            self.left - b.left,
            self.top - b.top,
            self.right - b.right,
            self.bottom - b.bottom,
        )
    }
}

impl Mul<f64> for Thickness {
    type Output = Thickness;
    fn mul(self, b: f64) -> Thickness {
        Thickness::new(self.left * b, self.top * b, self.right * b, self.bottom * b)
    }
}
