//! `ForceCompactTests.cs`, through the model: `Layout::force_compact` (and
//! `set_locations_from_system_configuration`) on monitors built the way the C#
//! test builds them. The three pure `DistanceHV_*` tests of that class live in
//! `distance_hv.rs`.
//!
//! ForceCompact places monitors nearest first. The ordering key is
//! `DistanceToTouch(...).DistanceHV()`: if the infinite no-touch axis leaks into
//! that value, every reachable monitor compares equal (infinity) and the order
//! degenerates to collection order — a far big monitor placed first steals the
//! place of a near small one, which then gets ejected to a random side (#450).

mod common;

use common::{add_with_source, assert_equal_precision, design_options, model, round_digits};
use lbm_layout::geo::dotnet::{enumerable_min, format_double, max, min};
use lbm_layout::geo::Rect;
use lbm_layout::model::{
    DisplaySize, DisplaySource, Layout, LayoutOptions, Monitor, PhysicalSource,
};
use lbm_layout::solve::distance::{RectDistance, ThicknessDistance};

/// `AddMonitor(layout, id, pnp, width, height, x, y, primary, pixelX, pixelY,
/// pixelWidth, pixelHeight)`: a borderless model, a monitor showing one attached
/// source, both handed to the layout, then `DepthProjection.X = x` and
/// `DepthProjection.Y = y`, in that order.
#[allow(clippy::too_many_arguments)]
fn add_monitor_px(
    layout: &mut Layout,
    id: &str,
    pnp: &str,
    width: f64,
    height: f64,
    x: f64,
    y: f64,
    primary: bool,
    pixel: Rect,
) {
    let model = model(layout, pnp, width, height, Some(0.0));
    let monitor = Monitor::new(id, &model);
    let mut source = DisplaySource::new(format!("{id}-src"));
    source.attached_to_desktop = true;
    source.primary = primary;
    source.in_pixel = DisplaySize::from_rect(pixel);
    let physical_source = PhysicalSource::new(format!("{id}-dev"), id, source);
    add_with_source(layout, monitor, physical_source, true);

    let location = layout.monitor(id).unwrap().location();
    layout.set_location(id, location.with_x(x));
    let location = layout.monitor(id).unwrap().location();
    layout.set_location(id, location.with_y(y));
}

/// `AddMonitor` with its default pixel rect, (0, 0) 1000 x 1000.
#[allow(clippy::too_many_arguments)]
fn add_monitor(
    layout: &mut Layout,
    id: &str,
    pnp: &str,
    width: f64,
    height: f64,
    x: f64,
    y: f64,
    primary: bool,
) {
    add_monitor_px(
        layout,
        id,
        pnp,
        width,
        height,
        x,
        y,
        primary,
        Rect::new(0.0, 0.0, 1000.0, 1000.0),
    );
}

fn projection(layout: &Layout, id: &str) -> DisplaySize {
    layout
        .depth_projection(layout.monitor(id).unwrap())
        .expect("every monitor here shows a source")
}

/// C#: `ForceCompactTests.PrimaryDrag_CompactPreservesArrangement`.
#[test]
fn primary_drag_compact_preserves_arrangement() {
    // Primary 700x400 at (0,0), a same-size neighbour touching its right side, a
    // 1650x920 TV touching the neighbour's right side, all three translated away by
    // (54, 6) — what dragging the primary does — then compacted: each comes back
    // against its former neighbour, nearest first, so the TV lands on the
    // neighbour, not on the primary over the neighbour's spot.
    let mut layout = Layout::new(design_options());

    // The far TV is deliberately FIRST in the collection: the degenerate
    // pre-fix ordering (all keys infinite) placed it first.
    add_monitor(
        &mut layout,
        "TV",
        "TV_0001",
        1650.0,
        920.0,
        1454.0,
        -294.0,
        false,
    );
    add_monitor(&mut layout, "P", "PHL0001", 700.0, 400.0, 0.0, 0.0, true);
    add_monitor(
        &mut layout,
        "S",
        "SAM0001",
        700.0,
        400.0,
        754.0,
        16.0,
        false,
    );

    layout.parse_physical_monitors();
    assert_eq!(layout.primary_monitor().map(|m| m.id.as_str()), Some("P"));

    layout.force_compact();

    // Neighbour pulled back against the primary's right edge, same height.
    assert_equal_precision(700.0, projection(&layout, "S").x, 2);
    assert_equal_precision(16.0, projection(&layout, "S").y, 2);

    // TV pulled back against the neighbour's right edge, not teleported.
    assert_equal_precision(1400.0, projection(&layout, "TV").x, 2);
    assert_equal_precision(-294.0, projection(&layout, "TV").y, 2);
}

