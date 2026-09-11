use super::{Point, Size, Vector};

/// `HLab.Geo.Rect`: a location and a size, in doubles.
///
/// The empty rectangle is the only one with a negative size: X and Y positive
/// infinity, width and height negative infinity. Its right and bottom read as
/// negative infinity.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

impl Rect {
    pub const EMPTY: Rect = Rect {
        x: f64::INFINITY,
        y: f64::INFINITY,
        width: f64::NEG_INFINITY,
        height: f64::NEG_INFINITY,
    };

    pub fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        debug_assert!(
            !(width < 0.0 || height < 0.0),
            "HLab.Geo rejects a negative rect size ({width} x {height})"
        );
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// `new Rect(Point location, Size size)`: empty when the size is.
    pub fn from_location_size(location: Point, size: Size) -> Self {
        if size.is_empty() {
            Self::EMPTY
        } else {
            Self::new(location.x, location.y, size.width(), size.height())
        }
    }

    /// `new Rect(Size size)`: at the origin, empty when the size is.
    pub fn from_size(size: Size) -> Self {
        Self::from_location_size(Point::new(0.0, 0.0), size)
    }

    /// `new Rect(Point, Point)`: the bounds of two points. The width is clamped
    /// to zero "to prevent double weirdness from causing us to be (-epsilon..0)".
    pub fn from_points(p1: Point, p2: Point) -> Self {
        let x = p1.x.min(p2.x);
        let y = p1.y.min(p2.y);
        Self::new(
            x,
            y,
            (p1.x.max(p2.x) - x).max(0.0),
            (p1.y.max(p2.y) - y).max(0.0),
        )
    }

    /// `width < 0`: a NaN width is not empty.
    pub fn is_empty(&self) -> bool {
        self.width < 0.0
    }

    pub fn x(&self) -> f64 {
        self.x
    }

    pub fn y(&self) -> f64 {
        self.y
    }

    pub fn width(&self) -> f64 {
        self.width
    }

    pub fn height(&self) -> f64 {
        self.height
    }

    pub fn left(&self) -> f64 {
        self.x
    }

    pub fn top(&self) -> f64 {
        self.y
    }

    pub fn right(&self) -> f64 {
        if self.is_empty() {
            f64::NEG_INFINITY
        } else {
            self.x + self.width
        }
    }

    pub fn bottom(&self) -> f64 {
        if self.is_empty() {
            f64::NEG_INFINITY
        } else {
            self.y + self.height
        }
    }

    pub fn location(&self) -> Point {
        Point::new(self.x, self.y)
    }

    pub fn size(&self) -> Size {
        if self.is_empty() {
            Size::EMPTY
        } else {
            Size::new(self.width, self.height)
        }
    }

    pub fn top_left(&self) -> Point {
        Point::new(self.left(), self.top())
    }

    pub fn top_right(&self) -> Point {
        Point::new(self.right(), self.top())
    }

    pub fn bottom_left(&self) -> Point {
        Point::new(self.left(), self.bottom())
    }

    pub fn bottom_right(&self) -> Point {
        Point::new(self.right(), self.bottom())
    }

    /// Inclusive of the edges. Written `x - width <= X` rather than
    /// `x <= X + width`, so an infinite width against an infinite X still works.
    pub fn contains_point(&self, p: Point) -> bool {
        !self.is_empty()
            && p.x >= self.x
            && p.x - self.width <= self.x
            && p.y >= self.y
            && p.y - self.height <= self.y
    }

    /// Non-empty and entirely inside, edges included.
    pub fn contains_rect(&self, r: &Rect) -> bool {
        if self.is_empty() || r.is_empty() {
            return false;
        }
        self.x <= r.x
            && self.y <= r.y
            && self.x + self.width >= r.x + r.width
            && self.y + self.height >= r.y + r.height
    }

    /// A shared edge counts as an intersection.
    pub fn intersects_with(&self, r: &Rect) -> bool {
        if self.is_empty() || r.is_empty() {
            return false;
        }
        r.left() <= self.right()
            && r.right() >= self.left()
            && r.top() <= self.bottom()
            && r.bottom() >= self.top()
    }

    /// `Rect.Intersect`: empty when the two do not intersect.
    pub fn intersect(&self, r: &Rect) -> Rect {
        if !self.intersects_with(r) {
            return Rect::EMPTY;
        }
        let left = self.left().max(r.left());
        let top = self.top().max(r.top());
        Rect {
            x: left,
            y: top,
            width: (self.right().min(r.right()) - left).max(0.0),
            height: (self.bottom().min(r.bottom()) - top).max(0.0),
        }
    }

