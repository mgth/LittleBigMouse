use std::cmp::{min, max};
use std::vec::Vec;
use crate::line::Line;
use crate::point::Point;

#[derive(Clone)]
pub struct Segment<T> {
    a: Point<T>,
    b: Point<T>
}

impl<T> Segment<T>
where
    T: Copy + std::ops::Sub<Output = T> + std::ops::Add<Output = T> + std::ops::Mul<Output = T> +
    PartialOrd + From<f64> + Into<f64>
{
    pub fn new(a: Point<T>, b: Point<T>) -> Self {
        Self { a, b }
    }

    pub fn a(&self) -> Point<T> {
        self.a
    }

    pub fn b(&self) -> Point<T> {
        self.b
    }

    pub fn to_line(&self) -> Line<T> {
        if self.a.x() == self.b.x() {
            return Line::vertical(self.a.x());
        }

        let slope = (self.a.y().into() - self.b.y().into()) / (self.a.x().into() - self.b.x().into());
        let origin = self.a.y().into() - slope * self.a.x().into();

        Line::new(T::from(slope), T::from(origin))
    }

    pub fn size(&self) -> f64 {
        (self.size_squared().into() as f64).sqrt()
    }

    pub fn size_squared(&self) -> T {
        let w = self.b.x() - self.a.x();
        let h = self.b.y() - self.a.y();
        w * w + h * h
    }

    pub fn intersect(&self, line: &Line<T>) -> Point<T> {
        const EPSILON: f64 = 0.001;

        if let Some(p) = self.to_line().intersect(line) {
            if p.x().into() < (min(self.a.x(), self.b.x())).into() - EPSILON { return Point::empty(); }
            if p.y().into() < (min(self.a.y(), self.b.y())).into() - EPSILON { return Point::empty(); }
            if p.x().into() > (max(self.a.x(), self.b.x())).into() + EPSILON { return Point::empty(); }
            if p.y().into() > (max(self.a.y(), self.b.y())).into() + EPSILON { return Point::empty(); }
            p
        } else {
            Point::empty()
        }
    }

    fn is_outside(n: T, p1: T, p2: T) -> bool {
        const EPSILON: f64 = 0.001;
        let n_f: f64 = n.into();
        let p1_f: f64 = p1.into();
        let p2_f: f64 = p2.into();

        if n_f < p1_f - EPSILON && n_f < p2_f - EPSILON { return true; }
        if n_f > p1_f + EPSILON && n_f > p2_f + EPSILON { return true; }
        false
    }

    pub fn is_intersecting(&self, line: &Line<T>) -> Option<Point<T>> {
        self.to_line().intersect(line).and_then(|p| {
            if Self::is_outside(p.x(), self.a.x(), self.b.x()) ||
                Self::is_outside(p.y(), self.a.y(), self.b.y()) {
                None
            } else {
                Some(p)
            }
        })
    }

    pub fn intersect_list(&self, line: &Line<T>) -> Vec<Point<T>> {
        self.is_intersecting(line)
            .map(|p| vec![p])
            .unwrap_or_default()
    }

    pub fn intersect_segment(&self, other: &Segment<T>) -> Point<T> {
        if let Some(p) = other.is_intersecting(&self.to_line()) {
            const EPSILON: f64 = 0.001;

            if p.x().into() < (min(self.a.x(), self.b.x())).into() - EPSILON { return Point::empty(); }
            if p.y().into() < (min(self.a.y(), self.b.y())).into() - EPSILON { return Point::empty(); }
            if p.x().into() > (max(self.a.x(), self.b.x())).into() + EPSILON { return Point::empty(); }
            if p.y().into() > (max(self.a.y(), self.b.y())).into() + EPSILON { return Point::empty(); }
            p
        } else {
            Point::empty()
        }
    }
}

impl<T> std::ops::Add<Point<T>> for &Segment<T>
where
    T: Copy + std::ops::Add<Output = T>
{
    type Output = Segment<T>;

    fn add(self, rhs: Point<T>) -> Self::Output {
        Segment::new(self.a + rhs, self.b + rhs)
    }
}

impl<T> std::ops::Sub<Point<T>> for &Segment<T>
where
    T: Copy + std::ops::Sub<Output = T>
{
    type Output = Segment<T>;

    fn sub(self, rhs: Point<T>) -> Self::Output {
        Segment::new(self.a - rhs, self.b - rhs)
    }
}
