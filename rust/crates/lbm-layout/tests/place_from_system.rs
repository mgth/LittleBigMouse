//! `PlaceFromSystemTests.cs`: placing monitors from the system's pixel
//! configuration, through `Layout::set_locations_from_system_configuration`.
//!
//! Two ways in, and they had better agree: a first run with nothing stored
//! places everything from scratch, and the "Place from system config" menu item
//! does the same thing on demand.
//!
//! `ITestOutputHelper.WriteLine` is `println!`, which the test harness shows for
//! a failing test.

mod common;

use common::{add_with_source, design_options, model};
use lbm_layout::collation::invariant_compare;
use lbm_layout::geo::dotnet::compare;
use lbm_layout::geo::{Point, Rect};
use lbm_layout::model::{DisplaySize, DisplaySource, Layout, Monitor, PhysicalSource};
use lbm_layout::solve::pixel_location::{self, DEFAULT_TOLERANCE_MM};
use lbm_layout::solve::MonitorSnapshot;

/// `AddMonitor(layout, id, widthMm, heightMm, pixelX, pixelY, pixelWidth,
/// pixelHeight, primary)`: a borderless `PNP_{id}` model, a monitor showing one
/// attached source, both handed to the layout. No position: every monitor starts
/// where a freshly built layout starts them, at the origin.
#[allow(clippy::too_many_arguments)]
fn add_monitor(
    layout: &mut Layout,
    id: &str,
    width_mm: f64,
    height_mm: f64,
    pixel_x: f64,
    pixel_y: f64,
    pixel_width: f64,
    pixel_height: f64,
    primary: bool,
) {
    let model = model(layout, &format!("PNP_{id}"), width_mm, height_mm, Some(0.0));
    let monitor = Monitor::new(id, &model);
    let mut source = DisplaySource::new(format!("{id}-src"));
    source.attached_to_desktop = true;
    source.primary = primary;
    source.in_pixel =
        DisplaySize::from_rect(Rect::new(pixel_x, pixel_y, pixel_width, pixel_height));
    let physical_source = PhysicalSource::new(format!("{id}-dev"), id, source);
    add_with_source(layout, monitor, physical_source, true);
}

/// Three monitors side by side in pixels, of different sizes and DPI — the shape
/// LittleBigMouse exists for.
fn three_in_a_row() -> Layout {
    let mut layout = Layout::new(design_options());
    add_monitor(
        &mut layout,
        "LEFT",
        520.0,
        320.0,
        -1920.0,
        0.0,
        1920.0,
        1080.0,
        false,
    );
    add_monitor(
        &mut layout,
        "MAIN",
        600.0,
        340.0,
        0.0,
        0.0,
        2560.0,
        1440.0,
        true,
    );
    add_monitor(
        &mut layout,
        "RIGHT",
        700.0,
        400.0,
        2560.0,
        0.0,
        1920.0,
        1080.0,
        false,
    );
    layout
}

/// A real desktop: two 1440p panels side by side with the second hanging 219 px
/// lower, and a 1280x720 TV — big and coarse — to their right.
fn real_desktop() -> Layout {
    let mut layout = Layout::new(design_options());
    add_monitor(
        &mut layout,
        "MAIN",
        600.0,
        340.0,
        0.0,
        0.0,
        2560.0,
        1440.0,
        true,
    );
    add_monitor(
        &mut layout,
        "SIDE",
        600.0,
        340.0,
        2560.0,
        -219.0,
        2560.0,
        1440.0,
        false,
    );
    add_monitor(
        &mut layout,
        "TV",
        1650.0,
        920.0,
        5120.0,
        0.0,
        1280.0,
        720.0,
        false,
    );
    layout
}

fn projection(layout: &Layout, id: &str) -> DisplaySize {
    layout
        .depth_projection(layout.monitor(id).unwrap())
        .expect("every monitor here shows a source")
}

/// `Describe`: `{Id}@({X:F1},{Y:F1})` per monitor, ordered by id with the
/// culture comparer, joined with " | ". Rust's `{:.1}` rounds exact ties to even
/// where .NET's "F1" rounds them away from zero; the tests only compare two
/// descriptions with each other.
fn describe(layout: &Layout) -> String {
    let mut monitors: Vec<_> = layout.monitors().iter().collect();
    monitors.sort_by(|a, b| invariant_compare(&a.id, &b.id));
    monitors
        .iter()
        .map(|m| {
            let dp = layout.depth_projection(m).unwrap();
            format!("{}@({:.1},{:.1})", m.id, dp.x, dp.y)
        })
        .collect::<Vec<_>>()
        .join(" | ")
}

/// C#: `PlaceFromSystemTests.PlacingTwiceLandsWherePlacingOnceDid_OnARealDesktop`.
#[test]
fn placing_twice_lands_where_placing_once_did_on_a_real_desktop() {
    let mut layout = real_desktop();

    layout.set_locations_from_system_configuration(true);
    let once = describe(&layout);
    println!("once:  {once}");

    layout.set_locations_from_system_configuration(true);
    let twice = describe(&layout);
    println!("twice: {twice}");

    assert_eq!(once, twice);
}

