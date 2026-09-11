//! The rectangle distances of C#'s `MonitorExtensions` that the layout uses.

use crate::geo::dotnet::max;
use crate::geo::{Rect, Thickness, Vector};

/// `Rect.Distance(Rect)`: per side, how far `other` lies beyond `this`, negative
/// when they overlap on that axis. Left is `this.X - other.Right`, right is
/// `other.X - this.Right`, and so on.
pub fn distance(this: Rect, other: Rect) -> Thickness {
    Thickness::new(
        this.x() - other.right(),
        this.y() - other.bottom(),
        other.x() - this.right(),
        other.y() - this.bottom(),
    )
}

/// `Thickness.DistanceHV`: one distance out of four sides. Per axis, the first
/// non-negative side, else the larger (less negative) one; an axis marked
/// infinite (no possible touch) yields the other axis alone; two non-negative
/// axes combine as a diagonal; otherwise the non-negative one, or the larger.
pub fn distance_hv(d: Thickness) -> f64 {
    let x = if d.left >= 0.0 {
        d.left
    } else if d.right >= 0.0 {
        d.right
    } else {
        max(d.left, d.right)
    };
    let y = if d.top >= 0.0 {
        d.top
    } else if d.bottom >= 0.0 {
        d.bottom
    } else {
        max(d.top, d.bottom)
    };
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_rect_is_at_minus_its_size_from_itself() {
        let r = Rect::new(0.0, 0.0, 527.0, 296.0);
        // The single-monitor case of the domain oracle: -296.
        assert_eq!(distance_hv(distance(r, r)), -296.0);
    }

    #[test]
    fn a_gap_on_both_axes_is_a_diagonal() {
        let a = Rect::new(0.0, 0.0, 10.0, 10.0);
        let b = Rect::new(13.0, 14.0, 5.0, 5.0);
        assert_eq!(distance_hv(distance(a, b)), 5.0);
    }

    #[test]
    fn side_by_side_rects_are_at_their_gap() {
        let a = Rect::new(0.0, 0.0, 10.0, 10.0);
        let b = Rect::new(12.0, 2.0, 5.0, 5.0);
        assert_eq!(distance_hv(distance(a, b)), 2.0);
    }
}
