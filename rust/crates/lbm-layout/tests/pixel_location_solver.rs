//! `PixelLocationSolverTests.cs`: millimetres to integer system pixels.

use lbm_layout::geo::dotnet::{max, min};
use lbm_layout::geo::{Point, Rect};
use lbm_layout::solve::pixel_location::{self, DEFAULT_TOLERANCE_MM};
use lbm_layout::solve::{IdMap, MonitorSnapshot};

const BEZEL: f64 = 20.0;

/// A monitor whose panel sits at (x, y) mm with uniform bezels. Only the size of
/// the pixel rect matters to this solver, so the pixel origin is left at zero.
#[allow(clippy::too_many_arguments)]
fn monitor(
    id: &str,
    x: f64,
    y: f64,
    width_mm: f64,
    height_mm: f64,
    pixel_width: f64,
    pixel_height: f64,
    primary: bool,
) -> MonitorSnapshot {
    MonitorSnapshot::new(
        id,
        Rect::new(x, y, width_mm, height_mm),
        Rect::new(
            x - BEZEL,
            y - BEZEL,
            width_mm + 2.0 * BEZEL,
            height_mm + 2.0 * BEZEL,
        ),
        Rect::new(0.0, 0.0, pixel_width, pixel_height),
        primary,
    )
}

fn solve(monitors: &[MonitorSnapshot]) -> IdMap<Point> {
    pixel_location::solve(monitors, DEFAULT_TOLERANCE_MM)
}

fn rect_of(m: &MonitorSnapshot, solved: &IdMap<Point>) -> Rect {
    Rect::from_location_size(solved[m.id.as_str()], m.pixel_size())
}

fn assert_no_overlap(monitors: &[MonitorSnapshot], solved: &IdMap<Point>) {
    for i in 0..monitors.len() {
        for j in i + 1..monitors.len() {
            let a = rect_of(&monitors[i], solved);
            let b = rect_of(&monitors[j], solved);
            let overlap_x = min(a.right(), b.right()) - max(a.x(), b.x());
            let overlap_y = min(a.bottom(), b.bottom()) - max(a.y(), b.y());
            assert!(
                !(overlap_x > 0.0 && overlap_y > 0.0),
                "{}{a:?} overlaps {}{b:?}",
                monitors[i].id,
                monitors[j].id
            );
        }
    }
}

#[test]
fn dual_monitor_side_by_side() {
    // Two 1920x1080 panels of 480x270mm, bezels touching.
    let a = monitor("A", 0.0, 0.0, 480.0, 270.0, 1920.0, 1080.0, true);
    let b = monitor(
        "B",
        480.0 + 2.0 * BEZEL,
        0.0,
        480.0,
        270.0,
        1920.0,
        1080.0,
        false,
    );

    let solved = solve(&[a, b]);

    assert_eq!(Point::new(0.0, 0.0), solved["A"]);
    assert_eq!(Point::new(1920.0, 0.0), solved["B"]);
}

#[test]
fn round_trip_from_system_is_stable() {
    // Mm layout equivalent to system rects (0,0,1920,1080) and (1920,0,1920,1080):
    // re-solving must return exactly the original pixels.
    let a = monitor("A", 0.0, 0.0, 480.0, 270.0, 1920.0, 1080.0, true);
    let b = monitor(
        "B",
        480.0 + 2.0 * BEZEL,
        0.0,
        480.0,
        270.0,
        1920.0,
        1080.0,
        false,
    );

    let solved = solve(&[b, a]);

    assert_eq!(Point::new(0.0, 0.0), solved["A"]);
    assert_eq!(Point::new(1920.0, 0.0), solved["B"]);
}

