//! `LayoutGeometryTests.cs`: the primitives both placement directions are built
//! on, tested on plain rectangles.

mod common;

use common::assert_equal_precision;
use lbm_layout::geo::Rect;
use lbm_layout::solve::edge_projection;
use lbm_layout::solve::layout_geometry::{self, LayoutGeometryRect};
use lbm_layout::solve::{Axis, AxisProfile, EdgeContact, Interval};

#[test]
fn on_reads_the_requested_axis() {
    let rect = Rect::new(10.0, 20.0, 30.0, 40.0);

    assert_eq!(Interval::new(10.0, 30.0), rect.on(Axis::Horizontal));
    assert_eq!(Interval::new(20.0, 40.0), rect.on(Axis::Vertical));
}

#[test]
fn perpendicular_is_the_other_axis() {
    assert_eq!(Axis::Vertical, Axis::Horizontal.perpendicular());
    assert_eq!(Axis::Horizontal, Axis::Vertical.perpendicular());
}

#[test]
fn overlap_with_is_negative_for_a_gap() {
    assert_eq!(
        5.0,
        Interval::new(0.0, 10.0).overlap_with(Interval::new(5.0, 10.0))
    );

    // Touching is zero overlap, not positive: a shared edge is not a crossable corridor.
    assert_eq!(
        0.0,
        Interval::new(0.0, 10.0).overlap_with(Interval::new(10.0, 10.0))
    );

    // Disjoint: the value is the gap, which is what the callers order islands by.
    assert_eq!(
        -3.0,
        Interval::new(0.0, 10.0).overlap_with(Interval::new(13.0, 10.0))
    );
}

#[test]
fn shared_midpoint_is_defined_even_without_overlap() {
    // Overlapping: midpoint of the shared part.
    assert_eq!(
        7.5,
        Interval::new(0.0, 10.0).shared_midpoint(Interval::new(5.0, 10.0))
    );

    // Disjoint: midpoint of the gap.
    assert_eq!(
        11.5,
        Interval::new(0.0, 10.0).shared_midpoint(Interval::new(13.0, 10.0))
    );
}

#[test]
fn overlap_needs_surface_not_just_a_shared_edge() {
    let a = Rect::new(0.0, 0.0, 10.0, 10.0);

    assert!(layout_geometry::overlap(
        &a,
        &Rect::new(5.0, 5.0, 10.0, 10.0)
    ));

    // Edge to edge, and corner to corner: neither is an overlap.
    assert!(!layout_geometry::overlap(
        &a,
        &Rect::new(10.0, 0.0, 10.0, 10.0)
    ));
    assert!(!layout_geometry::overlap(
        &a,
        &Rect::new(10.0, 10.0, 10.0, 10.0)
    ));
}

#[test]
fn contact_between_reads_both_sides() {
    let anchor = Interval::new(0.0, 10.0);

    assert_eq!(
        EdgeContact::After,
        layout_geometry::contact_between(anchor, Interval::new(10.0, 5.0), 0.0)
    );
    assert_eq!(
        EdgeContact::Before,
        layout_geometry::contact_between(anchor, Interval::new(-5.0, 5.0), 0.0)
    );
    assert_eq!(
        EdgeContact::None,
        layout_geometry::contact_between(anchor, Interval::new(12.0, 5.0), 0.0)
    );
}

#[test]
fn contact_between_tolerance_is_what_separates_the_two_directions() {
    let anchor = Interval::new(0.0, 10.0);
    let three_away = Interval::new(13.0, 5.0);

    // Pixel edges are integers the system reports verbatim: 3 apart is not adjacent.
    assert_eq!(
        EdgeContact::None,
        layout_geometry::contact_between(anchor, three_away, 0.0)
    );

    // Bezel widths are hand-entered millimetres: 3mm of air still counts.
    assert_eq!(
        EdgeContact::After,
        layout_geometry::contact_between(anchor, three_away, 5.0)
    );
}

#[test]
fn to_pixel_and_to_mm_are_inverses() {
    // A 27" 4K panel at scale 2: 597.7mm of glass over 1920 logical pixels.
    let profile = AxisProfile::new(Interval::new(100.0, 597.7), Interval::new(2560.0, 1920.0));

    assert_equal_precision(2560.0, profile.to_pixel(100.0), 9);
    assert_equal_precision(100.0, profile.to_mm(2560.0), 9);
    assert_equal_precision(1234.5, profile.to_pixel(profile.to_mm(1234.5)), 6);
}

#[test]
fn has_pixels_is_false_for_a_monitor_reporting_none() {
    assert!(!AxisProfile::new(Interval::new(0.0, 300.0), Interval::new(0.0, 0.0)).has_pixels());
    assert!(AxisProfile::new(Interval::new(0.0, 300.0), Interval::new(0.0, 1080.0)).has_pixels());
}

/// The invariant itself: whichever unknown you solve for, the shared physical
/// midpoint lands on the same coordinate on both monitors.
#[test]
fn pixel_origin_and_millimetre_origin_solve_the_same_invariant() {
    // 27" 4K at scale 2 next to a 24" FHD, physically centred: different pitches.
    let anchor = AxisProfile::new(Interval::new(0.0, 336.2), Interval::new(0.0, 1080.0));
    let target_mm = Interval::new(18.6, 299.0);
    let target = AxisProfile::new(target_mm, Interval::new(0.0, 1080.0));

    // Forward: where does the target go in pixels?
    let pixel_lo = edge_projection::pixel_origin(anchor, target);

    // Backward: given that pixel answer, where does it go in millimetres?
    let mm_lo = edge_projection::millimetre_origin(anchor, target.at_pixel(pixel_lo));

    assert_equal_precision(target_mm.lo, mm_lo, 6);
}

#[test]
fn millimetre_origin_matches_the_closed_form_when_pitches_are_equal() {
    // Same pitch on both: the iteration must land exactly on the proportional answer.
    let anchor = AxisProfile::new(Interval::new(0.0, 270.0), Interval::new(0.0, 1080.0));
    let target = AxisProfile::new(Interval::new(0.0, 270.0), Interval::new(-219.0, 1080.0));

    // 219 pixels up, at 0.25 mm/px.
    assert_equal_precision(
        -54.75,
        edge_projection::millimetre_origin(anchor, target),
        9,
    );
}
