//! `SnapToActualSizesTests.cs`. The apply-layout positions are computed against
//! PREDICTED logical sizes (round(native / scale)); the compositor's own rounding
//! is authoritative and can differ by a pixel. `snap_to_actual_sizes` rebuilds
//! the intended edge contacts with the actual sizes so a contact never silently
//! becomes a gap or an overlap.

mod common;

use common::assert_equal_precision;
use lbm_layout::geo::Point;
use lbm_layout::linux::{snap_to_actual_sizes, PlacedOutput as P};
use lbm_layout::solve::IdMap;

#[track_caller]
fn assert_at(result: &IdMap<Point>, name: &str, x: f64, y: f64) {
    assert_equal_precision(x, result[name].x, 2);
    assert_equal_precision(y, result[name].y, 2);
}

/// C#: `SnapToActualSizesTests.ActualSizeSmaller_ClosesTheGap`.
#[test]
fn actual_size_smaller_closes_the_gap() {
    let result = snap_to_actual_sizes(&[
        P::new("A", 0.0, 0.0, 1000.0, 1000.0, 1000.0, 1000.0),
        P::new("B", 1000.0, 0.0, 2000.0, 1000.0, 1999.0, 1000.0),
        P::new("C", 3000.0, 0.0, 1000.0, 1000.0, 1000.0, 1000.0),
    ]);

    assert_at(&result, "B", 1000.0, 0.0);
    assert_at(&result, "C", 2999.0, 0.0);
}

/// C#: `SnapToActualSizesTests.ActualSizeBigger_PushesTheNeighbourAway`.
#[test]
fn actual_size_bigger_pushes_the_neighbour_away() {
    let result = snap_to_actual_sizes(&[
        P::new("A", 0.0, 0.0, 1000.0, 1000.0, 1001.0, 1000.0),
        P::new("B", 1000.0, 0.0, 1000.0, 1000.0, 1000.0, 1000.0),
    ]);

    assert_at(&result, "B", 1001.0, 0.0);
}

/// C#: `SnapToActualSizesTests.VerticalContact_ChainsOnActualHeights`.
#[test]
fn vertical_contact_chains_on_actual_heights() {
    let result = snap_to_actual_sizes(&[
        P::new("A", 0.0, 0.0, 1000.0, 500.0, 1000.0, 499.0),
        P::new("B", 0.0, 500.0, 1000.0, 500.0, 1000.0, 500.0),
    ]);

    assert_at(&result, "B", 0.0, 499.0);
}

/// C#: `SnapToActualSizesTests.NoContact_KeepsIntendedPositions`.
#[test]
fn no_contact_keeps_intended_positions() {
    let result = snap_to_actual_sizes(&[
        P::new("A", 0.0, 0.0, 1000.0, 1000.0, 999.0, 1000.0),
        P::new("B", 5000.0, 0.0, 1000.0, 1000.0, 1000.0, 1000.0),
    ]);

    assert_at(&result, "A", 0.0, 0.0);
    assert_at(&result, "B", 5000.0, 0.0);
}

/// C#: `SnapToActualSizesTests.FourOutputRow_MixedDrift_KeepsEveryContact`.
#[test]
fn four_output_row_mixed_drift_keeps_every_contact() {
    // the maintainer's row: DELL exact, PHL 1px smaller, SAM 1px bigger, TV exact
    let result = snap_to_actual_sizes(&[
        P::new("DP-1", 0.0, 624.0, 1280.0, 720.0, 1280.0, 720.0),
        P::new("DP-2", 1280.0, 308.0, 3072.0, 1728.0, 3071.0, 1728.0),
        P::new("DP-3", 4352.0, 0.0, 3072.0, 1728.0, 3073.0, 1728.0),
        P::new("HDMI-A-1", 7424.0, 0.0, 2649.0, 1490.0, 2649.0, 1490.0),
    ]);

    assert_at(&result, "DP-1", 0.0, 624.0);
    assert_at(&result, "DP-2", 1280.0, 308.0);
    assert_at(&result, "DP-3", 4351.0, 0.0);
    assert_at(&result, "HDMI-A-1", 7424.0, 0.0);
}

/// C#: `SnapToActualSizesTests.PredictionExact_IsANoOp`.
#[test]
fn prediction_exact_is_a_no_op() {
    let result = snap_to_actual_sizes(&[
        P::new("A", 0.0, 0.0, 1000.0, 1000.0, 1000.0, 1000.0),
        P::new("B", 1000.0, 0.0, 1000.0, 1000.0, 1000.0, 1000.0),
    ]);

    assert_at(&result, "A", 0.0, 0.0);
    assert_at(&result, "B", 1000.0, 0.0);
}