#[test]
fn different_scales_share_crossing_point() {
    // 27" 4K at scale 2 (logical 1920x1080, panel 597.7x336.2mm) next to a 24" FHD
    // (531x299mm), physically centered vertically.
    let a = monitor("A", 0.0, 0.0, 597.7, 336.2, 1920.0, 1080.0, true);
    let b_y = (336.2 - 299.0) / 2.0;
    let b = monitor(
        "B",
        597.7 + 2.0 * BEZEL,
        b_y,
        531.0,
        299.0,
        1920.0,
        1080.0,
        false,
    );

    let solved = solve(&[a, b]);

    assert_eq!(Point::new(0.0, 0.0), solved["A"]);
    assert_eq!(1920.0, solved["B"].x);

    // The physical midpoint of the shared span projects to the same pixel Y on both
    // sides (±1 for rounding).
    let mid = (max(0.0, b_y) + min(336.2, b_y + 299.0)) / 2.0;
    let y_on_a = solved["A"].y + mid / (336.2 / 1080.0);
    let y_on_b = solved["B"].y + (mid - b_y) / (299.0 / 1080.0);
    assert!(
        (y_on_a - y_on_b).abs() <= 1.0,
        "crossing mismatch: {y_on_a} vs {y_on_b}"
    );
}

#[test]
fn grid_two_by_two() {
    let pitch = 480.0 + 2.0 * BEZEL; // panel + bezels, horizontal
    let pitch_v = 270.0 + 2.0 * BEZEL;

    let monitors = [
        monitor("A", 0.0, 0.0, 480.0, 270.0, 1920.0, 1080.0, true),
        monitor("B", pitch, 0.0, 480.0, 270.0, 1920.0, 1080.0, false),
        monitor("C", 0.0, pitch_v, 480.0, 270.0, 1920.0, 1080.0, false),
        monitor("D", pitch, pitch_v, 480.0, 270.0, 1920.0, 1080.0, false),
    ];

    let solved = solve(&monitors);

    assert_eq!(Point::new(0.0, 0.0), solved["A"]);
    assert_eq!(Point::new(1920.0, 0.0), solved["B"]);
    assert_eq!(Point::new(0.0, 1080.0), solved["C"]);
    // D is constrained through two BFS paths; both must agree here.
    assert_eq!(Point::new(1920.0, 1080.0), solved["D"]);
    assert_no_overlap(&monitors, &solved);
}

#[test]
fn small_physical_gap_counts_as_adjacent() {
    // 3mm of air between bezels: below tolerance, still exact pixel contact.
    let a = monitor("A", 0.0, 0.0, 480.0, 270.0, 1920.0, 1080.0, true);
    let b = monitor(
        "B",
        480.0 + 2.0 * BEZEL + 3.0,
        0.0,
        480.0,
        270.0,
        1920.0,
        1080.0,
        false,
    );

    let solved = solve(&[a, b]);

    assert_eq!(1920.0, solved["B"].x);
    assert_eq!(0.0, solved["B"].y);
}

#[test]
fn detached_island_snaps_into_contact() {
    // 500mm of air, diagonal offset: not adjacent, must be pulled into contact.
    let monitors = [
        monitor("A", 0.0, 0.0, 480.0, 270.0, 1920.0, 1080.0, true),
        monitor(
            "B",
            480.0 + 2.0 * BEZEL + 500.0,
            400.0,
            480.0,
            270.0,
            1920.0,
            1080.0,
            false,
        ),
    ];

    let solved = solve(&monitors);

    let a = rect_of(&monitors[0], &solved);
    let b = rect_of(&monitors[1], &solved);

    assert_no_overlap(&monitors, &solved);

    // Touching = zero distance on exactly one axis with overlap on the other.
    let touch_x = (b.x() == a.right() || a.x() == b.right())
        && min(a.bottom(), b.bottom()) - max(a.y(), b.y()) > 0.0;
    let touch_y = (b.y() == a.bottom() || a.y() == b.bottom())
        && min(a.right(), b.right()) - max(a.x(), b.x()) > 0.0;
    assert!(touch_x || touch_y, "island not in contact: A{a:?} B{b:?}");
}

