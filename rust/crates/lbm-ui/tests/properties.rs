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

/// What `DisplayConfigGetDeviceInfo` gives as a target's monitor device path, which the
/// Windows agent reports as the output's connector (`windows_topology.rs`).
const WINDOWS_CONNECTOR: &str =
    r"\\?\DISPLAY#SAM7184#5&2a1f3c4b&0&UID4352#{e6f07b5f-ee97-4a90-b076-33f57bf4eaa7}";

/// Lays out the properties panel beside a map at `width` and returns the rect the
/// properties content was given and the one the map was given.
fn beside_map(width: f32, content: impl Fn(&mut egui::Ui)) -> (egui::Rect, egui::Rect) {
    let properties_rect = std::cell::Cell::new(egui::Rect::NOTHING);
    let map_rect = std::cell::Cell::new(egui::Rect::NOTHING);
    let mut harness = Harness::builder()
        .with_size(egui::vec2(width, 600.0))
        .build_ui(|ui| {
            properties::panel(ui, |ui| {
                properties_rect.set(ui.max_rect());
                content(ui);
            });
            egui::CentralPanel::default().show(ui, |ui| {
                map_rect.set(ui.max_rect());
            });
        });
    // The panel keeps its size from one frame to the next, as it does in the app.
    for _ in 0..4 {
        harness.run();
    }
    (properties_rect.get(), map_rect.get())
}

#[test]
fn a_windows_interface_path_never_lets_the_map_cover_the_properties() {
    let outputs = lbm_ipc::displays::Outputs {
        signature: "s".into(),
        outputs: vec![lbm_ipc::displays::Output {
            connector: WINDOWS_CONNECTOR.into(),
            name: "Odyssey G80SD".into(),
            enabled: true,
            primary: true,
        }],
    };
    let panel = RefCell::new(lbm_ui::displays::Panel {
        selected: Some(WINDOWS_CONNECTOR.into()),
        confirm: None,
    });
    let used = std::cell::Cell::new(egui::Rect::NOTHING);
    let (properties, map) = beside_map(1100.0, |ui| {
        panel
            .borrow_mut()
            .draw(ui, Some(&outputs), None, false, true, true);
        used.set(ui.min_rect());
    });
    assert!(
        !properties.intersects(map),
        "properties {properties:?} under map {map:?}"
    );
    // The path wraps instead of running past the panel, where it would be cut off.
    assert!(
        used.get().right() <= properties.right() + 0.5,
        "content {:?} wider than properties {properties:?}",
        used.get()
    );
}

#[test]
fn content_wider_than_the_panel_is_clipped_not_slid_under_the_map() {
    let (properties, map) = beside_map(1100.0, |ui| {
        ui.label("Visible at the start of the panel");
        ui.allocate_exact_size(egui::vec2(900.0, 20.0), egui::Sense::hover());
    });
    assert!(
        !properties.intersects(map),
        "properties {properties:?} under map {map:?}"
    );
    assert!(properties.width() >= 280.0 - 2.0 * 14.0);
}
