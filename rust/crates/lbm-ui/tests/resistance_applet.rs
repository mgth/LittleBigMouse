use egui::{pos2, vec2};
use egui_kittest::{
    kittest::{NodeT, Queryable},
    Harness,
};
use lbm_layout::{
    geo::Rect,
    model::{BorderResistance, BorderSection},
};
use lbm_ui::resistance::{Edit, Editor, Monitor, Selection, Side};

struct State {
    editor: Editor,
    monitors: Vec<Monitor>,
    behind_clicked: bool,
}
fn harness() -> Harness<'static, State> {
    let mut editor = Editor::default();
    editor.selected = Some(Selection {
        monitor: "a".into(),
        side: Side::Right,
        index: 0,
    });
    let mut resistance = BorderResistance::default();
    resistance
        .right
        .sections
        .push(BorderSection::new(30., 230., 4., false, 12., false));
    Harness::builder()
        .with_size(vec2(700., 420.))
        .build_ui_state(
            |ui, state| {
                lbm_ui::theme::install(ui.ctx());
                let content = egui::Rect::from_min_size(pos2(40., 40.), vec2(600., 300.));
                state.behind_clicked |= ui
                    .interact(
                        content,
                        egui::Id::new("monitor"),
                        egui::Sense::click_and_drag(),
                    )
                    .clicked();
                let action = state
                    .editor
                    .bands(ui, &state.monitors, &[("a".into(), content)])
                    .action;
                if let Some(action) = action {
                    let sections = action.side.sections_mut(&mut state.monitors[0].resistance);
                    assert_eq!(*sections, action.expected);
                    match action.edit {
                        Edit::Replace { index, section } => sections[index] = section,
                        other => panic!("applet must not move or create sections: {other:?}"),
                    }
                }
            },
            State {
                editor,
                monitors: vec![Monitor {
                    id: "a".into(),
                    bounds: Rect::new(0., 0., 600., 300.),
                    excluded: false,
                    resistance,
                }],
                behind_clicked: false,
            },
        )
}
fn rect(h: &Harness<State>, label: &str) -> egui::Rect {
    let r = h
        .get_by_label(label)
        .accesskit_node()
        .bounding_box()
        .unwrap();
    egui::Rect::from_min_max(
        pos2(r.x0 as f32, r.y0 as f32),
        pos2(r.x1 as f32, r.y1 as f32),
    )
}
#[test]
fn continuous_slider_drag_keeps_capture_and_geometry_and_only_edits_its_channel() {
    let mut h = harness();
    h.run();
    let r = rect(&h, "Move resistance");
    h.drag_at(pos2(r.left() + 10., r.center().y));
    h.step();
    let mut previous = 0.;
    for fraction in [0.2, 0.4, 0.6, 0.8] {
        h.hover_at(pos2(r.left() + r.width() * fraction, r.center().y));
        h.step();
        let s = h.state().monitors[0].resistance.right.sections[0];
        assert!(s.move_resistance() > previous, "{s:?}");
        previous = s.move_resistance();
        assert_eq!(s.drag(), 12.);
        assert_eq!((s.from(), s.to()), (30., 230.));
        assert_eq!(
            rect(&h, "Move resistance"),
            r,
            "slider must not move under capture"
        );
    }
    h.drop_at(pos2(r.right() - 2., r.center().y));
    h.run();
    assert_eq!(
        h.state().monitors[0].resistance.right.sections[0].move_resistance(),
        50.
    );
    assert!(
        !h.state().behind_clicked,
        "applet must not click through to the monitor"
    );
}
#[test]
fn independent_modes_preserve_values_and_follow_external_property_edits() {
    let mut h = harness();
    h.run();
    h.get_by_label("Move: Block").click();
    h.run();
    let s = h.state().monitors[0].resistance.right.sections[0];
    assert!(s.move_block());
    assert!(!s.drag_block());
    assert_eq!((s.move_resistance(), s.drag()), (4., 12.));
    assert!(h.query_by_label("Move resistance").is_none());
    h.get_by_label("Drag: Block").click();
    h.run();
    assert!(h.state().monitors[0].resistance.right.sections[0].drag_block());
    h.get_by_label("Move: Custom").click();
    h.run();
    assert_eq!(
        h.state().monitors[0].resistance.right.sections[0].move_resistance(),
        4.
    );
    // A value from the property panel must not be clamped just by drawing the applet.
    h.state_mut().monitors[0].resistance.right.sections[0].set_move_resistance(125.);
    h.run();
    assert_eq!(
        h.state().monitors[0].resistance.right.sections[0].move_resistance(),
        125.
    );
    assert_eq!(
        h.get_by_label("Move resistance")
            .accesskit_node()
            .numeric_value(),
        Some(125.)
    );
}
#[test]
fn escape_collapses_and_selection_removal_retires_the_applet() {
    let mut h = harness();
    h.run();
    h.key_press(egui::Key::Escape);
    h.run();
    assert!(h.query_by_label("Move resistance").is_none());
    h.get_by_label("Move 4.0 · Drag 12.0").click();
    h.run();
    assert!(h.query_by_label("Move resistance").is_some());
    h.state_mut().monitors[0].resistance.right.sections.clear();
    h.run();
    assert!(h.query_by_label("Move resistance").is_none());
}

#[test]
fn resist_follows_defaults_and_free_does_not_erase_custom_values() {
    use lbm_layout::model::ResistanceMode;
    let mut h = harness();
    h.run();
    h.get_by_label("Move: Resist").click();
    h.run();
    h.state_mut().editor.defaults.movement = 8.;
    h.run();
    assert!(h.query_by_label("Default · 8.0 mm").is_some());
    let s = h.state().monitors[0].resistance.right.sections[0];
    assert_eq!(s.move_mode(), ResistanceMode::Resist);
    assert_eq!(s.drag_mode(), ResistanceMode::Custom);
    h.get_by_label("Move: Free").click();
    h.run();
    assert!(h.query_by_label("No resistance").is_some());
    h.get_by_label("Move: Custom").click();
    h.run();
    assert_eq!(
        h.state().monitors[0].resistance.right.sections[0].move_resistance(),
        4.
    );
    assert!(!h.state().behind_clicked);
}

#[test]
#[cfg(not(windows))]
fn resistance_modes_applet_snapshot() {
    let mut h = harness();
    h.run();
    h.get_by_label("Move: Resist").click();
    h.run();
    h.snapshot("resistance-modes-applet");
}

#[test]
fn default_switches_edit_each_channel_independently_without_changing_the_values() {
    let mut h = Harness::builder()
        .with_size(vec2(300., 200.))
        .build_ui_state(
            |ui, defaults| {
                lbm_ui::resistance::defaults(ui, defaults);
            },
            lbm_layout::model::ResistanceDefaults::default(),
        );
    h.run();
    h.get_by_label("Move: apply to crossings without a section")
        .click();
    h.run();
    assert!(h.state().move_without_section);
    assert!(!h.state().drag_without_section);
    h.get_by_label("Drag: apply to crossings without a section")
        .click();
    h.run();
    h.get_by_label("Move: apply to crossings without a section")
        .click();
    h.run();
    assert!(!h.state().move_without_section);
    assert!(h.state().drag_without_section);
    assert_eq!((h.state().movement, h.state().drag), (5., 15.));
}
