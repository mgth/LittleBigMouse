use egui_kittest::{
    kittest::{NodeT, Queryable},
    Harness,
};
use lbm_layout::{geo::Rect, model::DisplaySize};
use lbm_ui::properties::{self, Edit, Size};
use std::cell::RefCell;

fn size() -> Size {
    Size {
        physical: DisplaySize::from_rect(Rect::new(0.0, 0.0, 600.0, 340.0)),
        locked: false,
        per_monitor_borders: true,
        model: "Panel model".into(),
    }
}

#[test]
fn merely_showing_the_measurements_never_edits_them() {
    for missing in [false, true] {
        let mut value = size();
        if missing {
            value.physical.width = 0.0;
            value.physical.height = 0.0;
        }
        let mut harness = Harness::new_ui(|ui| {
            assert!(properties::size(ui, "a", &value).is_empty());
        });
        harness.run();
        harness.run();
    }
}

#[test]
fn the_aspect_lock_is_an_explicit_edit() {
    let value = size();
    let edits = RefCell::new(Vec::new());
    let mut harness = Harness::new_ui(|ui| {
        edits.borrow_mut().extend(properties::size(ui, "a", &value));
    });
    harness.get_by_label("Keep aspect ratio").click();
    harness.run();
    assert_eq!(*edits.borrow(), [Edit::Lock(true)]);
}

#[test]
fn a_measurement_can_be_typed_with_its_accessible_label() {
    let value = size();
    let edits = RefCell::new(Vec::new());
    let mut harness = Harness::new_ui(|ui| {
        edits.borrow_mut().extend(properties::size(ui, "a", &value));
    });
    // The label is also painted as text; select the interactive labelled node.
    let width = harness
        .get_all_by_label("Panel width")
        .find(|n| n.accesskit_node().role() != egui::accesskit::Role::Label)
        .unwrap();
    width.focus();
    harness.run();
    let width = harness
        .get_all_by_label("Panel width")
        .find(|n| n.accesskit_node().role() == egui::accesskit::Role::SpinButton)
        .unwrap();
    width.type_text("650");
    harness.key_press(egui::Key::Enter);
    harness.run();
    assert!(
        edits.borrow().contains(&Edit::Width(650.0)),
        "{:?}",
        edits.borrow()
    );
}

#[test]
fn the_properties_area_never_covers_the_map_at_wide_or_narrow_sizes() {
    for width in [480.0, 1100.0] {
        let value = size();
        let properties_rect = std::cell::Cell::new(egui::Rect::NOTHING);
        let map_rect = std::cell::Cell::new(egui::Rect::NOTHING);
        let mut harness = Harness::builder()
            .with_size(egui::vec2(width, 600.0))
            .build_ui(|ui| {
                properties::panel(ui, |ui| {
                    properties_rect.set(ui.max_rect());
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        properties::size(ui, "a", &value);
                    });
                });
                egui::CentralPanel::default().show(ui, |ui| {
                    map_rect.set(ui.max_rect());
                });
            });
        harness.run();
        assert!(!properties_rect.get().intersects(map_rect.get()));
        assert!(map_rect.get().width() > 100.0 && map_rect.get().height() > 100.0);
        assert!(harness.query_by_label("Keep aspect ratio").is_some());
    }
}

#[test]
fn view_switch_sits_beside_the_combo_and_preserves_selection() {
    let mut h = Harness::builder()
        .with_size(egui::vec2(300., 120.))
        .build_ui_state(
            |ui, state: &mut (Option<String>, bool)| {
                lbm_ui::theme::install(ui.ctx());
                properties::monitor_selector(
                    ui,
                    &mut state.0,
                    &[("a", "First"), ("b", "Second")],
                    &mut state.1,
                );
            },
            (Some("a".into()), false),
        );
    let combo = h.get_by_role_and_label(egui::accesskit::Role::ComboBox, "Screen");
    let combo_bounds = combo.accesskit_node().bounding_box().unwrap();
    let toggle_bounds = h
        .get_by_label("Single screen view")
        .accesskit_node()
        .bounding_box()
        .unwrap();
    assert!(toggle_bounds.x0 >= combo_bounds.x1);
    assert!(toggle_bounds.y0 <= combo_bounds.y1 && toggle_bounds.y1 >= combo_bounds.y0);
    combo.click();
    h.run();
    h.get_by_label("Second · b").click();
    h.run();
    assert_eq!(h.state().0.as_deref(), Some("b"));
    h.get_by_label("Single screen view").click();
    h.run();
    assert!(h.state().1);
    assert_eq!(h.state().0.as_deref(), Some("b"));
    h.get_by_label("Single screen view").click();
    h.run();
    assert!(!h.state().1);
    assert_eq!(h.state().0.as_deref(), Some("b"));
}
