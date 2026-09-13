//! The list mode, through the accessibility tree.
//!
//! The arithmetic is tested beside the code. What is asked here is what it cannot
//! answer: that the rows are really there to be clicked, that clicking one says which,
//! and that the pane beside them draws the chosen screen and no other.

use egui_kittest::kittest::Queryable;
use egui_kittest::Harness;
use lbm_layout::geo::Rect;
use lbm_ui::list;
use lbm_ui::map::MapMonitor;

fn screens() -> Vec<MapMonitor<'static>> {
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

fn harness<'a>(app: impl FnMut(&mut egui::Ui) + 'a) -> Harness<'a> {
    Harness::builder()
        .with_size(egui::vec2(1200.0, 900.0))
        .build_ui(app)
}

#[test]
fn every_screen_has_a_row_to_click() {
    let all = screens();
    let harness = harness(|ui| {
        list::draw(ui, ui.max_rect(), &all, None);
    });

    for name in ["Left screen", "Right screen"] {
        assert!(harness.query_by_label(name).is_some(), "{name} has no row");
    }
}

#[test]
fn clicking_a_row_picks_that_screen() {
    let all = screens();
    let picked = std::cell::Cell::new(None);
    let mut harness = harness(|ui| {
        if let Some(id) = list::draw(ui, ui.max_rect(), &all, None) {
            picked.set(Some(id));
        }
    });

    harness.get_by_label("Right screen").click();
    harness.run();

    assert_eq!(picked.get(), Some("right"));
}

/// The pane draws the chosen screen and only it — the whole difference from the map.
/// With one chosen, its name appears twice (its row, and the frame beside it); the other
/// appears once.
#[test]
fn only_the_chosen_screen_is_drawn_beside_the_list() {
    let all = screens();
    let harness = harness(|ui| {
        list::draw(ui, ui.max_rect(), &all, Some("right"));
    });

    assert_eq!(
        harness.get_all_by_label("Right screen").count(),
        2,
        "the chosen screen should be in the list and drawn beside it"
    );
    assert_eq!(
        harness.get_all_by_label("Left screen").count(),
        1,
        "the screen that was not chosen should only be a row"
    );
}

/// Nothing chosen draws no screen at all, rather than picking one for the user.
#[test]
fn nothing_chosen_draws_no_screen() {
    let all = screens();
    let harness = harness(|ui| {
        list::draw(ui, ui.max_rect(), &all, None);
    });

    for name in ["Left screen", "Right screen"] {
        assert_eq!(harness.get_all_by_label(name).count(), 1, "{name}");
    }
}
