//! `WallpaperSpanSlicerTests.cs`.

mod common;

use common::assert_equal_precision;
use lbm_layout::geo::{Rect, Size};
use lbm_layout::wallpaper::{compute_bounds_mm, compute_slices, ScreenInput, Slice};

fn screen(id: &str, x: f64, y: f64, w_mm: f64, h_mm: f64, w_px: f64, h_px: f64) -> ScreenInput {
    ScreenInput::new(id, Rect::new(x, y, w_mm, h_mm), Size::new(w_px, h_px))
}

/// `slices.Single(s => s.Id == id)`.
fn single<'a>(slices: &'a [Slice], id: &str) -> &'a Slice {
    let found: Vec<&Slice> = slices.iter().filter(|s| s.id == id).collect();
    assert_eq!(found.len(), 1, "Single(s => s.Id == {id:?})");
    found[0]
}

fn crop_of(slices: &[Slice], id: &str) -> Rect {
    single(slices, id).source_px
}

#[test]
fn bounds_mm_is_union_of_outside_bounds() {
    let bounds = compute_bounds_mm([
        Rect::new(-20.0, -20.0, 520.0, 340.0),
        Rect::new(500.0, 10.0, 520.0, 340.0),
    ]);

    assert_eq!(Rect::new(-20.0, -20.0, 1040.0, 370.0), bounds);
}

#[test]
fn bounds_mm_of_nothing_is_empty() {
    assert!(compute_bounds_mm([]).is_empty());
}

#[test]
fn two_equal_screens_split_the_image_in_halves() {
    // Two 500x300mm panels side by side, no bezels; image matches the box aspect exactly.
    let bounds = Rect::new(0.0, 0.0, 1000.0, 300.0);
    let slices = compute_slices(
        Size::new(2000.0, 600.0),
        bounds,
        &[
            screen("A", 0.0, 0.0, 500.0, 300.0, 1920.0, 1080.0),
            screen("B", 500.0, 0.0, 500.0, 300.0, 1920.0, 1080.0),
        ],
    );

    assert_eq!(Rect::new(0.0, 0.0, 1000.0, 600.0), crop_of(&slices, "A"));
    assert_eq!(Rect::new(1000.0, 0.0, 1000.0, 600.0), crop_of(&slices, "B"));
}

#[test]
fn mixed_dpi_screens_get_same_crop_different_output() {
    // Same physical panels, one FHD one 4K: identical crops, different output sizes.
    let bounds = Rect::new(0.0, 0.0, 1000.0, 300.0);
    let slices = compute_slices(
        Size::new(2000.0, 600.0),
        bounds,
        &[
            screen("FHD", 0.0, 0.0, 500.0, 300.0, 1920.0, 1080.0),
            screen("4K", 500.0, 0.0, 500.0, 300.0, 3840.0, 2160.0),
        ],
    );

    assert_eq!(
        crop_of(&slices, "FHD").size(),
        crop_of(&slices, "4K").size()
    );
    assert_eq!(Size::new(1920.0, 1080.0), single(&slices, "FHD").output_px);
    assert_eq!(Size::new(3840.0, 2160.0), single(&slices, "4K").output_px);
}

#[test]
fn bezel_gap_separates_crops_by_scaled_gap() {
    // 40mm of bezels between the panels: the image must skip gap*s pixels between crops.
    let bounds = Rect::new(0.0, 0.0, 1040.0, 300.0);
    let slices = compute_slices(
        Size::new(2080.0, 600.0),
        bounds,
        &[
            screen("A", 0.0, 0.0, 500.0, 300.0, 1920.0, 1080.0),
            screen("B", 540.0, 0.0, 500.0, 300.0, 1920.0, 1080.0),
        ],
    );

    let s = 2080.0 / 1040.0; // 2 px/mm
    assert_equal_precision(
        40.0 * s,
        crop_of(&slices, "B").left() - crop_of(&slices, "A").right(),
        6,
    );
}

#[test]
fn aspect_mismatch_is_center_cropped() {
    // Image twice as tall as needed: the scale is driven by width, excess height is
    // split evenly above and below.
    let bounds = Rect::new(0.0, 0.0, 1000.0, 300.0);
    let slices = compute_slices(
        Size::new(2000.0, 1200.0),
        bounds,
        &[screen("A", 0.0, 0.0, 1000.0, 300.0, 1920.0, 1080.0)],
    );

    // s = 2 px/mm, box = 2000x600 px, offsetY = (1200-600)/2 = 300.
    assert_eq!(Rect::new(0.0, 300.0, 2000.0, 600.0), crop_of(&slices, "A"));
}

#[test]
fn portrait_screen_gets_portrait_crop() {
    let bounds = Rect::new(0.0, 0.0, 800.0, 500.0);
    let slices = compute_slices(
        Size::new(1600.0, 1000.0),
        bounds,
        &[
            screen("L", 0.0, 0.0, 500.0, 300.0, 1920.0, 1080.0),
            screen("P", 500.0, 0.0, 300.0, 500.0, 1080.0, 1920.0),
        ],
    );

    let crop = crop_of(&slices, "P");
    assert!(crop.height() > crop.width());
    assert_eq!(Rect::new(1000.0, 0.0, 600.0, 1000.0), crop);
}

#[test]
fn crop_is_clamped_to_image_edges() {
    // A screen flush with the box edge must never produce a crop outside the image.
    let bounds = Rect::new(-500.0, -300.0, 1000.0, 600.0);
    let slices = compute_slices(
        Size::new(1000.0, 600.0),
        bounds,
        &[
            screen("A", -500.0, -300.0, 500.0, 600.0, 1920.0, 1080.0),
            screen(
                "B",
                500.0 - 500.0,
                -300.0 + 300.0,
                500.0,
                300.0,
                1920.0,
                1080.0,
            ),
        ],
    );

    let image = Rect::new(0.0, 0.0, 1000.0, 600.0);
    for slice in &slices {
        assert!(
            image.contains_rect(&slice.source_px),
            "{}: {:?} outside image",
            slice.id,
            slice.source_px
        );
    }
}

#[test]
fn empty_or_degenerate_inputs_yield_no_slices() {
    assert!(compute_slices(
        Size::new(0.0, 0.0),
        Rect::new(0.0, 0.0, 100.0, 100.0),
        &[screen("A", 0.0, 0.0, 100.0, 100.0, 100.0, 100.0)]
    )
    .is_empty());
    assert!(compute_slices(
        Size::new(100.0, 100.0),
        Rect::EMPTY,
        &[screen("A", 0.0, 0.0, 100.0, 100.0, 100.0, 100.0)]
    )
    .is_empty());
}
