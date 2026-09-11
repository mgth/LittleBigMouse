use crate::geo::dotnet::max;
use crate::geo::{Point, Rect, Size, Thickness, Vector};

use super::Ratio;

/// A content rectangle and four borders: what C#'s `IDisplaySize` exposes.
///
/// In C# every size is a node of a reactive chain (`DisplaySizeInMm` →
/// `DisplayRotate` → `DisplayScale` → `DisplayLocate` …), each node deriving
/// its values from its source. Here a size is a value and each node is a
/// method returning a new one; the formulas are the C# ones, in the same order.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DisplaySize {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub left_border: f64,
    pub top_border: f64,
    pub right_border: f64,
    pub bottom_border: f64,
}

impl DisplaySize {
    /// A rectangle without borders, which is what `DisplaySizeInPixels` is.
    pub fn from_rect(rect: Rect) -> Self {
        Self {
            x: rect.x(),
            y: rect.y(),
            width: rect.width(),
            height: rect.height(),
            left_border: 0.0,
            top_border: 0.0,
            right_border: 0.0,
            bottom_border: 0.0,
        }
    }

    pub fn borders(&self) -> Thickness {
        Thickness::new(
            self.left_border,
            self.top_border,
            self.right_border,
            self.bottom_border,
        )
    }

    pub fn location(&self) -> Point {
        Point::new(self.x, self.y)
    }

    pub fn size(&self) -> Size {
        Size::new(self.width, self.height)
    }

    /// `Location + new Vector(Width / 2, Height / 2)`.
    pub fn center(&self) -> Point {
        self.location() + Vector::new(self.width / 2.0, self.height / 2.0)
    }

    /// `new Rect(Location, Size)`.
    pub fn bounds(&self) -> Rect {
        Rect::from_location_size(self.location(), self.size())
    }

    /// `X - LeftBorder`.
    pub fn outside_x(&self) -> f64 {
        self.x - self.left_border
    }

    /// `Y - TopBorder`.
    pub fn outside_y(&self) -> f64 {
        self.y - self.top_border
    }

    /// `LeftBorder + Width + RightBorder`, left to right.
    pub fn outside_width(&self) -> f64 {
        self.left_border + self.width + self.right_border
    }

    /// `TopBorder + Height + BottomBorder`, top to bottom.
    pub fn outside_height(&self) -> f64 {
        self.top_border + self.height + self.bottom_border
    }

    /// `new Rect(OutsideX, OutsideY, OutsideWidth, OutsideHeight)`.
    pub fn outside_bounds(&self) -> Rect {
        Rect::new(
            self.outside_x(),
            self.outside_y(),
            self.outside_width(),
            self.outside_height(),
        )
    }

    /// `DisplayRotate`: `rotation` quarter turns. Width and height swap on odd
    /// turns (`r % 2 == 0 ? width : height`, `r % 2 == 1 ? width : height`, so a
    /// negative odd rotation keeps the height on both axes, as in C#), and each
    /// border takes the source border `(border + rotation) % 4` counted
    /// top, right, bottom, left — or -1 when that remainder is negative.
    pub fn rotate(self, rotation: i32) -> DisplaySize {
        let width = if rotation % 2 == 0 {
            self.width
        } else {
            self.height
        };
        let height = if rotation % 2 == 1 {
            self.width
        } else {
            self.height
        };
        let border = |index: i32| match (index + rotation) % 4 {
            0 => self.top_border,
            1 => self.right_border,
            2 => self.bottom_border,
            3 => self.left_border,
            _ => -1.0,
        };
        DisplaySize {
            x: self.x,
            y: self.y,
            width,
            height,
            top_border: border(0),
            right_border: border(1),
            bottom_border: border(2),
            left_border: border(3),
        }
    }

    /// `DisplayScale`: widths and left/right borders times `ratio.x`, heights
    /// and top/bottom borders times `ratio.y`. The location is not scaled.
    pub fn scale(self, ratio: Ratio) -> DisplaySize {
        DisplaySize {
            x: self.x,
            y: self.y,
            width: self.width * ratio.x,
            height: self.height * ratio.y,
            left_border: self.left_border * ratio.x,
            top_border: self.top_border * ratio.y,
            right_border: self.right_border * ratio.x,
            bottom_border: self.bottom_border * ratio.y,
        }
    }

