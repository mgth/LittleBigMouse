use std::ops::{Add, Div, Mul, Neg, Sub};

use super::Point;

/// `HLab.Geo.Vector`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vector {
    pub x: f64,
    pub y: f64,
}

impl Vector {
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    /// `x * x + y * y`, the dot product with itself.
    pub fn length_squared(self) -> f64 {
        self.dot(self)
    }

    pub fn length(self) -> f64 {
        self.length_squared().sqrt()
    }

    pub fn dot(self, other: Vector) -> f64 {
        self.x * other.x + self.y * other.y
    }

    pub fn component_multiply(self, other: Vector) -> Vector {
        Vector::new(self.x * other.x, self.y * other.y)
    }

    pub fn component_divide(self, other: Vector) -> Vector {
        Vector::new(self.x / other.x, self.y / other.y)
    }
}

impl Neg for Vector {
    type Output = Vector;
    fn neg(self) -> Vector {
        Vector::new(-self.x, -self.y)
    }
}

impl Add for Vector {
    type Output = Vector;
    fn add(self, v: Vector) -> Vector {
        Vector::new(self.x + v.x, self.y + v.y)
    }
}

impl Add<Point> for Vector {
    type Output = Point;
    fn add(self, p: Point) -> Point {
        Point::new(p.x + self.x, p.y + self.y)
    }
}

impl Sub for Vector {
    type Output = Vector;
    fn sub(self, v: Vector) -> Vector {
        Vector::new(self.x - v.x, self.y - v.y)
    }
}

impl Mul<f64> for Vector {
    type Output = Vector;
    fn mul(self, s: f64) -> Vector {
        Vector::new(self.x * s, self.y * s)
    }
}

impl Mul<Vector> for f64 {
    type Output = Vector;
    fn mul(self, v: Vector) -> Vector {
        v * self
    }
}

/// HLab.Geo divides by multiplying with the reciprocal: `v * (1.0 / s)`, which
/// is not always the same double as `v.x / s`.
impl Div<f64> for Vector {
    type Output = Vector;
    fn div(self, s: f64) -> Vector {
        self * (1.0 / s)
    }
}
