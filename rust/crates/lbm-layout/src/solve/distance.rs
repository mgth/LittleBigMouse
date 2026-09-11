//! The members of `MonitorExtensions` (`Monitors/MonitorExtensions.cs`) the solvers
//! use: distances between rect borders, and the ways of reading a `Thickness` of
//! such distances.
//!
//! `MonitorExtensions.Distance(Rect, IEnumerable<Rect>)` is not ported: no solver
//! calls it.

use crate::geo::dotnet::{enumerable_min, max, min};
use crate::geo::{Rect, Thickness, Vector};

/// `MonitorExtensions.Infinity`: positive infinity on all four sides, the
/// "cannot touch" answer of [`RectDistance::distance_to_touch`].
pub const INFINITY: Thickness = Thickness::uniform(f64::INFINITY);

/// The extension methods `MonitorExtensions` adds to `Rect`.
pub trait RectDistance {
    /// `MonitorExtensions.Distance(Rect, Rect)`: from each border of this rect to
    /// the opposite border of `other` — `X - other.Right` on the left,
    /// `other.X - Right` on the right. Negative where they overlap.
    fn distance(&self, other: &Rect) -> Thickness;

    /// `MonitorExtensions.DistanceToTouch(Rect, Rect, bool)`: the distances along
    /// the one axis a single translation can make the two touch on, the other axis
    /// set to infinity; [`INFINITY`] when neither can. `zero` counts an exactly
    /// zero distance as "apart" for that decision.
    fn distance_to_touch(&self, other: &Rect, zero: bool) -> Thickness;

    /// `MonitorExtensions.DistanceToTouch(Rect, IEnumerable<Rect>, bool)`: the
    /// side-by-side `Math.Min` of [`distance_to_touch`](Self::distance_to_touch)
    /// over `others`, starting from [`INFINITY`].
    fn distance_to_touch_all<'a>(
        &self,
        others: impl IntoIterator<Item = &'a Rect>,
        zero: bool,
    ) -> Thickness;
}

impl RectDistance for Rect {
    fn distance(&self, other: &Rect) -> Thickness {
        Thickness::new(
            self.x() - other.right(),
            self.y() - other.bottom(),
            other.x() - self.right(),
            other.y() - self.bottom(),
        )
    }

    fn distance_to_touch(&self, other: &Rect, zero: bool) -> Thickness {
        let distance = self.distance(other);
        if distance.top > 0.0
            || distance.bottom > 0.0
            || zero && (distance.top == 0.0 || distance.bottom == 0.0)
        {
            if distance.left > 0.0
                || distance.right > 0.0
                || zero && (distance.left == 0.0 || distance.right == 0.0)
            {
                return INFINITY;
            }
            return Thickness::new(f64::INFINITY, distance.top, f64::INFINITY, distance.bottom);
        }
        if distance.left > 0.0
            || distance.right > 0.0
            || zero && (distance.left == 0.0 || distance.right == 0.0)
        {
            return Thickness::new(distance.left, f64::INFINITY, distance.right, f64::INFINITY);
        }

        distance
    }

    fn distance_to_touch_all<'a>(
        &self,
        others: impl IntoIterator<Item = &'a Rect>,
        zero: bool,
    ) -> Thickness {
        let mut min = Thickness::uniform(f64::INFINITY);

        for other in others {
            min = min.min(self.distance_to_touch(other, zero));
        }
        min
    }
}

/// The extension methods `MonitorExtensions` adds to `Thickness`.
pub trait ThicknessDistance {
    /// `MonitorExtensions.IsPositiveInfinity`: all four sides positive infinity.
    fn is_positive_infinity(&self) -> bool;

    /// `MonitorExtensions.Min(Thickness, Thickness)`: side by side `Math.Min`, so
    /// a NaN side propagates.
    fn min(&self, other: Thickness) -> Thickness;

    /// `MonitorExtensions.DistanceHV`: one distance out of four. Per axis, the
    /// first non-negative of the two sides, else the larger (least negative) one;
    /// an infinite axis ("cannot touch that way") yields the other axis alone; two
    /// non-negative axes combine as a vector length.
    fn distance_hv(&self) -> f64;

    /// `MonitorExtensions.MinPositive`: the smallest non-negative side (LINQ `Min`,
    /// so of -0 and +0 the first), positive infinity when there is none.
    fn min_positive(&self) -> f64;

    /// `MonitorExtensions.ToArray`: left, top, right, bottom.
    fn to_array(&self) -> [f64; 4];
}

impl ThicknessDistance for Thickness {
    fn is_positive_infinity(&self) -> bool {
        self.left == f64::INFINITY
            && self.top == f64::INFINITY
            && self.right == f64::INFINITY
            && self.bottom == f64::INFINITY
    }

    fn min(&self, other: Thickness) -> Thickness {
        Thickness::new(
            min(self.left, other.left),
            min(self.top, other.top),
            min(self.right, other.right),
            min(self.bottom, other.bottom),
        )
    }

    fn distance_hv(&self) -> f64 {
        let x = if self.left >= 0.0 {
            self.left
        } else if self.right >= 0.0 {
            self.right
        } else {
            max(self.left, self.right)
        };
        let y = if self.top >= 0.0 {
            self.top
        } else if self.bottom >= 0.0 {
            self.bottom
        } else {
            max(self.top, self.bottom)
        };

        // DistanceToTouch marks an axis with no possible touch as infinite: use
        // the finite axis alone, or ordering by distance degenerates (#450).
        if x == f64::INFINITY {
            return y;
        }
        if y == f64::INFINITY {
            return x;
        }

        let v = Vector::new(x, y);

        if v.x >= 0.0 && v.y >= 0.0 {
            return v.length();
        }

        if v.x >= 0.0 {
            return v.x;
        }
        if v.y >= 0.0 {
            return v.y;
        }

        max(v.x, v.y)
    }

    fn min_positive(&self) -> f64 {
        enumerable_min(self.to_array().into_iter().filter(|d| *d >= 0.0)).unwrap_or(f64::INFINITY)
    }

    fn to_array(&self) -> [f64; 4] {
        [self.left, self.top, self.right, self.bottom]
    }
}