/// Several detached islands, each snapped against everything placed before it,
/// must still end up as one connected desktop.
#[test]
fn several_detached_islands_all_end_up_connected() {
    let monitors = [
        monitor("A", 0.0, 0.0, 480.0, 270.0, 1920.0, 1080.0, true),
        // Adjacent: placed by the walk, so the anchored group starts as a column.
        monitor(
            "B",
            0.0,
            270.0 + 2.0 * BEZEL,
            480.0,
            270.0,
            1920.0,
            1080.0,
            false,
        ),
        // Islands, diagonally scattered so the placed group grows an irregular outline.
        monitor("C", 1600.0, -1200.0, 480.0, 270.0, 1920.0, 1080.0, false),
        monitor("D", 1500.0, 900.0, 480.0, 270.0, 1920.0, 1080.0, false),
        monitor("E", 3000.0, -200.0, 480.0, 270.0, 1920.0, 1080.0, false),
    ];

    let solved = solve(&monitors);

    assert_no_overlap(&monitors, &solved);
    assert_connected(&monitors, &solved);
}

/// Every monitor reachable from every other through real shared edges.
fn assert_connected(monitors: &[MonitorSnapshot], solved: &IdMap<Point>) {
    let rects: Vec<Rect> = monitors.iter().map(|m| rect_of(m, solved)).collect();

    fn touch(a: &Rect, b: &Rect) -> bool {
        (a.x() == b.right() || b.x() == a.right())
            && min(a.bottom(), b.bottom()) - max(a.y(), b.y()) > 0.0
            || (a.y() == b.bottom() || b.y() == a.bottom())
                && min(a.right(), b.right()) - max(a.x(), b.x()) > 0.0
    }

    let mut seen = vec![rects[0]];
    let mut todo: Vec<Rect> = rects[1..].to_vec();

    let mut grown = true;
    while grown {
        grown = false;
        for i in (0..todo.len()).rev() {
            if !seen.iter().any(|s| touch(s, &todo[i])) {
                continue;
            }
            seen.push(todo[i]);
            todo.remove(i);
            grown = true;
        }
    }

    assert!(
        todo.is_empty(),
        "disconnected desktop: {}",
        monitors
            .iter()
            .map(|m| format!("{}{:?}", m.id, rect_of(m, solved)))
            .collect::<Vec<_>>()
            .join(" ")
    );
}

#[test]
fn primary_always_at_origin() {
    // Primary in the middle of a row, listed last.
    let left = monitor("L", 0.0, 0.0, 480.0, 270.0, 1920.0, 1080.0, false);
    let main = monitor(
        "M",
        480.0 + 2.0 * BEZEL,
        0.0,
        480.0,
        270.0,
        1920.0,
        1080.0,
        true,
    );
    let right = monitor(
        "R",
        2.0 * (480.0 + 2.0 * BEZEL),
        0.0,
        480.0,
        270.0,
        1920.0,
        1080.0,
        false,
    );

    let solved = solve(&[left, right, main]);

    assert_eq!(Point::new(0.0, 0.0), solved["M"]);
    assert_eq!(Point::new(-1920.0, 0.0), solved["L"]);
    assert_eq!(Point::new(1920.0, 0.0), solved["R"]);
}

#[test]
fn vertically_stacked_with_different_widths() {
    // Laptop panel under a wide monitor, horizontally centered in mm.
    let a = monitor("A", 0.0, 0.0, 597.7, 336.2, 2560.0, 1440.0, true);
    let b_x = (597.7 - 302.0) / 2.0;
    let b = monitor(
        "B",
        b_x,
        336.2 + 2.0 * BEZEL,
        302.0,
        189.0,
        1920.0,
        1200.0,
        false,
    );

    let solved = solve(&[a, b]);

    assert_eq!(1440.0, solved["B"].y);

    let mid = (max(0.0, b_x) + min(597.7, b_x + 302.0)) / 2.0;
    let x_on_a = solved["A"].x + mid / (597.7 / 2560.0);
    let x_on_b = solved["B"].x + (mid - b_x) / (302.0 / 1920.0);
    assert!(
        (x_on_a - x_on_b).abs() <= 1.0,
        "crossing mismatch: {x_on_a} vs {x_on_b}"
    );
}
