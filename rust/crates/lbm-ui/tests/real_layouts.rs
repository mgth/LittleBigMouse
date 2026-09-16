//! The map, on the desktops the oracle pins.
//!
//! Everything else in this crate feeds the map rectangles I typed myself, and rectangles
//! I typed are rectangles I already believed in: two screens, side by side, round
//! numbers, a corner at the origin. Real desktops are none of those. They start at
//! negative millimetres, mix a 1110 mm television with a 300 mm laptop panel, carry
//! fractional heights out of a scale of 1.25, and come nine at a time.
//!
//! So the layouts here are built by the real pipeline from the real scenario inputs —
//! `lbm_layout::linux::populate`, the same call the agent makes — and the extents are
//! compared against what the C# recorded in `expected/layout.json`.
//!
//! Only the scenarios with no `store` in their input are used. The others need
//! `lbm-store` to reproduce their goldens, and the map has no business depending on the
//! persistence engine to be tested; these nine place and anchor from the detected
//! outputs alone, so `populate` with a loader that does nothing reproduces them exactly.

use lbm_layout::geo::Rect;
use lbm_layout::linux::{LinuxEdid, LinuxMonitor};
use lbm_layout::model::{Layout, LayoutOptions};
use lbm_ui::map::{self, MapMonitor, MARGIN};
use serde_json::Value;

/// The scenarios whose `input.json` has no `store`, with the number of monitors each
/// ends up with — the count is here so that a scenario quietly losing a screen shows up
/// as a failure rather than as a smaller test.
const SCENARIOS: &[(&str, usize)] = &[
    ("grid-2x2", 4),
    ("laptop-tv", 3),
    ("negative-positions", 3),
    ("nine-monitors-long-id", 9),
    ("portrait-rotated", 2),
    ("single-1080p-fresh", 1),
    ("six-monitors", 6),
    ("three-screens-mixed-scale", 3),
];

fn repo_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .expect("the repository root is three levels above the crate")
        .to_path_buf()
}

fn json(path: &std::path::Path) -> Value {
    serde_json::from_str(
        &std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display())),
    )
    .unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// The reader `domain_oracle_pipeline.rs` uses, kept to the members `AddMonitor` reads.
fn linux_monitor(v: &Value) -> LinuxMonitor {
    let f = |k: &str| v[k].as_f64().unwrap_or_else(|| panic!("{k}"));
    let i = |k: &str| v[k].as_i64().unwrap_or_else(|| panic!("{k}")) as i32;
    let b = |k: &str| v[k].as_bool().unwrap_or_else(|| panic!("{k}"));
    let s = |v: &Value| v.as_str().map(str::to_owned);
    let edid = match &v["Edid"] {
        Value::Null => None,
        e => Some(LinuxEdid {
            manufacturer_code: s(&e["ManufacturerCode"]),
            product_code: s(&e["ProductCode"]),
            serial: s(&e["Serial"]),
            serial_number: s(&e["SerialNumber"]),
            model: s(&e["Model"]),
            physical_width: e["PhysicalWidth"].as_f64().unwrap(),
            physical_height: e["PhysicalHeight"].as_f64().unwrap(),
            video_interface: s(&e["VideoInterface"]),
        }),
    };
    LinuxMonitor {
        connector_name: v["ConnectorName"].as_str().unwrap().to_owned(),
        logical_x: f("LogicalX"),
        logical_y: f("LogicalY"),
        logical_width: f("LogicalWidth"),
        logical_height: f("LogicalHeight"),
        pixel_width: i("PixelWidth"),
        pixel_height: i("PixelHeight"),
        scale: f("Scale"),
        width_mm: f("WidthMm"),
        height_mm: f("HeightMm"),
        primary: b("Primary"),
        enabled: b("Enabled"),
        orientation: i("Orientation"),
        frequency: i("Frequency"),
        edid,
    }
}

/// What the map needs, owned, because `MapMonitor` borrows its strings.
struct Screen {
    id: String,
    mm_outside: Rect,
    mm_content: Rect,
}

/// A scenario's monitors as the map sees them, and the extent the C# recorded for it.
fn desktop(scenario: &str) -> (Vec<Screen>, Rect) {
    let dir = repo_root().join("domain-oracle/scenarios").join(scenario);
    let input = json(&dir.join("input.json"));
    let monitors: Vec<LinuxMonitor> = input["displays"]
        .as_array()
        .expect("displays")
        .iter()
        .map(linux_monitor)
        .collect();

    let mut layout = Layout::new(LayoutOptions::default());
    lbm_layout::linux::populate(&mut layout, &monitors, |_| {
        Ok::<(), std::convert::Infallible>(())
    })
    .expect("a loader that does nothing cannot fail");

    let screens = layout
        .monitors()
        .iter()
        .filter_map(|m| {
            // `None` when the monitor has no active source — a disabled output is not
            // on the map, which is the same rule the C# presenter applies.
            let p = layout.depth_projection(m)?;
            Some(Screen {
                id: m.id.clone(),
                mm_outside: p.outside_bounds(),
                mm_content: p.bounds(),
            })
        })
        .collect();

    let golden = json(&dir.join("expected/layout.json"));
    let g = &golden["PhysicalBounds"];
    let extent = Rect::new(
        g["X"].as_f64().unwrap(),
        g["Y"].as_f64().unwrap(),
        g["Width"].as_f64().unwrap(),
        g["Height"].as_f64().unwrap(),
    );
    (screens, extent)
}

