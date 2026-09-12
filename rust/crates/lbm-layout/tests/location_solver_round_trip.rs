//! `LocationSolverRoundTripTests.cs`: read a system pixel configuration into
//! millimetres with `system_location`, push it back out with `pixel_location`,
//! and land on the configuration you started from — where that can hold. The
//! second half pins down which configurations are lost and what they become.
//!
//! `ITestOutputHelper.WriteLine` is `println!`, which the test harness shows for a
//! failing test.

mod common;

use std::collections::HashSet;

use common::assert_equal_precision;
use lbm_layout::geo::{Point, Rect};
use lbm_layout::solve::compaction::{self, CompactionOptions};
use lbm_layout::solve::layout_geometry;
use lbm_layout::solve::pixel_location::{self, DEFAULT_TOLERANCE_MM};
use lbm_layout::solve::system_location;
use lbm_layout::solve::{Axis, CompactionMonitor, IdMap, MonitorSnapshot};

/// A monitor as a freshly built layout has it: known panel size and bezels, known
/// pixel rect, and no millimetre position yet — everything stacked at the origin.
#[allow(clippy::too_many_arguments)]
fn monitor_with_bezel(
    id: &str,
    width_mm: f64,
    height_mm: f64,
    pixel_x: f64,
    pixel_y: f64,
    pixel_width: f64,
    pixel_height: f64,
    primary: bool,
    bezel: f64,
) -> MonitorSnapshot {
    MonitorSnapshot::new(
        id,
        Rect::new(0.0, 0.0, width_mm, height_mm),
        Rect::new(
            -bezel,
            -bezel,
            width_mm + 2.0 * bezel,
            height_mm + 2.0 * bezel,
        ),
        Rect::new(pixel_x, pixel_y, pixel_width, pixel_height),
        primary,
    )
}

/// `Monitor(...)` with its default 10 mm bezel.
#[allow(clippy::too_many_arguments)]
fn monitor(
    id: &str,
    width_mm: f64,
    height_mm: f64,
    pixel_x: f64,
    pixel_y: f64,
    pixel_width: f64,
    pixel_height: f64,
    primary: bool,
) -> MonitorSnapshot {
    monitor_with_bezel(
        id,
        width_mm,
        height_mm,
        pixel_x,
        pixel_y,
        pixel_width,
        pixel_height,
        primary,
        10.0,
    )
}

/// "Place from system" end to end, as `SetLocationsFromSystemConfiguration` runs
/// it: solve the walk, then compact (`minimalEdgeOverlap` defaults to 20).
fn place_from_system(
    monitors: &[MonitorSnapshot],
    minimal_edge_overlap: f64,
) -> Vec<MonitorSnapshot> {
    let solved = system_location::solve(monitors, None);
    let placed: Vec<MonitorSnapshot> = monitors
        .iter()
        .map(|m| match solved.get(&m.id) {
            Some(&p) => m.moved_to(p),
            None => m.clone(),
        })
        .collect();

    let compaction_input: Vec<CompactionMonitor> =
        placed.iter().map(|m| m.for_compaction()).collect();
    let offsets = compaction::solve(
        &compaction_input,
        CompactionOptions::new(false, false, minimal_edge_overlap),
    );

    placed
        .iter()
        .map(|m| match offsets.get(&m.id) {
            Some(v) => m.moved_to(Point::new(m.mm_bounds.x() + v.x, m.mm_bounds.y() + v.y)),
            None => m.clone(),
        })
        .collect()
}

/// Pixel positions relative to the primary — the anchor both directions agree on.
fn system_pixels(monitors: &[MonitorSnapshot]) -> IdMap<Point> {
    let primaries: Vec<&MonitorSnapshot> = monitors.iter().filter(|m| m.primary).collect();
    assert_eq!(primaries.len(), 1, "Single(m => m.Primary)");
    let origin = primaries[0].pixel_bounds.location();
    let mut pixels = IdMap::new();
    for m in monitors {
        pixels.insert(
            &m.id,
            Point::new(m.pixel_bounds.x() - origin.x, m.pixel_bounds.y() - origin.y),
        );
    }
    pixels
}

