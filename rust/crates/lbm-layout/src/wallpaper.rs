//! `Wallpaper/WallpaperSpanSlicer.cs`: slice a single source image over the whole
//! monitor set in physical (mm) space, so each screen gets the portion of the image
//! its panel covers and the picture stays continuous through bezels and across
//! mixed sizes and DPIs.

use crate::geo::dotnet::{max, min};
use crate::geo::{Rect, Size};

/// `WallpaperSpanSlicer.ScreenInput`: a screen's panel area in layout mm
/// (`DepthProjection.Bounds`) and the native pixel size its slice is rendered at.
#[derive(Clone, Debug, PartialEq)]
pub struct ScreenInput {
    pub id: String,
    pub visible_mm: Rect,
    pub output_px: Size,
}

impl ScreenInput {
    /// The positional constructor `ScreenInput(Id, VisibleMm, OutputPx)`.
    pub fn new(id: impl Into<String>, visible_mm: Rect, output_px: Size) -> Self {
        Self {
            id: id.into(),
            visible_mm,
            output_px,
        }
    }
}

/// `WallpaperSpanSlicer.Slice`: the crop rectangle in source-image pixels (clamped
/// to the image, so empty when the screen falls outside it) and the output size.
#[derive(Clone, Debug, PartialEq)]
pub struct Slice {
    pub id: String,
    pub source_px: Rect,
    pub output_px: Size,
}

/// `WallpaperSpanSlicer.ComputeBoundsMm`: the bounding box of the monitor set,
/// bezels included (`DepthProjection.OutsideBounds`), empty when there is nothing
/// to bound. Folded by hand, skipping empty rects, rather than with HLab.Geo's
/// `Union`.
pub fn compute_bounds_mm(outside_bounds_mm: impl IntoIterator<Item = Rect>) -> Rect {
    let mut left = f64::INFINITY;
    let mut top = f64::INFINITY;
    let mut right = f64::NEG_INFINITY;
    let mut bottom = f64::NEG_INFINITY;

    for r in outside_bounds_mm {
        if r.is_empty() {
            continue;
        }
        left = min(left, r.left());
        top = min(top, r.top());
        right = max(right, r.right());
        bottom = max(bottom, r.bottom());
    }

    if left.is_infinite() {
        Rect::EMPTY
    } else {
        Rect::new(left, top, right - left, bottom - top)
    }
}

/// `WallpaperSpanSlicer.ComputeSlices`: scale the image so the mm bounding box fits
/// inside it (the smaller of the two px-per-mm ratios), centre the box, and crop
/// each screen's visible area out of it. No slices for an empty or degenerate box
/// or image.
pub fn compute_slices(image_px: Size, bounds_mm: Rect, screens: &[ScreenInput]) -> Vec<Slice> {
    if bounds_mm.is_empty()
        || bounds_mm.width() <= 0.0
        || bounds_mm.height() <= 0.0
        || image_px.width() <= 0.0
        || image_px.height() <= 0.0
    {
        return Vec::new();
    }

    // px per mm so the box fits inside the image; the excess on the other axis is
    // centre-cropped.
    let s = min(
        image_px.width() / bounds_mm.width(),
        image_px.height() / bounds_mm.height(),
    );
    let offset_x = (image_px.width() - bounds_mm.width() * s) / 2.0;
    let offset_y = (image_px.height() - bounds_mm.height() * s) / 2.0;

    let image = Rect::new(0.0, 0.0, image_px.width(), image_px.height());

    screens
        .iter()
        .map(|screen| {
            let crop = Rect::new(
                (screen.visible_mm.x() - bounds_mm.x()) * s + offset_x,
                (screen.visible_mm.y() - bounds_mm.y()) * s + offset_y,
                screen.visible_mm.width() * s,
                screen.visible_mm.height() * s,
            )
            // The in-place `crop.Intersect(image)`.
            .intersect(&image);

            Slice {
                id: screen.id.clone(),
                source_px: crop,
                output_px: screen.output_px,
            }
        })
        .collect()
}