fn as_map(screens: &[Screen]) -> Vec<MapMonitor<'_>> {
    screens
        .iter()
        .map(|s| MapMonitor {
            id: &s.id,
            name: &s.id,
            mm_outside: s.mm_outside,
            mm_content: s.mm_content,
            logo: None,
            wallpaper: None,
            details: &[],
        })
        .collect()
}

fn window(w: f32, h: f32) -> egui::Rect {
    egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(w, h))
}

/// The map's own union has to be the layout's, on every real desktop — otherwise one of
/// the two is wrong and the map would be fitting a box the model does not agree with.
#[test]
fn the_maps_extent_is_the_one_the_layout_publishes() {
    for (scenario, count) in SCENARIOS {
        let (screens, golden) = desktop(scenario);
        assert_eq!(screens.len(), *count, "{scenario}: monitors on the map");

        let mine = map::extent(&as_map(&screens));
        for (got, want, axis) in [
            (mine.left(), golden.left(), "left"),
            (mine.top(), golden.top(), "top"),
            (mine.width(), golden.width(), "width"),
            (mine.height(), golden.height(), "height"),
        ] {
            assert!(
                (got - want).abs() < 1e-9,
                "{scenario}: {axis} is {got}, the C# recorded {want}"
            );
        }
    }
}

/// A fit that does not fit is the one thing this module cannot get wrong. Nine screens,
/// a corner at -682 mm, a desktop twice as wide as it is tall in a window that is the
/// other way round: every frame still has to land inside the margin.
#[test]
fn every_screen_of_every_desktop_lands_inside_the_window() {
    let shapes = [
        window(800.0, 600.0),
        window(1600.0, 400.0),
        window(400.0, 1200.0),
        window(200.0, 180.0),
    ];

    for (scenario, _) in SCENARIOS {
        let (screens, _) = desktop(scenario);
        let monitors = as_map(&screens);
        let extent = map::extent(&monitors);

        for win in shapes {
            let fit = map::fit(extent, win);
            let box_ = win.shrink(MARGIN);
            for m in &monitors {
                let drawn = fit.place(m).outside;
                // A tenth of a point of slack: the rects are f32 out of f64 millimetres.
                assert!(
                    drawn.left() >= box_.left() - 0.1
                        && drawn.top() >= box_.top() - 0.1
                        && drawn.right() <= box_.right() + 0.1
                        && drawn.bottom() <= box_.bottom() + 0.1,
                    "{scenario} in {:?}: {} was drawn at {drawn:?}, outside {box_:?}",
                    win.size(),
                    m.id
                );
            }
        }
    }
}

/// Inside is not enough: a ratio of nearly nothing would also be inside. The desktop has
/// to *reach* the box on the axis that binds it, or the map is smaller than it could be.
#[test]
fn the_desktop_reaches_the_edge_of_the_box_on_one_axis() {
    for (scenario, _) in SCENARIOS {
        let (screens, _) = desktop(scenario);
        let monitors = as_map(&screens);
        let extent = map::extent(&monitors);
        let win = window(1000.0, 700.0);
        let box_ = win.shrink(MARGIN);
        let fit = map::fit(extent, win);

        let width = (extent.width() * fit.ratio) as f32;
        let height = (extent.height() * fit.ratio) as f32;
        assert!(
            (width - box_.width()).abs() < 0.1 || (height - box_.height()).abs() < 0.1,
            "{scenario}: {width}x{height} drawn in a box of {}x{}",
            box_.width(),
            box_.height()
        );
    }
}

/// `laptop-tv` is a 1110 mm television beside a 300 mm laptop panel. The small one is
/// where a map that scales badly loses a screen: it collapses to nothing, or its name
/// is dropped while there is still room for it.
#[test]
fn the_small_screen_of_a_lopsided_desktop_is_still_a_screen() {
    let (screens, _) = desktop("laptop-tv");
    let monitors = as_map(&screens);
    let fit = map::fit(map::extent(&monitors), window(1200.0, 900.0));

    let smallest = monitors
        .iter()
        .min_by(|a, b| {
            let area = |m: &MapMonitor| m.mm_outside.width() * m.mm_outside.height();
            area(a).partial_cmp(&area(b)).unwrap()
        })
        .expect("three screens");

    let drawn = fit.place(smallest);
    assert!(
        drawn.outside.width() > 1.0 && drawn.outside.height() > 1.0,
        "the small screen collapsed: {:?}",
        drawn.outside
    );
    assert!(
        drawn.content.width() < drawn.outside.width(),
        "its bezel did not survive the scaling"
    );
    assert!(
        drawn.name_height.is_some(),
        "there was room to name it and it was not named"
    );
    // And it is a target: the whole point of drawing it is that it can be picked.
    assert_eq!(
        map::hit(&monitors, &fit, drawn.outside.center()),
        Some(smallest.id)
    );
}