/// The C# sorts these lines with the default, culture-sensitive string comparer;
/// that only orders a log line, so they are left in input order here.
fn describe(placed: &[MonitorSnapshot], back: &IdMap<Point>) -> String {
    placed
        .iter()
        .map(|m| {
            format!(
                "{} mm({:.1},{:.1}) -> px({},{})",
                m.id,
                m.mm_bounds.x(),
                m.mm_bounds.y(),
                back[m.id.as_str()].x,
                back[m.id.as_str()].y
            )
        })
        .collect::<Vec<_>>()
        .join(" | ")
}

fn single<'a>(monitors: &'a [MonitorSnapshot], id: &str) -> &'a MonitorSnapshot {
    let found: Vec<&MonitorSnapshot> = monitors.iter().filter(|m| m.id == id).collect();
    assert_eq!(found.len(), 1, "Single(m => m.Id == {id:?})");
    found[0]
}

/// System pixels in, the same system pixels out. One pixel of slack: the outbound
/// direction rounds, because the system only accepts integers.
fn assert_round_trips(monitors: &[MonitorSnapshot]) {
    let placed = place_from_system(monitors, 20.0);
    let back = pixel_location::solve(&placed, DEFAULT_TOLERANCE_MM);
    let expected = system_pixels(monitors);

    println!("{}", describe(&placed, &back));

    for monitor in monitors {
        let want = expected[monitor.id.as_str()];
        let got = back[monitor.id.as_str()];

        assert!(
            (got.x - want.x).abs() <= 1.0,
            "{} X: got {}, expected {}",
            monitor.id,
            got.x,
            want.x
        );
        assert!(
            (got.y - want.y).abs() <= 1.0,
            "{} Y: got {}, expected {}",
            monitor.id,
            got.y,
            want.y
        );
    }
}

/// Two monitors share an edge the cursor can cross: equal edges on one axis with
/// overlap on the other.
fn share_an_edge(a: &Rect, b: &Rect) -> bool {
    (a.right() == b.x() || b.right() == a.x())
        && layout_geometry::overlap_on(a, b, Axis::Vertical) > 0.0
        || (a.bottom() == b.y() || b.bottom() == a.y())
            && layout_geometry::overlap_on(a, b, Axis::Horizontal) > 0.0
}

// ---------------------------------------------------------------- invertible

#[test]
fn side_by_side_round_trips() {
    assert_round_trips(&[
        monitor("A", 600.0, 340.0, 0.0, 0.0, 2560.0, 1440.0, true),
        monitor("B", 600.0, 340.0, 2560.0, 0.0, 2560.0, 1440.0, false),
    ]);
}

/// Different panel sizes, wildly different pitches, and a neighbour hanging 219 px
/// lower.
#[test]
fn mixed_pitches_and_vertical_offset_round_trips() {
    assert_round_trips(&[
        monitor("MAIN", 600.0, 340.0, 0.0, 0.0, 2560.0, 1440.0, true),
        monitor("SIDE", 600.0, 340.0, 2560.0, -219.0, 2560.0, 1440.0, false),
        monitor("TV", 1650.0, 920.0, 5120.0, 0.0, 1280.0, 720.0, false),
    ]);
}

#[test]
fn vertical_stack_of_different_widths_round_trips() {
    assert_round_trips(&[
        monitor("TOP", 600.0, 340.0, 0.0, 0.0, 2560.0, 1440.0, true),
        monitor("LAPTOP", 302.0, 189.0, 320.0, 1440.0, 1920.0, 1200.0, false),
    ]);
}

#[test]
fn primary_in_the_middle_round_trips() {
    assert_round_trips(&[
        monitor("LEFT", 520.0, 320.0, -1920.0, 0.0, 1920.0, 1080.0, false),
        monitor("MAIN", 600.0, 340.0, 0.0, 0.0, 2560.0, 1440.0, true),
        monitor("RIGHT", 700.0, 400.0, 2560.0, 0.0, 1920.0, 1080.0, false),
    ]);
}

