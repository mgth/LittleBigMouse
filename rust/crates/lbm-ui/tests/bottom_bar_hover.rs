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

/// A button must not leave its style changes on the row: the next one, and whatever else
/// is drawn after the bar, keep the padding and the resting look they had.
#[test]
fn a_button_leaves_the_rows_style_alone() {
    use std::{cell::RefCell, rc::Rc};
    let seen = Rc::new(RefCell::new(Vec::new()));
    let out = seen.clone();
    let mut harness = Harness::builder()
        .with_size(egui::vec2(400.0, 60.0))
        .build_ui(move |ui| {
            lbm_ui::theme::install(ui.ctx());
            egui::CentralPanel::default().show(ui, |ui| {
                ui.horizontal(|ui| {
                    let snapshot = |ui: &egui::Ui| {
                        let v = &ui.visuals().widgets.inactive;
                        out.borrow_mut().push((
                            ui.spacing().button_padding,
                            v.weak_bg_fill,
                            v.bg_stroke.color,
                        ));
                    };
                    snapshot(ui);
                    lbm_ui::icons::button(ui, lbm_ui::icons::Icon::Undo, "Undo", None, true);
                    snapshot(ui);
                    lbm_ui::icons::button(ui, lbm_ui::icons::Icon::Play, "Start", None, true);
                    snapshot(ui);
                });
            });
        });
    harness.run();
    let seen = seen.borrow();
    // Within each frame the three readings agree (the first frame may still carry the
    // default style, before the theme is installed).
    assert!(!seen.is_empty());
    for frame in seen.chunks(3) {
        assert!(frame.iter().all(|s| *s == frame[0]), "{frame:?}");
    }
}