/// C#: `PlaceFromSystemTests.ARealDesktopKeepsItsOrderAndItsOffset`.
#[test]
fn a_real_desktop_keeps_its_order_and_its_offset() {
    let mut layout = real_desktop();
    layout.set_locations_from_system_configuration(true);
    println!("{}", describe(&layout));

    let main = projection(&layout, "MAIN");
    let side = projection(&layout, "SIDE");
    let tv = projection(&layout, "TV");

    assert!(main.x < side.x, "MAIN left of SIDE: {}", describe(&layout));
    assert!(side.x < tv.x, "SIDE left of TV: {}", describe(&layout));

    // SIDE's top edge is 219 px above MAIN's (Windows Y grows downward). On a panel
    // 1440 px tall and 340 mm high that is 219 * 340 / 1440 = 51.7 mm up — the two
    // screens still overlap over most of their height. Anything near a full screen
    // height means the offset was not carried across at all.
    let expected = -219.0 * 340.0 / 1440.0;
    let actual = side.y - main.y;
    println!("offset: expected {expected:.1} mm, got {actual:.1} mm");

    assert!(
        (actual - expected).abs() < 5.0,
        "SIDE sits {actual:.1} mm from MAIN, expected {expected:.1} mm: {}",
        describe(&layout)
    );
}

/// C#: `PlaceFromSystemTests.PlacingFromTheSystemAndBackIsANoOp`.
#[test]
fn placing_from_the_system_and_back_is_a_no_op() {
    // The property the perpendicular rule exists for: place from the system, push
    // the result back to the system, and land on the pixel configuration you
    // started from.
    let mut layout = real_desktop();
    layout.set_locations_from_system_configuration(true);
    println!("{}", describe(&layout));

    // `m.Snapshot(m.ActiveSource.Source.Primary)`, then `PixelLocationSolver.Solve`
    // with its default tolerance.
    let source_of = |m: &Monitor| {
        layout
            .source(m.active_source.as_deref().unwrap())
            .unwrap()
            .source
            .clone()
    };
    let snapshots: Vec<MonitorSnapshot> = layout
        .monitors()
        .iter()
        .map(|m| {
            let dp = layout.depth_projection(m).unwrap();
            let source = source_of(m);
            MonitorSnapshot::new(
                m.id.clone(),
                dp.bounds(),
                dp.outside_bounds(),
                source.in_pixel.bounds(),
                source.primary,
            )
        })
        .collect();
    let solved = pixel_location::solve(&snapshots, DEFAULT_TOLERANCE_MM);

    // Both sides are anchored on the primary, so compare against the system's own
    // pixel positions relative to it.
    let primaries: Vec<&Monitor> = layout
        .monitors()
        .iter()
        .filter(|m| source_of(m).primary)
        .collect();
    assert_eq!(primaries.len(), 1, "`Single` primary");
    let primary_px = source_of(primaries[0]).in_pixel.bounds();

    for monitor in layout.monitors() {
        let px = source_of(monitor).in_pixel.bounds();
        let expected = Point::new(px.x() - primary_px.x(), px.y() - primary_px.y());
        let actual = solved[monitor.id.as_str()];
        println!(
            "{}: system ({},{}) -> solved ({},{})",
            monitor.id, expected.x, expected.y, actual.x, actual.y
        );

        assert!(
            (actual.x - expected.x).abs() <= 1.0,
            "{} X: {} vs {}",
            monitor.id,
            actual.x,
            expected.x
        );
        assert!(
            (actual.y - expected.y).abs() <= 1.0,
            "{} Y: {} vs {}",
            monitor.id,
            actual.y,
            expected.y
        );
    }
}

/// C#: `PlaceFromSystemTests.PlacingTwiceLandsWherePlacingOnceDid`.
#[test]
fn placing_twice_lands_where_placing_once_did() {
    // The startup path places from scratch; the menu item places again over the
    // result. Both call the same method, so the only way they can disagree is if
    // the method's answer depends on where the monitors already were.
    let mut layout = three_in_a_row();

    layout.set_locations_from_system_configuration(true);
    let once = describe(&layout);
    println!("once:  {once}");

    layout.set_locations_from_system_configuration(true);
    let twice = describe(&layout);
    println!("twice: {twice}");

    assert_eq!(once, twice);
}

/// C#: `PlaceFromSystemTests.PlacingKeepsThePixelOrderOfTheDesktop`.
#[test]
fn placing_keeps_the_pixel_order_of_the_desktop() {
    // Whatever the millimetre widths, three monitors laid out left to right in the
    // system must come out left to right here.
    let mut layout = three_in_a_row();

    layout.set_locations_from_system_configuration(true);
    println!("{}", describe(&layout));

    let left = projection(&layout, "LEFT");
    let main = projection(&layout, "MAIN");
    let right = projection(&layout, "RIGHT");

    assert!(
        left.x < main.x,
        "LEFT must stay left of MAIN: {}",
        describe(&layout)
    );
    assert!(
        main.x < right.x,
        "MAIN must stay left of RIGHT: {}",
        describe(&layout)
    );
}

/// C#: `PlaceFromSystemTests.PlacingLeavesNoOverlapAndNoGap`.
#[test]
fn placing_leaves_no_overlap_and_no_gap() {
    // Monitors adjacent in pixels are adjacent in millimetres: edge to edge, which
    // is the whole point of deriving the physical layout from the system one.
    let mut layout = three_in_a_row();

    layout.set_locations_from_system_configuration(true);
    println!("{}", describe(&layout));

    // `OrderBy(m => m.DepthProjection.X)`: stable, `double.CompareTo` order.
    let mut ordered: Vec<(&str, Rect, f64)> = layout
        .monitors()
        .iter()
        .map(|m| {
            let dp = layout.depth_projection(m).unwrap();
            (m.id.as_str(), dp.outside_bounds(), dp.x)
        })
        .collect();
    ordered.sort_by(|a, b| compare(a.2, b.2));
    for i in 1..ordered.len() {
        let (previous_id, previous, _) = ordered[i - 1];
        let (id, current, _) = ordered[i];
        assert!(
            (current.left() - previous.right()).abs() < 0.001,
            "{id} should touch {previous_id}: {}",
            describe(&layout)
        );
    }
}
