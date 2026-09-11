//! `LayoutGeometry.cs`: the geometry both placement directions share (axes, spans,
//! edge contact) and `MonitorSnapshot`, the monitor as the solvers see it. The
//! `EdgeProjection` class of the same file is [`super::edge_projection`].

use super::CompactionMonitor;
use crate::geo::dotnet::{max, min};
use crate::geo::{Point, Rect, Size};

/// `Axis`: which of the two coordinate axes a rule is expressed on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    Horizontal,
    Vertical,
}

impl Axis {
    /// `LayoutGeometry.Perpendicular`: the axis a contact on this one leaves to be
    /// resolved.
    pub fn perpendicular(self) -> Axis {
        if self == Axis::Horizontal {
            Axis::Vertical
        } else {
            Axis::Horizontal
        }
    }
}

/// `Interval`: the segment `[lo, lo + size]` on one axis, in whatever unit the
/// caller works in.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Interval {
    pub lo: f64,
    pub size: f64,
}

impl Interval {
    /// The positional constructor `Interval(Lo, Size)`.
    pub const fn new(lo: f64, size: f64) -> Self {
        Self { lo, size }
    }

    /// `Interval.Hi`: `lo + size`, even for the empty rect's span, where that is
    /// NaN (positive plus negative infinity).
    pub fn hi(self) -> f64 {
        self.lo + self.size
    }

    /// `Interval.MovedTo`: the same span, starting at `lo`.
    pub fn moved_to(self, lo: f64) -> Interval {
        Interval { lo, ..self }
    }

    /// `Interval.OverlapWith`: length shared with `other`; zero or negative when
    /// they do not overlap, the negative value being the gap between them.
    pub fn overlap_with(self, other: Interval) -> f64 {
        min(self.hi(), other.hi()) - max(self.lo, other.lo)
    }

    /// `Interval.SharedMidpoint`: midpoint of the part shared with `other`, and of
    /// the gap between them when they do not overlap.
    pub fn shared_midpoint(self, other: Interval) -> f64 {
        (max(self.lo, other.lo) + min(self.hi(), other.hi())) / 2.0
    }
}

/// `EdgeContact`: how a span sits against another one along the axis they meet on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EdgeContact {
    /// No edge in common on this axis.
    None,
    /// The span ends where the anchor starts: it sits on the low side.
    Before,
    /// The span starts where the anchor ends: it sits on the high side.
    After,
}

/// The extension methods `LayoutGeometry` adds to `Rect`.
pub trait LayoutGeometryRect {
    /// `LayoutGeometry.On`: the extent of the rect along `axis`.
    fn on(&self, axis: Axis) -> Interval;

    /// `LayoutGeometry.MovedOn`: the same rect with its extent along `axis`
    /// starting at `lo`. Unused by the C# solvers, ported with the rest.
    fn moved_on(&self, axis: Axis, lo: f64) -> Rect;

    /// `LayoutGeometry.Translated`: `new Rect(new Point(X + dx, Y + dy), Size)`,
    /// so the empty rect stays empty.
    fn translated(&self, dx: f64, dy: f64) -> Rect;
}

impl LayoutGeometryRect for Rect {
    fn on(&self, axis: Axis) -> Interval {
        if axis == Axis::Horizontal {
            Interval::new(self.x(), self.width())
        } else {
            Interval::new(self.y(), self.height())
        }
    }

    fn moved_on(&self, axis: Axis, lo: f64) -> Rect {
        if axis == Axis::Horizontal {
            Rect::from_location_size(Point::new(lo, self.y()), self.size())
        } else {
            Rect::from_location_size(Point::new(self.x(), lo), self.size())
        }
    }

    fn translated(&self, dx: f64, dy: f64) -> Rect {
        Rect::from_location_size(Point::new(self.x() + dx, self.y() + dy), self.size())
    }
}

/// `LayoutGeometry.OverlapOn`: length `a` and `b` share along `axis`.
pub fn overlap_on(a: &Rect, b: &Rect, axis: Axis) -> f64 {
    a.on(axis).overlap_with(b.on(axis))
}

/// `LayoutGeometry.Overlap`: true when the two rects share actual surface, a
/// shared edge alone not counting.
pub fn overlap(a: &Rect, b: &Rect) -> bool {
    overlap_on(a, b, Axis::Horizontal) > 0.0 && overlap_on(a, b, Axis::Vertical) > 0.0
}

/// `LayoutGeometry.ContactBetween(Interval, Interval, double)`: whether `other`
/// meets `anchor` along one of their edges, within `tolerance`. `After` is tested
/// first; both can only hold for a span of zero or negative size.
pub fn contact_between(anchor: Interval, other: Interval, tolerance: f64) -> EdgeContact {
    if (other.lo - anchor.hi()).abs() <= tolerance {
        return EdgeContact::After;
    }
    if (other.hi() - anchor.lo).abs() <= tolerance {
        return EdgeContact::Before;
    }
    EdgeContact::None
}

