//! Port of `geo::Rect<T>`.

use super::{cmax, cmin, Coord, Line, Point, Segment};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect<T> {
    left: T,
    top: T,
    width: T,
    height: T,
}

impl<T: Coord> Rect<T> {
    pub fn new(left: T, top: T, width: T, height: T) -> Self {
        Rect {
            left,
            top,
            width,
            height,
        }
    }

    /// C++ `Rect(Point p1, Point p2)`.
    pub fn from_points(p1: Point<T>, p2: Point<T>) -> Self {
        let left = cmin(p1.x(), p2.x());
        let top = cmin(p1.y(), p2.y());
        Rect {
            left,
            top,
            width: cmax(p1.x(), p2.x()) - left,
            height: cmax(p1.y(), p2.y()) - top,
        }
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

    pub fn empty() -> Self {
        Rect::new(T::ZERO, T::ZERO, T::ZERO, T::ZERO)
    }

    pub fn is_empty(&self) -> bool {
        self.width == T::ZERO || self.height == T::ZERO
    }

    /// C++ `Rect::Contains`. The `point.x - width < left` form is the C++
    /// overflow-avoiding equivalent of `point.x < right`.
    pub fn contains(&self, point: Point<T>) -> bool {
        point.x() >= self.left
            && point.x() - self.width < self.left
            && point.y() >= self.top
            && point.y() - self.height < self.top
    }

    /// C++ `Rect::Intersect(const Line&)` — intersection points with the four edges.
    pub fn intersect(&self, l: &Line<T>) -> Vec<Point<T>> {
        let mut result = Vec::new();
        let mut p = Point::empty();
        if Segment::new(self.top_left(), self.bottom_left()).is_intersecting(l, &mut p) {
            result.push(p);
        }
        if Segment::new(self.top_left(), self.top_right()).is_intersecting(l, &mut p) {
            result.push(p);
        }
        if Segment::new(self.top_right(), self.bottom_right()).is_intersecting(l, &mut p) {
            result.push(p);
        }
        if Segment::new(self.bottom_left(), self.bottom_right()).is_intersecting(l, &mut p) {
            result.push(p);
        }
        result
    }
}