/// Bezel widths are the one thing the system never sees. They must survive the
/// outbound trip anyway, as exact pixel contact.
#[test]
fn thick_bezels_round_trip_as_exact_pixel_contact() {
    let monitors = [
        monitor_with_bezel("A", 600.0, 340.0, 0.0, 0.0, 1920.0, 1080.0, true, 25.0),
        monitor_with_bezel("B", 600.0, 340.0, 1920.0, 0.0, 1920.0, 1080.0, false, 25.0),
    ];

    assert_round_trips(&monitors);

    let placed = place_from_system(&monitors, 20.0);
    let a = single(&placed, "A");
    let b = single(&placed, "B");

    // 50mm of frame between the two panels, and the bezels themselves flush.
    assert_equal_precision(50.0, b.mm_bounds.x() - a.mm_bounds.right(), 6);
    assert_equal_precision(a.mm_outside_bounds.right(), b.mm_outside_bounds.x(), 6);
}

/// Running "place from system" twice must give what running it once gave.
#[test]
fn placing_is_idempotent() {
    let monitors = [
        monitor("MAIN", 600.0, 340.0, 0.0, 0.0, 2560.0, 1440.0, true),
        monitor("SIDE", 600.0, 340.0, 2560.0, -219.0, 2560.0, 1440.0, false),
        monitor("TV", 1650.0, 920.0, 5120.0, 0.0, 1280.0, 720.0, false),
    ];

    let once = place_from_system(&monitors, 20.0);
    let twice = place_from_system(&once, 20.0);

    for monitor in &once {
        let again = single(&twice, &monitor.id);
        assert_equal_precision(monitor.mm_bounds.x(), again.mm_bounds.x(), 9);
        assert_equal_precision(monitor.mm_bounds.y(), again.mm_bounds.y(), 9);
    }
}

/// A 2x2 grid: D shares a pixel edge with the primary on both axes at once, a
/// corner, which does not claim; it waits for B (or C) and docks bezel-flush.
#[test]
fn diagonal_neighbour_in_a_grid_is_refused_and_the_grid_round_trips() {
    let monitors = [
        monitor("A", 600.0, 340.0, 0.0, 0.0, 1920.0, 1080.0, true),
        monitor("B", 600.0, 340.0, 1920.0, 0.0, 1920.0, 1080.0, false),
        monitor("C", 600.0, 340.0, 0.0, 1080.0, 1920.0, 1080.0, false),
        monitor("D", 600.0, 340.0, 1920.0, 1080.0, 1920.0, 1080.0, false),
    ];

    // The walk alone, before compaction: every monitor bezel-flush against its
    // neighbours, D at 620 rather than the 600 a panel-flush diagonal dock gave.
    let walked = system_location::solve(&monitors, None);
    assert_eq!(Point::new(620.0, 0.0), walked["B"]);
    assert_eq!(Point::new(0.0, 360.0), walked["C"]);
    assert_eq!(Point::new(620.0, 360.0), walked["D"]);

    // Bezels touching on all four inner edges, so compaction has nothing left to
    // separate and the grid keeps its shape.
    let placed = place_from_system(&monitors, 20.0);
    assert_eq!(Point::new(620.0, 0.0), single(&placed, "B").mm_location());
    assert_eq!(Point::new(0.0, 360.0), single(&placed, "C").mm_location());
    assert_eq!(Point::new(620.0, 360.0), single(&placed, "D").mm_location());

    assert_round_trips(&monitors);
}

// ------------------------------------------------------------ not invertible