    /// `DisplayLocate`: the same size at a location of its own.
    pub fn located(self, location: Point) -> DisplaySize {
        DisplaySize {
            x: location.x,
            y: location.y,
            ..self
        }
    }

    /// `DisplayBorderOverride`: the same rectangle with borders taken elsewhere.
    pub fn with_borders(self, borders: Thickness) -> DisplaySize {
        DisplaySize {
            left_border: borders.left,
            top_border: borders.top,
            right_border: borders.right,
            bottom_border: borders.bottom,
            ..self
        }
    }

    /// `DisplayScaleDip` over a pixel rectangle: everything, the location
    /// included, times `96 / effectiveDpi` per axis.
    pub fn scale_dip(self, effective_dpi: Ratio) -> DisplaySize {
        let rx = 96.0 / effective_dpi.x;
        let ry = 96.0 / effective_dpi.y;
        DisplaySize {
            x: self.x * rx,
            y: self.y * ry,
            width: self.width * rx,
            height: self.height * ry,
            left_border: self.left_border * rx,
            top_border: self.top_border * ry,
            right_border: self.right_border * rx,
            bottom_border: self.bottom_border * ry,
        }
    }
}

/// A monitor model's physical size in millimetres: C#'s `DisplaySizeInMm`,
/// the one mutable root every monitor geometry derives from.
///
/// Borders default to 20 mm and never go below zero. The width and height
/// setters honour `FixedAspectRatio` by scaling the other dimension, and store
/// the value they were given, not the one they clamped — C# does exactly that,
/// so a negative width is kept as is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MmSize {
    width: f64,
    height: f64,
    x: f64,
    y: f64,
    left_border: f64,
    top_border: f64,
    right_border: f64,
    bottom_border: f64,
    fixed_aspect_ratio: bool,
}

impl Default for MmSize {
    fn default() -> Self {
        Self {
            width: 0.0,
            height: 0.0,
            x: 0.0,
            y: 0.0,
            left_border: 20.0,
            top_border: 20.0,
            right_border: 20.0,
            bottom_border: 20.0,
            fixed_aspect_ratio: false,
        }
    }
}

impl MmSize {
    pub fn as_display_size(&self) -> DisplaySize {
        DisplaySize {
            x: self.x,
            y: self.y,
            width: self.width,
            height: self.height,
            left_border: self.left_border,
            top_border: self.top_border,
            right_border: self.right_border,
            bottom_border: self.bottom_border,
        }
    }

    pub fn width(&self) -> f64 {
        self.width
    }

    pub fn height(&self) -> f64 {
        self.height
    }

    pub fn borders(&self) -> Thickness {
        self.as_display_size().borders()
    }

    pub fn fixed_aspect_ratio(&self) -> bool {
        self.fixed_aspect_ratio
    }

    /// Returns whether the value changed. In C# this one is a plain
    /// `RaiseAndSetIfChanged`: it never marks the size unsaved.
    pub fn set_fixed_aspect_ratio(&mut self, value: bool) -> bool {
        let changed = self.fixed_aspect_ratio != value;
        self.fixed_aspect_ratio = value;
        changed
    }

    /// `DisplaySizeInMm.Width`'s setter. With a fixed aspect ratio the height is
    /// scaled by `max(value, 0) / old width` first. Returns whether anything
    /// changed.
    pub fn set_width(&mut self, value: f64) -> bool {
        let mut changed = false;
        if self.fixed_aspect_ratio {
            let ratio = max(value, 0.0) / self.width;
            self.fixed_aspect_ratio = false;
            changed |= self.set_height(self.height * ratio);
            self.fixed_aspect_ratio = true;
        }
        changed | set_if_changed(&mut self.width, value)
    }

    /// `DisplaySizeInMm.Height`'s setter, the mirror of [`MmSize::set_width`].
    pub fn set_height(&mut self, value: f64) -> bool {
        let mut changed = false;
        if self.fixed_aspect_ratio {
            let ratio = max(value, 0.0) / self.height;
            self.fixed_aspect_ratio = false;
            changed |= self.set_width(self.width * ratio);
            self.fixed_aspect_ratio = true;
        }
        changed | set_if_changed(&mut self.height, value)
    }