/// C#: `ForceCompactTests.PrimaryDrag_StackedClusterComesBackAsOneBlock`.
#[test]
fn primary_drag_stacked_cluster_comes_back_as_one_block() {
    // A monitor stacked on top of the TV must stay aligned with it through a
    // primary drag: both belong to the same touching cluster and travel as one
    // block.
    let mut layout = Layout::new(design_options());

    add_monitor(&mut layout, "P", "PHL0001", 700.0, 400.0, 0.0, 0.0, true);
    // TV touching the primary's right side, stacked monitor on top of the TV.
    add_monitor(
        &mut layout,
        "TV",
        "TV_0001",
        1650.0,
        920.0,
        754.0,
        -300.0,
        false,
    );
    add_monitor(
        &mut layout,
        "S",
        "SAM0001",
        700.0,
        400.0,
        954.0,
        -700.0,
        false,
    );

    layout.parse_physical_monitors();
    layout.force_compact();

    // The {TV, stacked} cluster slides back 54 as one rigid block.
    assert_equal_precision(700.0, projection(&layout, "TV").x, 2);
    assert_equal_precision(-300.0, projection(&layout, "TV").y, 2);
    assert_equal_precision(900.0, projection(&layout, "S").x, 2);
    assert_equal_precision(-700.0, projection(&layout, "S").y, 2);
}

/// C#: `ForceCompactTests.OverlappingMonitors_GetSpreadApart_PrimaryStays`.
#[test]
fn overlapping_monitors_get_spread_apart_primary_stays() {
    // Two monitors overlapping each other while both border-to-border with the TV:
    // the overlap is resolved (the primary never moves) without tearing the
    // existing contacts apart.
    let mut layout = Layout::new(design_options());

    add_monitor(&mut layout, "TV", "TV_0001", 1650.0, 920.0, 0.0, 0.0, false);
    // Both under the TV, touching its bottom edge, overlapping each other.
    add_monitor(
        &mut layout,
        "P",
        "PHL0001",
        700.0,
        400.0,
        100.0,
        920.0,
        true,
    );
    add_monitor(
        &mut layout,
        "S",
        "SAM0001",
        700.0,
        400.0,
        400.0,
        920.0,
        false,
    );

    layout.parse_physical_monitors();
    layout.force_compact();

    // Primary untouched.
    assert_equal_precision(100.0, projection(&layout, "P").x, 2);
    assert_equal_precision(920.0, projection(&layout, "P").y, 2);
    // TV untouched: still in contact with the primary.
    assert_equal_precision(0.0, projection(&layout, "TV").x, 2);
    assert_equal_precision(0.0, projection(&layout, "TV").y, 2);

    // The overlap is gone…
    let p = projection(&layout, "P").outside_bounds();
    let s = projection(&layout, "S").outside_bounds();
    let overlap_x = min(p.right(), s.right()) - max(p.left(), s.left());
    let overlap_y = min(p.bottom(), s.bottom()) - max(p.top(), s.top());
    assert!(
        !(overlap_x > 0.01 && overlap_y > 0.01),
        "still overlapping: {overlap_x}x{overlap_y}"
    );

    // …and the freed monitor is still in contact with the rest, not teleported.
    let tv = projection(&layout, "TV").outside_bounds();
    let d1 = enumerable_min(s.distance(&p).to_array().map(f64::abs)).unwrap();
    let d2 = enumerable_min(s.distance(&tv).to_array().map(f64::abs)).unwrap();
    assert!(
        d1 < 0.01 || d2 < 0.01,
        "lost all contacts (to primary: {d1}, to tv: {d2})"
    );
}