/// A gap between two monitors in the system configuration has no physical
/// meaning: it is closed on the way in and cannot come back.
#[test]
fn pixel_gap_is_closed_and_does_not_come_back() {
    let monitors = [
        monitor("A", 600.0, 340.0, 0.0, 0.0, 1920.0, 1080.0, true),
        monitor(
            "B",
            600.0,
            340.0,
            1920.0 + 300.0,
            0.0,
            1920.0,
            1080.0,
            false,
        ),
    ];

    let placed = place_from_system(&monitors, 20.0);
    let back = pixel_location::solve(&placed, DEFAULT_TOLERANCE_MM);
    println!("{}", describe(&placed, &back));

    // Not where it was: the gap is gone.
    assert_ne!(2220.0, back["B"].x);

    // Only the alignment hints fire, leaving B on top of the primary; compaction
    // breaks the total overlap with the shortest push, vertical on two identical
    // panels. Two screens side by side come back stacked.
    assert_eq!(Point::new(0.0, 1080.0), back["B"]);

    // Whatever it chose, the desktop is connected and free of overlap.
    let a = Rect::from_location_size(back["A"], monitors[0].pixel_size());
    let b = Rect::from_location_size(back["B"], monitors[1].pixel_size());
    assert!(!layout_geometry::overlap(&a, &b));
    assert_eq!(a.bottom(), b.y());
}

/// Two monitors overlapping in the system configuration: resolved on the way in,
/// the round trip returns a layout that merely touches.
#[test]
fn pixel_overlap_is_resolved_and_does_not_come_back() {
    let monitors = [
        monitor("A", 600.0, 340.0, 0.0, 0.0, 1920.0, 1080.0, true),
        monitor("B", 600.0, 340.0, 960.0, 540.0, 1920.0, 1080.0, false),
    ];

    let placed = place_from_system(&monitors, 20.0);
    let back = pixel_location::solve(&placed, DEFAULT_TOLERANCE_MM);
    println!("{}", describe(&placed, &back));

    assert_ne!(Point::new(960.0, 540.0), back["B"]);

    let a = Rect::from_location_size(back["A"], monitors[0].pixel_size());
    let b = Rect::from_location_size(back["B"], monitors[1].pixel_size());
    assert!(
        !layout_geometry::overlap(&a, &b),
        "A{a:?} still overlaps B{b:?}"
    );
}

/// Two monitors meeting corner to corner: compaction slides the monitor into the
/// 20mm corridor, and the outbound direction only docks along a real edge.
#[test]
fn corner_to_corner_contact_is_not_invertible() {
    let monitors = [
        monitor("A", 600.0, 340.0, 0.0, 0.0, 1920.0, 1080.0, true),
        monitor("B", 600.0, 340.0, 1920.0, 1080.0, 1920.0, 1080.0, false),
    ];

    // Inbound, the walk refuses the contact on both axes, but the contact rule
    // still suggests a position: B keeps the diagonal, bezel-flush at (620,360).
    let walked = system_location::solve(&monitors, None);
    assert_eq!(Point::new(620.0, 360.0), walked["B"]);

    let placed = place_from_system(&monitors, 20.0);
    let back = pixel_location::solve(&placed, DEFAULT_TOLERANCE_MM);
    println!("{}", describe(&placed, &back));

    let a = single(&placed, "A");
    let b = single(&placed, "B");

    // Compaction keeps the frames flush on the right and slides B up until the two
    // panels share the 20mm corridor the options demand.
    assert_equal_precision(a.mm_outside_bounds.right(), b.mm_outside_bounds.x(), 6);
    assert_equal_precision(
        20.0,
        layout_geometry::overlap_on(&a.mm_bounds, &b.mm_bounds, Axis::Vertical),
        6,
    );

    // Outbound: not a corner any more. B is docked on one axis with a real shared edge.
    let pa = Rect::from_location_size(back["A"], monitors[0].pixel_size());
    let pb = Rect::from_location_size(back["B"], monitors[1].pixel_size());

    assert!(
        share_an_edge(&pa, &pb),
        "corner became {pa:?} / {pb:?}, still not crossable"
    );
    assert_ne!(Point::new(1920.0, 1080.0), back["B"]);
}

