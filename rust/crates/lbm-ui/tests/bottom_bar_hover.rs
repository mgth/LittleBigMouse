//! Hovering a bottom-bar button must not move or resize any button.
//!
//! egui's default hover `expansion` grew the hovered button's allocation, which pushed
//! its neighbours sideways as the pointer crossed the bar.

use egui_kittest::kittest::Queryable;
use egui_kittest::Harness;

const LABELS: [&str; 5] = ["Save", "Undo", "Start", "Stop", "Live"];

fn rects(harness: &Harness<'_>) -> Vec<egui::Rect> {
    LABELS
        .iter()
        .map(|label| {
            harness
                .query_by_label_contains(label)
                .unwrap_or_else(|| panic!("no button {label}"))
                .rect()
        })
        .collect()
}

#[test]
fn hovering_a_button_changes_no_rectangle() {
    let state = lbm_ui::State {
        engine: lbm_ui::Engine::Running,
        hook_connected: true,
        saved: false,
        ..Default::default()
    };
    let mut harness = Harness::builder()
        .with_size(egui::vec2(700.0, 60.0))
        .build_ui(move |ui| {
            lbm_ui::theme::install(ui.ctx());
            egui::CentralPanel::default().show(ui, |ui| {
                lbm_ui::bottom_bar(ui, &state);
            });
        });
    harness.run();
    let rest = rects(&harness);

    for label in LABELS {
        harness.get_by_label_contains(label).hover();
        harness.run();
        assert_eq!(rects(&harness), rest, "hovering {label} moved a button");
    }
}