    /// The instance `Rect.Union(Rect)` of HLab.Geo, which is **not** WPF's: when
    /// either rectangle is empty the result is empty, where WPF would return the
    /// other one. The domain folds bounds with it (`MonitorsLayout.PhysicalBounds`),
    /// so a layout holding one empty rectangle has empty bounds.
    ///
    /// (HLab.Geo's static `Rect.Union(a, b)` drops its result and returns `a`;
    /// nothing in the domain calls it, so it is not ported.)
    pub fn union(&self, r: &Rect) -> Rect {
        if self.is_empty() {
            return *self;
        }
        if r.is_empty() {
            return *r;
        }
        let left = self.left().min(r.left());
        let top = self.top().min(r.top());
        // "We need this check so that the math does not result in NaN."
        let width = if r.width == f64::INFINITY || self.width == f64::INFINITY {
            f64::INFINITY
        } else {
            (self.right().max(r.right()) - left).max(0.0)
        };
        let height = if r.height == f64::INFINITY || self.height == f64::INFINITY {
            f64::INFINITY
        } else {
            (self.bottom().max(r.bottom()) - top).max(0.0)
        };
        Rect::new(left, top, width, height)
    }

    /// `Rect.Offset`. C# throws on the empty rectangle; it is returned unchanged.
    pub fn offset(&self, v: Vector) -> Rect {
        debug_assert!(!self.is_empty(), "HLab.Geo cannot offset the empty rect");
        if self.is_empty() {
            return *self;
        }
        Rect {
            x: self.x + v.x,
            y: self.y + v.y,
            ..*self
        }
    }
}

impl Default for Rect {
    fn default() -> Self {
        Rect::new(0.0, 0.0, 0.0, 0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(x: f64, y: f64, w: f64, h: f64) -> Rect {
        Rect::new(x, y, w, h)
    }

    #[test]
    fn empty_reads_like_hlab() {
        let e = Rect::EMPTY;
        assert!(e.is_empty());
        assert_eq!(e.left(), f64::INFINITY);
        assert_eq!(e.top(), f64::INFINITY);
        assert_eq!(e.right(), f64::NEG_INFINITY);
        assert_eq!(e.bottom(), f64::NEG_INFINITY);
        assert!(e.size().is_empty());
        assert!(!r(0.0, 0.0, 0.0, 0.0).is_empty());
    }

    #[test]
    fn union_of_two_rects_bounds_both() {
        let u = r(0.0, 0.0, 10.0, 10.0).union(&r(20.0, -5.0, 5.0, 5.0));
        assert_eq!(u, r(0.0, -5.0, 25.0, 15.0));
    }

    #[test]
    fn union_with_an_empty_rect_is_empty_unlike_wpf() {
        let a = r(0.0, 0.0, 10.0, 10.0);
        assert!(a.union(&Rect::EMPTY).is_empty());
        assert!(Rect::EMPTY.union(&a).is_empty());
    }

    #[test]
    fn union_keeps_an_infinite_extent() {
        let u = r(0.0, 0.0, f64::INFINITY, 1.0).union(&r(5.0, 5.0, 1.0, 1.0));
        assert_eq!(u.width(), f64::INFINITY);
        assert_eq!(u.height(), 6.0);
    }

    #[test]
    fn contains_is_inclusive_of_edges() {
        let a = r(0.0, 0.0, 10.0, 10.0);
        assert!(a.contains_point(Point::new(10.0, 10.0)));
        assert!(a.contains_point(Point::new(0.0, 0.0)));
        assert!(!a.contains_point(Point::new(10.000001, 5.0)));
        assert!(!Rect::EMPTY.contains_point(Point::new(0.0, 0.0)));
        assert!(a.contains_rect(&a));
    }

    #[test]
    fn touching_rects_intersect_in_a_line() {
        let a = r(0.0, 0.0, 10.0, 10.0);
        let b = r(10.0, 0.0, 10.0, 10.0);
        assert!(a.intersects_with(&b));
        assert_eq!(a.intersect(&b), r(10.0, 0.0, 0.0, 10.0));
        assert!(a.intersect(&r(11.0, 0.0, 1.0, 1.0)).is_empty());
    }

    #[test]
    fn from_points_orders_the_corners() {
        let a = Rect::from_points(Point::new(5.0, 7.0), Point::new(1.0, 2.0));
        assert_eq!(a, r(1.0, 2.0, 4.0, 5.0));
    }
}