/// The same corner with `MinimalEdgeOverlap = 0`: the walk leaves B bezel-flush on
/// the diagonal and compaction has nothing to undo. Only the millimetre half
/// survives; the pixel solver refuses corners whatever the options say.
#[test]
fn corner_to_corner_contact_survives_when_no_corridor_is_demanded() {
    let monitors = [
        monitor("A", 600.0, 340.0, 0.0, 0.0, 1920.0, 1080.0, true),
        monitor("B", 600.0, 340.0, 1920.0, 1080.0, 1920.0, 1080.0, false),
    ];

    let placed = place_from_system(&monitors, 0.0);
    let back = pixel_location::solve(&placed, DEFAULT_TOLERANCE_MM);
    println!("{}", describe(&placed, &back));

    let a = single(&placed, "A");
    let b = single(&placed, "B");

    // Frames meeting at the single point (610, 350), panels still 20mm apart on each axis.
    assert_eq!(Point::new(620.0, 360.0), b.mm_location());
    assert_equal_precision(a.mm_outside_bounds.right(), b.mm_outside_bounds.x(), 6);
    assert_equal_precision(a.mm_outside_bounds.bottom(), b.mm_outside_bounds.y(), 6);
}

/// A monitor touching nothing in the system configuration is placed by compaction,
/// not by the walk, so there is nothing to invert; it must still end up part of
/// the single connected block.
#[test]
fn detached_monitor_is_docked_rather_than_inverted() {
    let monitors = [
        monitor("MAIN", 600.0, 340.0, 0.0, 0.0, 2560.0, 1440.0, true),
        monitor("FAR", 520.0, 300.0, 6000.0, 4000.0, 1920.0, 1080.0, false),
    ];

    let placed = place_from_system(&monitors, 20.0);
    let back = pixel_location::solve(&placed, DEFAULT_TOLERANCE_MM);
    println!("{}", describe(&placed, &back));

    assert_ne!(Point::new(6000.0, 4000.0), back["FAR"]);

    let main = Rect::from_location_size(back["MAIN"], monitors[0].pixel_size());
    let far = Rect::from_location_size(back["FAR"], monitors[1].pixel_size());
    assert!(!layout_geometry::overlap(&main, &far));

    assert!(
        share_an_edge(&main, &far),
        "island left disconnected: {main:?} / {far:?}"
    );
}

/// A monitor reporting no pixels has no pitch: the projection cannot run for it,
/// and it must neither produce a NaN position nor take the layout down.
#[test]
fn monitor_without_pixels_is_placed_without_projection() {
    let monitors = [
        monitor("MAIN", 600.0, 340.0, 0.0, 0.0, 2560.0, 1440.0, true),
        monitor("GHOST", 520.0, 300.0, 2560.0, 0.0, 0.0, 0.0, false),
    ];

    let placed = place_from_system(&monitors, 20.0);
    println!(
        "{}",
        placed
            .iter()
            .map(|m| format!("{}({:.1},{:.1})", m.id, m.mm_bounds.x(), m.mm_bounds.y()))
            .collect::<Vec<_>>()
            .join(" | ")
    );

    for monitor in &placed {
        assert!(!monitor.mm_bounds.x().is_nan(), "{} X is NaN", monitor.id);
        assert!(!monitor.mm_bounds.y().is_nan(), "{} Y is NaN", monitor.id);
    }
}

/// No primary yet: nothing is anchored, so nothing may move.
#[test]
fn no_primary_moves_nothing() {
    let solved = system_location::solve(
        &[
            monitor("A", 600.0, 340.0, 0.0, 0.0, 2560.0, 1440.0, false),
            monitor("B", 520.0, 300.0, 2560.0, 0.0, 1920.0, 1080.0, false),
        ],
        None,
    );

    assert!(solved.is_empty());
}

/// Monitors not offered for placement neither move nor anchor anything: the
/// `placeAll: false` path the platform factories take on startup.
#[test]
fn monitors_not_offered_for_placement_stay_put() {
    let monitors = [
        monitor("MAIN", 600.0, 340.0, 0.0, 0.0, 2560.0, 1440.0, true),
        monitor("SIDE", 600.0, 340.0, 2560.0, -219.0, 2560.0, 1440.0, false),
    ];

    let solved = system_location::solve(&monitors, Some(&HashSet::new()));
    assert!(solved.is_empty());

    // Offered, and then it does move.
    let side: HashSet<String> = ["SIDE".to_owned()].into();
    assert!(system_location::solve(&monitors, Some(&side)).contains_key("SIDE"));
}