    pub fn set_left_border(&mut self, value: f64) -> bool {
        set_if_changed(&mut self.left_border, max(value, 0.0))
    }

    pub fn set_top_border(&mut self, value: f64) -> bool {
        set_if_changed(&mut self.top_border, max(value, 0.0))
    }

    pub fn set_right_border(&mut self, value: f64) -> bool {
        set_if_changed(&mut self.right_border, max(value, 0.0))
    }

    pub fn set_bottom_border(&mut self, value: f64) -> bool {
        set_if_changed(&mut self.bottom_border, max(value, 0.0))
    }
}

/// `SetUnsavedValue`'s comparison: `EqualityComparer<double>.Default`, for which
/// NaN equals NaN and -0 equals +0. Returns whether the field changed.
pub(crate) fn set_if_changed(field: &mut f64, value: f64) -> bool {
    if dotnet_equals(*field, value) {
        return false;
    }
    *field = value;
    true
}

/// `double.Equals(double)`: `==`, except that NaN equals NaN.
pub(crate) fn dotnet_equals(a: f64, b: f64) -> bool {
    a == b || (a.is_nan() && b.is_nan())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn size(w: f64, h: f64, l: f64, t: f64, r: f64, b: f64) -> DisplaySize {
        DisplaySize {
            x: 1.0,
            y: 2.0,
            width: w,
            height: h,
            left_border: l,
            top_border: t,
            right_border: r,
            bottom_border: b,
        }
    }

    #[test]
    fn a_quarter_turn_swaps_the_axes_and_moves_each_border_one_side() {
        let s = size(600.0, 340.0, 1.0, 2.0, 3.0, 4.0).rotate(1);
        assert_eq!((s.width, s.height), (340.0, 600.0));
        // top <- right, right <- bottom, bottom <- left, left <- top
        assert_eq!(s.top_border, 3.0);
        assert_eq!(s.right_border, 4.0);
        assert_eq!(s.bottom_border, 1.0);
        assert_eq!(s.left_border, 2.0);
        assert_eq!((s.x, s.y), (1.0, 2.0));
    }

    #[test]
    fn a_negative_rotation_reads_like_csharp() {
        // (border + r) % 4 is negative for r = -1 and the first borders: -1.
        let s = size(600.0, 340.0, 1.0, 2.0, 3.0, 4.0).rotate(-1);
        assert_eq!(s.top_border, -1.0);
        assert_eq!(s.right_border, 2.0);
        // r % 2 == -1: neither branch swaps, height stays on both axes.
        assert_eq!((s.width, s.height), (340.0, 340.0));
    }

    #[test]
    fn outside_bounds_add_the_borders_around_the_content() {
        let s = size(600.0, 340.0, 10.0, 20.0, 30.0, 40.0);
        assert_eq!(s.outside_bounds(), Rect::new(-9.0, -18.0, 640.0, 400.0));
        assert_eq!(s.bounds(), Rect::new(1.0, 2.0, 600.0, 340.0));
    }

    #[test]
    fn a_fixed_ratio_width_edit_scales_the_height() {
        let mut mm = MmSize::default();
        mm.set_width(600.0);
        mm.set_height(300.0);
        mm.set_fixed_aspect_ratio(true);
        mm.set_width(1200.0);
        assert_eq!((mm.width(), mm.height()), (1200.0, 600.0));
        assert!(mm.fixed_aspect_ratio());
    }

    #[test]
    fn borders_never_go_below_zero_but_a_negative_width_is_kept() {
        let mut mm = MmSize::default();
        mm.set_left_border(-5.0);
        assert_eq!(mm.borders().left, 0.0);
        mm.set_width(-3.0);
        assert_eq!(mm.width(), -3.0);
    }

    #[test]
    fn setters_report_whether_the_value_changed_nan_included() {
        let mut mm = MmSize::default();
        assert!(mm.set_width(f64::NAN));
        assert!(!mm.set_width(f64::NAN));
        assert!(!mm.set_top_border(20.0));
    }
}
