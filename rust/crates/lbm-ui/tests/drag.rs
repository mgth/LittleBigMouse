//! Dragging a screen: the gesture as egui really reports it, and the drop as a real
//! layout really takes it.
//!
//! The arithmetic of the snap is tested beside the code, on rectangles. What is asked
//! here is what arithmetic cannot answer:
//!
//! * that a press, a move and a release on a frame come back as a drag at all — the
//!   press/drag decision, `total_drag_delta` and `drag_stopped` are egui's, not mine, and
//!   a module that got them wrong would still pass every unit test in `drag.rs`;
//! * that dropping a screen on a **real** desktop leaves a layout that makes sense —
//!   the primary rule and the compaction are where a drop stops being arithmetic.

use egui_kittest::kittest::Queryable;
use egui_kittest::Harness;
use lbm_layout::geo::Rect;
use lbm_layout::linux::{LinuxEdid, LinuxMonitor};
use lbm_layout::model::{Layout, LayoutOptions};
use lbm_ui::drag;
use lbm_ui::map::{self, MapMonitor};
use serde_json::Value;

//==========================================================================//
// The gesture                                                              //
//==========================================================================//

fn two_screens() -> Vec<MapMonitor<'static>> {
    vec![
        MapMonitor {
            id: "left",
            name: "Left screen",
            mm_outside: Rect::new(0.0, 0.0, 640.0, 380.0),
            mm_content: Rect::new(20.0, 20.0, 600.0, 340.0),
            logo: None,
            wallpaper: None,
        },
        MapMonitor {
            id: "right",
            name: "Right screen",
            mm_outside: Rect::new(700.0, 0.0, 640.0, 380.0),
            mm_content: Rect::new(720.0, 20.0, 600.0, 340.0),
            logo: None,
            wallpaper: None,
        },
    ]
}

/// Every gesture the map reported, in order, owned so the harness can be dropped.
#[derive(Debug, PartialEq)]
enum Said {
    Clicked(String),
    Dragged(String, egui::Vec2),
    Dropped(String),
}

fn watching<'a>(log: &'a std::cell::RefCell<Vec<Said>>) -> Harness<'a> {
    let screens = two_screens();
    Harness::builder()
        .with_size(egui::vec2(1200.0, 900.0))
        .build_ui(move |ui| {
            let fit = map::fit(map::extent(&screens), ui.max_rect());
            if let Some(gesture) = map::draw(ui, &screens, &fit, None) {
                log.borrow_mut().push(match gesture {
                    map::Gesture::Clicked(id) => Said::Clicked(id.to_owned()),
                    map::Gesture::Dragged { id, by } => Said::Dragged(id.to_owned(), by),
                    map::Gesture::Dropped { id } => Said::Dropped(id.to_owned()),
                });
            }
        })
}

/// Where the right screen is drawn, so the pointer can be put on it.
fn middle_of(harness: &Harness, label: &str) -> egui::Pos2 {
    harness.get_by_label(label).rect().center()
}

