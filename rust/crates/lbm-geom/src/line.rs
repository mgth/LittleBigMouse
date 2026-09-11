//! Port of `geo::Line<T>` — `Y = slope * X + origin`. The slope is always `f64`;
//! a vertical line is encoded with `slope == f64::MAX` (C++ `DBL_MAX` sentinel).

use super::{Coord, Point};

#[derive(Debug, Clone, Copy)]
pub struct Line<T> {
    slope: f64,
    /// `y` for `x == 0` (or, for a vertical line, the constant `x`).
    origin: T,
}

impl<T: Coord> Line<T> {
    pub fn new(slope: f64, y_for_x0: T) -> Self {
        Line {
            slope,
            origin: y_for_x0,
        }
    }

    /// C++ `Line(T x)` — the vertical line `x == origin`.
    pub fn vertical(x: T) -> Self {
        Line {
            slope: f64::MAX,
            origin: x,
        }
    }

    // Faithful to the C++ `!(_slope < DBL_MAX)`: this also treats a NaN slope as
    // vertical, which `slope >= f64::MAX` would not.
    #[allow(clippy::neg_cmp_op_on_partial_ord)]
    pub fn is_vertical(&self) -> bool {
        !(self.slope < f64::MAX)
    }

    pub fn slope(&self) -> f64 {
        self.slope
    }

    pub fn x_at_y0(&self) -> T {
        if self.is_vertical() {
            return self.origin;
        }
        T::from_f64(self.origin.to_f64() / self.slope)
    }

    pub fn y_at_x0(&self) -> T {
        if self.is_vertical() {
            return T::from_f64(0.0);
        }
        self.origin
    }

    pub fn x(&self, y: T) -> T {
        if self.slope == 0.0 {
            return T::from_f64(0.0); // Error
        }
        if self.is_vertical() {
            return self.origin;
        }
        T::from_f64(self.origin.to_f64() - y.to_f64() / self.slope)
    }

    pub fn y(&self, x: T) -> T {
        if self.slope == 0.0 {
            return self.origin;
        }
        if self.is_vertical() {
            return T::from_f64(0.0); // Error
        }
        T::from_f64(self.slope * x.to_f64() + self.origin.to_f64())
    }

    pub fn origin_point(&self) -> Point<T> {
        Point::new(T::from_f64(0.0), self.y_at_x0())
    }

    /// C++ `Line::IsIntersecting`. On success writes the intersection into `p`.
    pub fn is_intersecting(&self, l: &Line<T>, p: &mut Point<T>) -> bool {
        // Parallel lines.
        if self.slope == l.slope {
            if self.origin == l.origin {
                *p = self.origin_point();
                return true;
            }
            return false;
        }

        let (x, y): (T, T);
        if self.is_vertical() {
            x = self.origin;
            y = l.y(x);
        } else if l.is_vertical() {
            x = l.origin;
            y = self.y(x);
        } else {
            x = T::from_f64((self.origin.to_f64() - l.origin.to_f64()) / (l.slope - self.slope));
            y = self.y(x);
        }

        *p = Point::new(x, y);
        true
    }
}