/// `LayoutGeometry.ContactBetween(Rect, Rect, Axis, double)`: [`contact_between`]
/// on the two rects' extents along `axis`.
pub fn contact_between_rects(
    anchor: &Rect,
    other: &Rect,
    axis: Axis,
    tolerance: f64,
) -> EdgeContact {
    contact_between(anchor.on(axis), other.on(axis), tolerance)
}

/// `AxisProfile`: one monitor reduced to a single axis, the mm span of its panel
/// and the pixel span the system gives that same panel.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AxisProfile {
    pub mm: Interval,
    pub pixel: Interval,
}

impl AxisProfile {
    /// The positional constructor `AxisProfile(Mm, Pixel)`.
    pub const fn new(mm: Interval, pixel: Interval) -> Self {
        Self { mm, pixel }
    }

    /// `AxisProfile.Pitch`: millimetres per pixel along this axis. Infinite or NaN
    /// for a monitor reporting no pixels; see [`has_pixels`](Self::has_pixels).
    pub fn pitch(self) -> f64 {
        self.mm.size / self.pixel.size
    }

    /// `AxisProfile.HasPixels`: a monitor reporting no pixels has nothing to be
    /// proportional to.
    pub fn has_pixels(self) -> bool {
        self.pixel.size > 0.0
    }

    /// `AxisProfile.ToPixel`.
    pub fn to_pixel(self, mm: f64) -> f64 {
        self.pixel.lo + (mm - self.mm.lo) / self.pitch()
    }

    /// `AxisProfile.ToMm`.
    pub fn to_mm(self, pixel: f64) -> f64 {
        self.mm.lo + (pixel - self.pixel.lo) * self.pitch()
    }

    /// `AxisProfile.AtMm`: the same profile with its mm span starting at `lo`.
    pub fn at_mm(self, lo: f64) -> AxisProfile {
        AxisProfile {
            mm: self.mm.moved_to(lo),
            ..self
        }
    }

    /// `AxisProfile.AtPixel`: the same profile with its pixel span starting at `lo`.
    pub fn at_pixel(self, lo: f64) -> AxisProfile {
        AxisProfile {
            pixel: self.pixel.moved_to(lo),
            ..self
        }
    }
}

/// `MonitorSnapshot`: one monitor as the placement solvers see it.
///
/// `mm_bounds` is the panel (`DepthProjection.Bounds`), `mm_outside_bounds` the
/// panel plus bezels (`DepthProjection.OutsideBounds`), `pixel_bounds` the rect the
/// system positions it with (`ActiveSource.Source.InPixel.Bounds`). "Place from
/// system" reads the pixel rect and answers in mm; "apply to system" reads the mm
/// rects and only the pixel size.
#[derive(Clone, Debug, PartialEq)]
pub struct MonitorSnapshot {
    pub id: String,
    pub mm_bounds: Rect,
    pub mm_outside_bounds: Rect,
    pub pixel_bounds: Rect,
    pub primary: bool,
}

impl MonitorSnapshot {
    /// The positional constructor
    /// `MonitorSnapshot(Id, MmBounds, MmOutsideBounds, PixelBounds, Primary)`.
    pub fn new(
        id: impl Into<String>,
        mm_bounds: Rect,
        mm_outside_bounds: Rect,
        pixel_bounds: Rect,
        primary: bool,
    ) -> Self {
        Self {
            id: id.into(),
            mm_bounds,
            mm_outside_bounds,
            pixel_bounds,
            primary,
        }
    }

    /// `MonitorSnapshot.MmLocation`: the panel origin.
    pub fn mm_location(&self) -> Point {
        self.mm_bounds.location()
    }

    /// `MonitorSnapshot.PixelSize`.
    pub fn pixel_size(&self) -> Size {
        self.pixel_bounds.size()
    }

    /// `MonitorSnapshot.Profile`.
    pub fn profile(&self, axis: Axis) -> AxisProfile {
        AxisProfile::new(self.mm_bounds.on(axis), self.pixel_bounds.on(axis))
    }

    /// `MonitorSnapshot.LowBorder`: the bezel between the panel and the outside
    /// rect, on the low side of `axis`.
    pub fn low_border(&self, axis: Axis) -> f64 {
        self.mm_bounds.on(axis).lo - self.mm_outside_bounds.on(axis).lo
    }

    /// `MonitorSnapshot.MovedTo`: the same monitor with its panel origin at `mm`,
    /// bezels following.
    pub fn moved_to(&self, mm: Point) -> MonitorSnapshot {
        MonitorSnapshot {
            mm_bounds: Rect::from_location_size(mm, self.mm_bounds.size()),
            mm_outside_bounds: self
                .mm_outside_bounds
                .translated(mm.x - self.mm_bounds.x(), mm.y - self.mm_bounds.y()),
            ..self.clone()
        }
    }

    /// `MonitorSnapshot.ForCompaction`: the millimetre half, which is all
    /// compaction reads.
    pub fn for_compaction(&self) -> CompactionMonitor {
        CompactionMonitor::new(
            self.id.clone(),
            self.mm_bounds,
            self.mm_outside_bounds,
            self.primary,
        )
    }
}
