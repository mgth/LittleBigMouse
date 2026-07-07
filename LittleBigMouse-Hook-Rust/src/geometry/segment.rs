//! Port of `geo::Segment<T>`.

use super::{cmax, cmin, Coord, Line, Point};

const EPSILON: f64 = 0.001;

#[derive(Debug, Clone, Copy)]
pub struct Segment<T> {
    a: Point<T>,
    b: Point<T>,
}

impl<T: Coord> Segment<T> {
    pub fn new(a: Point<T>, b: Point<T>) -> Self {
        Segment { a, b }
    }

    pub fn a(&self) -> Point<T> {
        self.a
    }

    pub fn b(&self) -> Point<T> {
        self.b
    }

    /// C++ `Segment::Line()`.
    pub fn line(&self) -> Line<T> {
        if self.a.x() == self.b.x() {
            return Line::vertical(self.a.x());
        }
        let slope =
            (self.a.y().to_f64() - self.b.y().to_f64()) / (self.a.x().to_f64() - self.b.x().to_f64());
        let origin = T::from_f64(self.a.y().to_f64() - slope * self.a.x().to_f64());
        Line::new(slope, origin)
    }

    pub fn size_squared(&self) -> T {
        let w = self.b.x() - self.a.x();
        let h = self.b.y() - self.a.y();
        (w * w) + (h * h)
    }

    pub fn size(&self) -> f64 {
        self.size_squared().to_f64().sqrt()
    }

    /// C++ `Segment::OutSide`.
    fn outside(n: T, p1: T, p2: T) -> bool {
        let n = n.to_f64();
        let p1 = p1.to_f64();
        let p2 = p2.to_f64();
        if n < p1 - EPSILON && n < p2 - EPSILON {
            return true;
        }
        if n > p1 + EPSILON && n > p2 + EPSILON {
            return true;
        }
        false
    }

    /// C++ `Segment::IsIntersecting(const Line&, Point&)`. On success writes the
    /// clamped intersection into `point`.
    pub fn is_intersecting(&self, line: &Line<T>, point: &mut Point<T>) -> bool {
        let mut p = Point::empty();
        if self.line().is_intersecting(line, &mut p) {
            if Self::outside(p.x(), self.a.x(), self.b.x())
                || Self::outside(p.y(), self.a.y(), self.b.y())
            {
                *point = Point::empty();
                return false;
            }
            *point = p;
            return true;
        }
        *point = Point::empty();
        false
    }

    /// C++ `Segment::IsIntersecting(const Segment&, Point&)` — does segment `s`
    /// cross this segment's line within this segment's extent?
    pub fn is_intersecting_segment(&self, s: &Segment<T>, point: &mut Point<T>) -> bool {
        let mut p = Point::empty();
        if s.is_intersecting(&self.line(), &mut p) {
            if Self::outside(p.x(), self.a.x(), self.b.x())
                || Self::outside(p.y(), self.a.y(), self.b.y())
            {
                *point = Point::empty();
                return false;
            }
            *point = p;
            return true;
        }
        *point = Point::empty();
        false
    }

    /// C++ `Segment::Intersect(const Line&)` — intersection clamped to this
    /// segment's bounds (± epsilon), else empty.
    pub fn intersect(&self, line: &Line<T>) -> Point<T> {
        let mut p = Point::default();
        if self.line().is_intersecting(line, &mut p) {
            if p.x().to_f64() < cmin(self.a.x(), self.b.x()).to_f64() - EPSILON {
                return Point::empty();
            }
            if p.y().to_f64() < cmin(self.a.y(), self.b.y()).to_f64() - EPSILON {
                return Point::empty();
            }
            if p.x().to_f64() > cmax(self.a.x(), self.b.x()).to_f64() + EPSILON {
                return Point::empty();
            }
            if p.y().to_f64() > cmax(self.a.y(), self.b.y()).to_f64() + EPSILON {
                return Point::empty();
            }
        }
        p
    }
}

impl<T: Coord> std::ops::Add<Point<T>> for Segment<T> {
    type Output = Segment<T>;
    fn add(self, p: Point<T>) -> Segment<T> {
        Segment::new(self.a + p, self.b + p)
    }
}

impl<T: Coord> std::ops::Sub<Point<T>> for Segment<T> {
    type Output = Segment<T>;
    fn sub(self, p: Point<T>) -> Segment<T> {
        Segment::new(self.a - p, self.b - p)
    }
}