/// A press, a move, a release: a drag that reports the whole gesture and then a drop.
///
/// The distance matters. egui holds the press/drag decision open until the pointer has
/// moved further than `max_click_dist`, so a "drag" of two points is a click — which is
/// the behaviour the C# was missing, and worth knowing is here.
#[test]
fn a_press_a_move_and_a_release_is_a_drag_and_then_a_drop() {
    let log = std::cell::RefCell::new(Vec::new());
    let mut harness = watching(&log);
    harness.run();

    let from = middle_of(&harness, "Right screen");
    let by = egui::vec2(-60.0, 25.0);

    harness.drag_at(from);
    harness.step();
    // In two moves, because a drag reports the whole gesture and not the last step: the
    // second has to say 60 and not 30.
    harness.hover_at(from + by / 2.0);
    harness.step();
    harness.hover_at(from + by);
    harness.step();
    harness.drop_at(from + by);
    harness.step();

    let said = log.borrow();
    let dragged: Vec<&Said> = said
        .iter()
        .filter(|s| matches!(s, Said::Dragged(..)))
        .collect();
    assert!(
        !dragged.is_empty(),
        "the press and the move were not a drag: {said:?}"
    );
    match dragged.last().expect("a move") {
        Said::Dragged(id, total) => {
            assert_eq!(id, "right");
            assert!(
                (*total - by).length() < 1.0,
                "the last move reported {total:?} for a gesture of {by:?}"
            );
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(
        said.last(),
        Some(&Said::Dropped("right".to_owned())),
        "the release was not a drop: {said:?}"
    );
    assert!(
        !said.iter().any(|s| matches!(s, Said::Clicked(_))),
        "a drag also counted as a click, so it would move the screen and select it: {said:?}"
    );
}

/// The other half of the same decision: a press and a release in the same place is a
/// click and **not** a zero-length drag, so it selects without touching the layout.
#[test]
fn a_press_and_a_release_in_place_is_a_click_and_nothing_else() {
    let log = std::cell::RefCell::new(Vec::new());
    let mut harness = watching(&log);
    harness.run();

    let at = middle_of(&harness, "Left screen");
    harness.drag_at(at);
    harness.step();
    harness.drop_at(at);
    harness.step();

    let said = log.borrow();
    assert_eq!(
        said.iter()
            .filter(|s| matches!(s, Said::Clicked(_)))
            .count(),
        1,
        "{said:?}"
    );
    assert!(
        !said
            .iter()
            .any(|s| matches!(s, Said::Dragged(..) | Said::Dropped(_))),
        "a still pointer moved a screen: {said:?}"
    );
}

//==========================================================================//
// The drop, on real desktops                                               //
//==========================================================================//

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

/// The reader `real_layouts.rs` uses, kept to the members `AddMonitor` reads.
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

/// One of the oracle's desktops, built by the real pipeline with a loader that does
/// nothing — the recipe `real_layouts.rs` documents, which needs no store.
fn desktop(scenario: &str) -> Layout {
    let dir = repo_root().join("domain-oracle/scenarios").join(scenario);
    let monitors: Vec<LinuxMonitor> = json(&dir.join("input.json"))["displays"]
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
    layout
}

/// Every monitor's outside rectangle, by id.
fn placed(layout: &Layout) -> Vec<(String, Rect)> {
    layout
        .monitors()
        .iter()
        .filter_map(|m| Some((m.id.clone(), layout.depth_projection(m)?.outside_bounds())))
        .collect()
}

fn primary_of(layout: &Layout) -> String {
    layout.primary_monitor().expect("a primary").id.clone()
}

/// A screen that is not the primary moves, and only it — until compaction has its say.
///
/// `three-screens-mixed-scale` has its screens in a row, so nudging one *towards* its
/// neighbour cannot leave a gap for the compaction to close, and the move is the move.
#[test]
fn dragging_a_screen_that_is_not_the_primary_moves_that_screen() {
    let mut layout = desktop("three-screens-mixed-scale");
    let primary = primary_of(&layout);
    let before = placed(&layout);

    let (moved, was) = before
        .iter()
        .find(|(id, _)| *id != primary)
        .cloned()
        .expect("a screen that is not the primary");

    // Downwards: the row's screens are side by side, so nothing is in the way and no gap
    // opens along the axis the compaction cares about.
    drag::drop_screen(&mut layout, &moved, (0.0, 40.0));

    let after = placed(&layout);
    let now = after
        .iter()
        .find(|(id, _)| *id == moved)
        .map(|(_, r)| *r)
        .expect("still there");
    assert!(
        (now.top() - (was.top() + 40.0)).abs() < 1e-6,
        "{moved} went from {} to {} instead of {}",
        was.top(),
        now.top(),
        was.top() + 40.0
    );
    for (id, then) in &before {
        if id == &moved {
            continue;
        }
        let still = after
            .iter()
            .find(|(o, _)| o == id)
            .map(|(_, r)| *r)
            .unwrap();
        assert!(
            (still.left() - then.left()).abs() < 1e-6,
            "{id} moved sideways when {moved} was dragged"
        );
    }
}

/// **The primary does not move.** Dragging it moves everything else the other way, which
/// looks the same on screen and leaves the millimetre space anchored where it was.
#[test]
fn dragging_the_primary_moves_every_other_screen_the_other_way() {
    let mut layout = desktop("three-screens-mixed-scale");
    let primary = primary_of(&layout);
    let before = placed(&layout);

    // Upwards: this desktop is a row, so a vertical move leaves every screen still
    // touching its neighbours and the compaction has nothing to say. See below for what
    // happens along the row.
    let by = (0.0, -15.0);
    drag::drop_screen(&mut layout, &primary, by);
    let after = placed(&layout);

    let at = |list: &[(String, Rect)], id: &str| {
        list.iter().find(|(o, _)| o == id).map(|(_, r)| *r).unwrap()
    };
    assert_eq!(
        at(&after, &primary).location(),
        at(&before, &primary).location(),
        "the primary moved, so the origin of the millimetre space moved with it"
    );
    for (id, then) in &before {
        if id == &primary {
            continue;
        }
        let now = at(&after, id);
        assert!(
            (now.top() - (then.top() - by.1)).abs() < 1e-6,
            "{id} went from {} to {} instead of {}",
            then.top(),
            now.top(),
            then.top() - by.1
        );
    }
}

/// And the other half of that rule, which is only visible on a real desktop: **dragging
/// the primary along a flush row does nothing at all.**
///
/// Moving every other screen towards the primary makes them overlap it, and the
/// compaction that follows pushes them straight back out — they were already touching, so
/// there was nowhere to go. Found by writing the test above with a diagonal drag and
/// watching the horizontal half of it vanish.
///
/// It is not a defect of this port: `EndMove` shifts the others and calls `Compact()` in
/// the same two steps (`FrameMover.cs:131-144`), so the Avalonia app does this too.
#[test]
fn dragging_the_primary_along_a_flush_row_is_undone_by_the_compaction() {
    let mut layout = desktop("three-screens-mixed-scale");
    let primary = primary_of(&layout);
    let before = placed(&layout);

    drag::drop_screen(&mut layout, &primary, (25.0, 0.0));

    for (id, then) in &before {
        let now = placed(&layout)
            .into_iter()
            .find(|(o, _)| o == id)
            .map(|(_, r)| r)
            .unwrap();
        assert!(
            (now.left() - then.left()).abs() < 1e-6,
            "{id} kept some of the drag: {} to {}",
            then.left(),
            now.left()
        );
    }
}

/// A drop that leaves a gap does not leave a gap: `EndMove` compacts, and compaction is
/// allowed to move screens the user never touched.
///
/// This is the reason the window re-reads the whole layout after a drop instead of
/// assuming it knows where the dragged screen ended up.
#[test]
fn a_screen_dropped_far_away_is_pulled_back_against_the_others() {
    let mut layout = desktop("three-screens-mixed-scale");
    assert!(
        !layout.options.allow_discontinuity,
        "this desktop allows gaps, so there would be nothing to compact"
    );
    let primary = primary_of(&layout);
    let before = layout.physical_bounds();

    // The leftmost screen, dragged half a metre further left: straight out into nothing,
    // leaving a 500 mm hole between it and the desktop.
    let (far, was) = placed(&layout)
        .into_iter()
        .filter(|(id, _)| *id != primary)
        .min_by(|a, b| a.1.left().total_cmp(&b.1.left()))
        .expect("a screen that is not the primary");
    drag::drop_screen(&mut layout, &far, (-500.0, 0.0));

    let now = placed(&layout)
        .into_iter()
        .find(|(id, _)| *id == far)
        .map(|(_, r)| r)
        .expect("still there");
    assert!(
        (now.left() - was.left()).abs() < 1e-6,
        "{far} was left where it was dropped, at {}, instead of being pulled back to {}",
        now.left(),
        was.left()
    );
    assert!(
        (layout.physical_bounds().width() - before.width()).abs() < 1e-6,
        "the desktop grew a hole: {} wide, was {}",
        layout.physical_bounds().width(),
        before.width()
    );
}

/// Dropping a screen where it already is changes nothing — no drift from the arithmetic,
/// and, on a desktop the compaction is happy with, no shuffle either.
#[test]
fn a_screen_dropped_where_it_was_is_a_screen_that_did_not_move() {
    for scenario in ["three-screens-mixed-scale", "grid-2x2", "six-monitors"] {
        let mut layout = desktop(scenario);
        let before = placed(&layout);
        let (first, _) = before.first().cloned().expect("a screen");

        drag::drop_screen(&mut layout, &first, (0.0, 0.0));

        for (id, then) in &before {
            let now = placed(&layout)
                .into_iter()
                .find(|(o, _)| o == id)
                .map(|(_, r)| r)
                .unwrap();
            assert!(
                (now.left() - then.left()).abs() < 1e-9 && (now.top() - then.top()).abs() < 1e-9,
                "{scenario}: {id} moved on a drop of nothing, {then:?} to {now:?}"
            );
        }
    }
}
