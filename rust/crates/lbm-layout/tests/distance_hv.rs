//! The three pure `DistanceHV` tests of `ForceCompactTests.cs`. The rest of that
//! file drives `ForceCompact` through the model: `force_compact.rs`.

mod common;

use common::assert_equal_precision;
use lbm_layout::geo::Thickness;
use lbm_layout::solve::distance::{ThicknessDistance, INFINITY};

/// C#: `ForceCompactTests.DistanceHV_FiniteAxisWins_OverInfiniteAxis`.
#[test]
fn distance_hv_finite_axis_wins_over_infinite_axis() {
    // Side-by-side: horizontal touch at 54.47, no vertical touch possible.
    let d = Thickness::new(54.47, f64::INFINITY, -1510.0, f64::INFINITY);
    assert_equal_precision(54.47, d.distance_hv(), 2);

    // Stacked: vertical touch at 33, no horizontal touch possible.
    let v = Thickness::new(f64::INFINITY, 33.0, f64::INFINITY, -400.0);
    assert_equal_precision(33.0, v.distance_hv(), 2);
}

/// C#: `ForceCompactTests.DistanceHV_UnreachableByOneTranslation_StaysInfinite`.
#[test]
fn distance_hv_unreachable_by_one_translation_stays_infinite() {
    assert_eq!(INFINITY.distance_hv(), f64::INFINITY);
}

/// C#: `ForceCompactTests.DistanceHV_Overlap_StaysNegative`.
#[test]
fn distance_hv_overlap_stays_negative() {
    // Full overlap: raw distances, all negative — unchanged behavior.
    let d = Thickness::new(-100.0, -200.0, -300.0, -50.0);
    assert_equal_precision(-50.0, d.distance_hv(), 2);
}
