use std::ops::{Add, Sub};

use super::Vector;

/// `HLab.Geo.Point`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

impl Point {
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    pub fn offset(self, dx: f64, dy: f64) -> Self {
        Self::new(self.x + dx, self.y + dy)
    }

    pub fn with_x(self, x: f64) -> Self {
        Self::new(x, self.y)
    }

    pub fn with_y(self, y: f64) -> Self {
        Self::new(self.x, y)
    }
}

impl Add<Vector> for Point {
    type Output = Point;
    fn add(self, v: Vector) -> Point {
        Point::new(self.x + v.x, self.y + v.y)
    }
}

/// HLab.Geo also adds two points, component-wise.
impl Add<Point> for Point {
    type Output = Point;
    fn add(self, p: Point) -> Point {
        Point::new(self.x + p.x, self.y + p.y)
    }
}

impl Sub<Vector> for Point {
    type Output = Point;
    fn sub(self, v: Vector) -> Point {
        Point::new(self.x - v.x, self.y - v.y)
    }
}

impl Sub<Point> for Point {
    type Output = Vector;
    fn sub(self, p: Point) -> Vector {
        Vector::new(self.x - p.x, self.y - p.y)
    }
}