/// C#: `ForceCompactTests.SystemPlacement_SideBySidePixels_StaysSideBySideInMm`.
#[test]
fn system_placement_side_by_side_pixels_stays_side_by_side_in_mm() {
    // Fresh-install auto-placement: three 4K monitors side by side in PIXEL space
    // whose physical widths differ wildly (two 32" and a TV). The monitor whose only
    // pixel adjacency is with the MIDDLE one must end up beside it.
    let mut layout = Layout::new(design_options());

    // Collection order puts the TV first, like the real HEC/PHL/SAME layout.
    add_monitor_px(
        &mut layout,
        "TV",
        "TV_0001",
        1650.0,
        920.0,
        0.0,
        0.0,
        false,
        Rect::new(7680.0, 0.0, 3840.0, 2160.0),
    );
    add_monitor_px(
        &mut layout,
        "P",
        "PHL0001",
        700.0,
        400.0,
        0.0,
        0.0,
        true,
        Rect::new(0.0, 0.0, 3840.0, 2160.0),
    );
    add_monitor_px(
        &mut layout,
        "S",
        "SAM0001",
        700.0,
        400.0,
        0.0,
        0.0,
        false,
        Rect::new(3840.0, 0.0, 3840.0, 2160.0),
    );

    layout.parse_physical_monitors();
    layout.set_locations_from_system_configuration(true);

    // P | S | H side by side.
    assert_equal_precision(700.0, projection(&layout, "S").x, 2);
    assert_equal_precision(1400.0, projection(&layout, "TV").x, 2);

    // S has P's pitch, so sharing the same pixel rows puts it at the same height.
    assert_equal_precision(400.0, projection(&layout, "S").bounds().bottom(), 2);

    // The TV does not, and it is centred on what it shares with its neighbour,
    // which is the rule the pixel solver inverts: 460mm of TV either side of the
    // 200mm midpoint of S.
    let middle = projection(&layout, "S").bounds();
    let tv = projection(&layout, "TV").bounds();
    assert_equal_precision(200.0, middle.y() + middle.height() / 2.0, 2);
    assert_equal_precision(200.0, tv.y() + tv.height() / 2.0, 2);
}

/// C#: `ForceCompactTests.MinimalEdgeOverlap_IsIndependentOfTheAlgorithm`, both rows.
#[test]
fn minimal_edge_overlap_is_independent_of_the_algorithm() {
    // The corridor requirement is a layout-editing rule, independent of the crossing
    // algorithm: the same corner-only meeting is slid into a 20mm corridor under
    // both.
    for algorithm in ["Strait", "Cross"] {
        println!("InlineData({algorithm:?})");
        let mut layout = Layout::new(LayoutOptions {
            algorithm: algorithm.to_owned(),
            minimal_edge_overlap: 20.0,
            ..design_options()
        });

        add_monitor(&mut layout, "P", "PHL0001", 700.0, 400.0, 0.0, 0.0, true);
        add_monitor(
            &mut layout,
            "S",
            "SAM0001",
            600.0,
            340.0,
            700.0,
            400.0,
            false,
        );

        layout.parse_physical_monitors();
        layout.force_compact();

        let corner = projection(&layout, "S");
        assert_equal_precision(700.0, corner.x, 2);
        assert_equal_precision(380.0, corner.y, 2);
    }
}

/// C#: `ForceCompactTests.ForceCompact_DoesNotDependOnDeclarationOrder`.
#[test]
fn force_compact_does_not_depend_on_declaration_order() {
    // Same layout, monitors declared in a different order: the compacted result
    // must be identical. Through the model, so the snapshot/apply bridge is covered
    // too.
    fn run(order: &[usize]) -> String {
        let mut layout = Layout::new(design_options());

        for &i in order {
            match i {
                0 => add_monitor(&mut layout, "P", "PHL0001", 700.0, 400.0, 0.0, 0.0, true),
                1 => add_monitor(
                    &mut layout,
                    "TV",
                    "TV_0001",
                    1650.0,
                    920.0,
                    1454.0,
                    -294.0,
                    false,
                ),
                2 => add_monitor(
                    &mut layout,
                    "S",
                    "SAM0001",
                    700.0,
                    400.0,
                    754.0,
                    16.0,
                    false,
                ),
                _ => add_monitor(
                    &mut layout,
                    "D",
                    "DEL0001",
                    520.0,
                    330.0,
                    -1400.0,
                    700.0,
                    false,
                ),
            }
        }

        layout.parse_physical_monitors();
        layout.force_compact();

        // `OrderBy(m => m.Id, StringComparer.Ordinal)`, positions `Math.Round(v, 6)`.
        let mut monitors: Vec<_> = layout.monitors().iter().collect();
        monitors.sort_by(|a, b| a.id.as_bytes().cmp(b.id.as_bytes()));
        monitors
            .iter()
            .map(|m| {
                let dp = layout.depth_projection(m).unwrap();
                format!(
                    "{}={},{}",
                    m.id,
                    format_double(round_digits(dp.x, 6)),
                    format_double(round_digits(dp.y, 6))
                )
            })
            .collect::<Vec<_>>()
            .join(" ")
    }

    let expected = run(&[0, 1, 2, 3]);

    assert_eq!(expected, run(&[3, 2, 1, 0]));
    assert_eq!(expected, run(&[1, 3, 0, 2]));
    assert_eq!(expected, run(&[2, 0, 3, 1]));
    assert_eq!(expected, run(&[1, 2, 3, 0]));
}
