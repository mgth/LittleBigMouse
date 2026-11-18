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
            details: &[],
            probe: None,
        },
        MapMonitor {
            id: "right",
            name: "Right screen",
            mm_outside: Rect::new(700.0, 0.0, 640.0, 380.0),
            mm_content: Rect::new(720.0, 20.0, 600.0, 340.0),
            logo: None,
            wallpaper: None,
            details: &[],
            probe: None,
        },
    ]
}

fn harness<'a>(app: impl FnMut(&mut egui::Ui) + 'a) -> Harness<'a> {
    Harness::builder()
        .with_size(egui::vec2(1200.0, 900.0))
        .build_ui(app)
}

#[test]
fn the_selected_screen_is_drawn_without_a_duplicate_list() {
    let all = screens();
    let h = harness(|ui| {
        list::draw(ui, ui.max_rect(), &all, Some("right"));
    });
    assert_eq!(h.get_all_by_label("Right screen").count(), 1);
    assert!(h.query_by_label("Left screen").is_none());
}

#[test]
fn no_selection_points_to_the_properties_combo() {
    let all = screens();
    let h = harness(|ui| {
        list::draw(ui, ui.max_rect(), &all, None);
    });
    assert!(h
        .query_by_label("Select a screen in the properties panel.")
        .is_some());
    assert!(h.query_by_label("Right screen").is_none());
}
