use std::cmp::{min, max};
use std::fmt;
use crate::line::Line;
use crate::point::Point;
use crate::segment::Segment;

pub struct Rect<T> {
    left: T,
    top: T,
    width: T,
    height: T,
}

impl<T> Rect<T>
where
    T: Copy + PartialOrd + std::ops::Add<Output = T>,
{
    pub fn new(left: T, top: T, width: T, height: T) -> Self {
        Self {
            left,
            top,
            width,
            height,
        }
    }

    pub fn from_points(p1: Point<T>, p2: Point<T>) -> Self {
        Self {
            left: min(p1.x(), p2.x()),
            top: min(p1.y(), p2.y()),
            width: max(p1.x(), p2.x()) - min(p1.x(), p2.x()),
            height: max(p1.y(), p2.y()) - min(p1.y(), p2.y()),
        }
    }

    pub fn empty() -> Self
    where
        T: Default,
    {
        Self::new(T::default(), T::default(), T::default(), T::default())
    }

    pub fn left(&self) -> T {
        self.left
    }

    pub fn top(&self) -> T {
        self.top
    }

    pub fn width(&self) -> T {
        self.width
    }

    pub fn height(&self) -> T {
        self.height
    }

    pub fn right(&self) -> T {
        self.left + self.width
    }

    pub fn bottom(&self) -> T {
        self.top + self.height
    }

    pub fn top_left(&self) -> Point<T> {
        Point::new(self.left(), self.top())
    }

    pub fn top_right(&self) -> Point<T> {
        Point::new(self.right(), self.top())
    }

    pub fn bottom_left(&self) -> Point<T> {
        Point::new(self.left(), self.bottom())
    }

    pub fn bottom_right(&self) -> Point<T> {
        Point::new(self.right(), self.bottom())
    }

    pub fn is_empty(&self) -> bool
    where
        T: PartialEq + Default,
    {
        self.width == T::default() || self.height == T::default()
    }

    pub fn contains(&self, point: &Point<T>) -> bool {
        point.x() >= self.left
            && (point.x() - self.width) < self.left
            && point.y() >= self.top
            && (point.y() - self.height) < self.top
    }

    pub fn intersect(&self, line: &Line<T>) -> Vec<Point<T>>
    where
        T: Clone,
    {
        let mut result = Vec::new();
        let mut p = Point::default();

        if Segment::new(self.top_left(), self.bottom_left()).is_intersecting(line, &mut p) {
            result.push(p.clone());
        }
        if Segment::new(self.top_left(), self.top_right()).is_intersecting(line, &mut p) {
            result.push(p.clone());
        }
        if Segment::new(self.top_right(), self.bottom_right()).is_intersecting(line, &mut p) {
            result.push(p.clone());
        }
        if Segment::new(self.bottom_left(), self.bottom_right()).is_intersecting(line, &mut p) {
            result.push(p.clone());
        }
        result
    }
}

impl<T: PartialEq> PartialEq for Rect<T> {
    fn eq(&self, other: &Self) -> bool {
        self.left == other.left
            && self.top == other.top
            && self.width == other.width
            && self.height == other.height
    }
}

impl<T: fmt::Display> fmt::Display for Rect<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[{} {}, {}]",
            self.top_left(),
            self.width,
            self.height
        )
    }
}
