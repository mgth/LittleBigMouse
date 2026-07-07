use std::fmt;
use std::ops::{Add, Sub};
use std::num::Bounded;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Point<T> {
    x: T,
    y: T,
}

impl<T> Point<T> {
    pub fn new(x: T, y: T) -> Self {
        Point { x, y }
    }

    pub fn x(&self) -> &T {
        &self.x
    }

    pub fn y(&self) -> &T {
        &self.y
    }
}

impl<T> Default for Point<T>
where
    T: Default,
{
    fn default() -> Self {
        Point {
            x: T::default(),
            y: T::default(),
        }
    }
}

impl<T> Add for Point<T>
where
    T: Add<Output = T>,
{
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Point {
            x: self.x + other.x,
            y: self.y + other.y,
        }
    }
}

impl<T> Sub for Point<T>
where
    T: Sub<Output = T>,
{
    type Output = Self;

    fn sub(self, other: Self) -> Self {
        Point {
            x: self.x - other.x,
            y: self.y - other.y,
        }
    }
}

impl<T> Point<T>
where
    T: Ord + Bounded,
{
    pub fn empty() -> Self {
        Point {
            x: T::max_value(),
            y: T::max_value(),
        }
    }

    pub fn is_empty(&self) -> bool {
        !(self.x < T::max_value() || self.y < T::max_value())
    }
}

impl<T: fmt::Display> fmt::Display for Point<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{},{}]", self.x, self.y)
    }
}
