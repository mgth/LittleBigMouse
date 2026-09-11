//! Port of `geo::Point<T>`.

use std::fmt;
use std::ops::{Add, Sub};

use super::Coord;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point<T> {
    x: T,
    y: T,
}

impl<T: Coord> Point<T> {
    pub fn new(x: T, y: T) -> Self {
        Point { x, y }
    }

    pub fn x(&self) -> T {
        self.x
    }

    pub fn y(&self) -> T {
        self.y
    }

    /// C++ `Point::Empty()` — both coordinates at the type maximum.
    pub fn empty() -> Self {
        Point {
            x: T::MAX,
            y: T::MAX,
        }
    }

    /// C++ `IsEmpty()`.
    pub fn is_empty(&self) -> bool {
        !(self.x < T::MAX || self.y < T::MAX)
    }
}

impl<T: Coord> Default for Point<T> {
    fn default() -> Self {
        Point {
            x: T::ZERO,
            y: T::ZERO,
        }
    }
}

impl<T: Coord> Add for Point<T> {
    type Output = Self;
    fn add(self, o: Self) -> Self {
        Point {
            x: self.x + o.x,
            y: self.y + o.y,
        }
    }
}

impl<T: Coord> Sub for Point<T> {
    type Output = Self;
    fn sub(self, o: Self) -> Self {
        Point {
            x: self.x - o.x,
            y: self.y - o.y,
        }
    }
}

impl<T: fmt::Display> fmt::Display for Point<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{},{}]", self.x, self.y)
    }
}
