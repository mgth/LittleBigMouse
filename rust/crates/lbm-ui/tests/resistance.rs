use egui_kittest::{
    kittest::{NodeT, Queryable},
    Harness,
};
use lbm_layout::{
    geo::Rect,
    model::{BorderResistance, BorderSection},
};
use lbm_ui::resistance::{
    self as r, geometry as g, Action, Edit, Editor, Monitor, Selection, Side,
};
use std::cell::RefCell;
fn section(a: f64, b: f64) -> BorderSection {
    BorderSection::new(a, b, 0.0, true, 0.0, false)
}
fn monitor(id: &str, x: f64, y: f64) -> Monitor {
    Monitor {
        id: id.into(),
        bounds: Rect::new(x, y, 600.0, 300.0),
        excluded: false,
        resistance: BorderResistance::default(),
    }
}
#[test]
fn creation_resize_and_move_stop_at_neighbours_and_reject_slivers() {
    let sections = [section(20.0, 60.0), section(150.0, 180.0)];
    let gap = g::gap(&sections, None, 100.0, 300.0);
    assert_eq!(
        g::clamp(g::Span::new(100.0, 250.0), gap),
        Some(g::Span::new(100.0, 150.0))
    );
    assert_eq!(g::clamp(g::Span::new(100.0, 101.0), gap), None);
    assert_eq!(
        g::shifted(g::Span::new(80.0, 110.0), 145.0, gap),
        Some(g::Span::new(120.0, 150.0))
    );
    assert_eq!(
        g::largest(&sections, g::Span::new(0.0, 300.0)),
        Some(g::Span::new(180.0, 300.0))
    );
}
#[test]
fn shared_boundaries_belong_to_the_section_starting_there() {
    let sections = [section(0.0, 80.0), section(80.0, 160.0)];
    assert_eq!(g::grab(&sections, 79.0, 2.0), g::Grab::To(0));
    assert_eq!(g::grab(&sections, 80.0, 2.0), g::Grab::From(1));
    assert_eq!(g::grab(&sections, 81.0, 2.0), g::Grab::From(1));
    assert_eq!(g::grab(&sections, 120.0, 2.0), g::Grab::Move(1));
}
#[test]
fn snapping_uses_visible_edges_and_parallel_sections_even_on_distant_screens() {
    let mut monitors = vec![monitor("a", 0.0, 0.0), monitor("b", 2000.0, 60.0)];
    monitors[0]
        .resistance
        .right
        .sections
        .push(section(30.0, 70.0));
    monitors[0]
        .resistance
        .left
        .sections
        .push(section(40.0, 80.0));
    monitors[1]
        .resistance
        .left
        .sections
        .push(section(0.0, 50.0));
    let targets = g::targets(&monitors, "a", Side::Right, Some(0));
    for v in [0.0, 300.0, 150.0, 40.0, 80.0, 60.0, 110.0] {
        assert!(targets.contains(&v));
    }
    assert!(!targets.contains(&30.0));
    assert!(!targets.contains(&70.0));
    assert_eq!(g::snap(108.0, 300.0, 4.0, &targets, true), 110.0);
    assert_eq!(g::snap(108.0, 300.0, 4.0, &targets, false), 108.0);
    assert_eq!(g::snap_move(42.0, 30.0, 4.0, &[40.0]), 40.0);
    assert_eq!(g::snap_move(38.0, 40.0, 4.0, &[80.0]), 40.0);
}
#[test]
fn mirroring_keeps_the_largest_free_piece_and_respects_travel_and_exclusion() {
    let mut monitors = vec![monitor("a", 0.0, 0.0), monitor("b", 620.0, 60.0)];
    monitors[0]
        .resistance
        .right
        .sections
        .push(section(80.0, 180.0));
    monitors[1]
        .resistance
        .left
        .sections
        .push(section(40.0, 60.0));
    let key = Selection {
        monitor: "a".into(),
        side: Side::Right,
        index: 0,
    };
    let (target, s) = g::mirror(&monitors, &key, 20.0).unwrap();
    assert_eq!(target.side, Side::Left);
    assert_eq!(target.monitor, "b");
    assert_eq!(s, section(60.0, 120.0));
    assert!(g::mirror(&monitors, &key, 19.0).is_none());
    monitors[1].excluded = true;
    assert!(g::mirror(&monitors, &key, 20.0).is_none());
}
#[test]
fn all_bands_project_millimetres_in_the_same_direction_as_the_engine() {
    let content = egui::Rect::from_min_size(egui::pos2(10.0, 20.0), egui::vec2(600.0, 300.0));
    for side in Side::ALL {
        let band = r::band(content, side);
        let s = r::segment(
            band,
            side,
            g::Span::new(30.0, 80.0),
            if side.vertical() { 300.0 } else { 600.0 },
        );
        assert!(content.contains_rect(s));
        if side.vertical() {
            assert!((s.top() - 50.0).abs() < 0.001);
            assert!((s.bottom() - 100.0).abs() < 0.001);
        } else {
            assert!((s.left() - 40.0).abs() < 0.001);
            assert!((s.right() - 90.0).abs() < 0.001);
        }
    }
    assert!(!g::fully_blocked(&[section(0.0, 300.0)], 300.0));
    assert!(g::fully_blocked(
        &[BorderSection::new(0.0, 300.0, 0.0, true, 0.0, true)],
        300.0
    ));
}

// Map frames are sensed first, exactly as in the application. Bands must win the gesture.
fn bands<'a>(
    editor: &'a RefCell<Editor>,
    monitors: &'a RefCell<Vec<Monitor>>,
    out: &'a RefCell<Vec<Action>>,
) -> Harness<'a> {
    Harness::builder()
        .with_size(egui::vec2(700.0, 420.0))
        .with_step_dt(1.0 / 60.0)
        .build_ui(move |ui| {
            let content =
                egui::Rect::from_min_size(egui::pos2(40.0, 40.0), egui::vec2(600.0, 300.0));
            ui.interact(
                content,
                egui::Id::new("monitor"),
                egui::Sense::click_and_drag(),
            );
            let interaction =
                editor
                    .borrow_mut()
                    .bands(ui, &monitors.borrow(), &[("a".into(), content)]);
            if let Some(action) = interaction.action {
                out.borrow_mut().push(action);
            }
        })
}
fn drag(h: &mut Harness, from: egui::Pos2, to: egui::Pos2) {
    h.drag_at(from);
    h.step();
    h.hover_at(to);
    h.step();
}
#[test]
fn a_draw_gesture_commits_only_on_release() {
    let editor = RefCell::new(Editor::default());
    let monitors = RefCell::new(vec![monitor("a", 0.0, 0.0)]);
    let actions = RefCell::new(vec![]);
    let mut h = bands(&editor, &monitors, &actions);
    h.run();
    drag(&mut h, egui::pos2(634.0, 90.0), egui::pos2(634.0, 150.0));
    assert!(editor.borrow().active());
    assert!(actions.borrow().is_empty());
    h.drop_at(egui::pos2(634.0, 150.0));
    h.run();
    assert!(!editor.borrow().active());
    let log = actions.borrow();
    assert_eq!(log.len(), 1);
    let Edit::Insert(s) = log[0].edit else {
        panic!("expected a new section");
    };
    assert!((s.from() - 50.0).abs() < 0.001 && (s.to() - 110.0).abs() < 0.001);
    assert!(s.move_block() && !s.drag_block());
}
#[test]
fn escape_focus_loss_and_layout_change_discard_a_gesture() {
    for reason in 0..3 {
        let editor = RefCell::new(Editor::default());
        let monitors = RefCell::new(vec![monitor("a", 0.0, 0.0)]);
        let actions = RefCell::new(vec![]);
        monitors.borrow_mut()[0]
            .resistance
            .right
            .sections
            .push(section(50.0, 130.0));
        let mut h = bands(&editor, &monitors, &actions);
        h.run();
        drag(&mut h, egui::pos2(634.0, 130.0), egui::pos2(634.0, 160.0));
        assert!(editor.borrow().active());
        match reason {
            0 => h.key_press(egui::Key::Escape),
            1 => h.input_mut().focused = false,
            _ => monitors.borrow_mut()[0].resistance.right.sections.clear(),
        };
        h.step();
        h.drop_at(egui::pos2(634.0, 160.0));
        h.run();
        assert!(actions.borrow().is_empty());
        assert!(!editor.borrow().active());
    }
}
#[test]
fn a_resize_keeps_the_grab_offset_and_a_move_keeps_its_length() {
    for (start, end, wanted) in [
        (91.0, 101.0, section(60.0, 130.0)),
        (130.0, 160.0, section(80.0, 160.0)),
    ] {
        let editor = RefCell::new(Editor::default());
        let monitors = RefCell::new(vec![monitor("a", 0.0, 0.0)]);
        let actions = RefCell::new(vec![]);
        monitors.borrow_mut()[0]
            .resistance
            .right
            .sections
            .push(section(50.0, 130.0));
        let mut h = bands(&editor, &monitors, &actions);
        h.run();
        h.event(egui::Event::ModifiersChanged(egui::Modifiers::CTRL));
        drag(&mut h, egui::pos2(634.0, start), egui::pos2(634.0, end));
        h.drop_at(egui::pos2(634.0, end));
        h.run();
        let log = actions.borrow();
        assert_eq!(log.len(), 1);
        let Edit::Replace { section: s, .. } = log[0].edit else {
            panic!("expected resize/move");
        };
        assert!(
            (s.from() - wanted.from()).abs() < 0.001 && (s.to() - wanted.to()).abs() < 0.001,
            "{s:?}"
        );
    }
}
#[test]
fn the_panel_exposes_creation_blocks_mirror_and_deletion() {
    let mut monitors = vec![monitor("a", 0.0, 0.0), monitor("b", 620.0, 0.0)];
    monitors[0]
        .resistance
        .right
        .sections
        .push(section(40.0, 90.0));
    let mut editor = Editor::default();
    editor.selected = Some(Selection {
        monitor: "a".into(),
        side: Side::Right,
        index: 0,
    });
    let actions = RefCell::new(vec![]);
    let mut h = Harness::builder()
        .with_size(egui::vec2(420.0, 700.0))
        .build_ui(|ui| {
            if let Some(a) = editor.panel(ui, &monitors, "a", 200.0) {
                actions.borrow_mut().push(a);
            }
        });
    h.run();
    assert!(!h.get_by_label("Mirror").accesskit_node().is_disabled());
    h.get_by_label("Block drags").click();
    h.run();
    assert!(actions
        .borrow()
        .iter()
        .any(|a| matches!(a.edit,Edit::Replace{section:s,..} if s.drag_block())));
    h.get_by_label("Delete section").click();
    h.run();
    assert!(actions
        .borrow()
        .iter()
        .any(|a| matches!(a.edit, Edit::Delete(0))));
    h.get_by_label("Add section").click();
    h.run();
    assert!(actions
        .borrow()
        .iter()
        .any(|a| matches!(a.edit, Edit::Insert(_))));
}

#[test]
fn double_click_fills_a_gap_and_delete_uses_the_selected_section() {
    let editor = RefCell::new(Editor::default());
    let monitors = RefCell::new(vec![monitor("a", 0.0, 0.0)]);
    let actions = RefCell::new(vec![]);
    let mut h = bands(&editor, &monitors, &actions);
    h.run();
    let at = egui::pos2(634.0, 130.0);
    for _ in 0..2 {
        h.drag_at(at);
        h.step();
        h.drop_at(at);
        h.step();
    }
    let action = actions
        .borrow_mut()
        .pop()
        .expect("double click should fill the edge");
    let Edit::Insert(s) = action.edit else {
        panic!("expected insertion");
    };
    assert_eq!(s, section(0.0, 300.0));
    monitors.borrow_mut()[0].resistance.right.sections.push(s);
    editor.borrow_mut().selected = Some(Selection {
        monitor: "a".into(),
        side: Side::Right,
        index: 0,
    });
    h.run();
    h.key_press(egui::Key::Delete);
    h.run();
    assert!(actions
        .borrow()
        .iter()
        .any(|a| matches!(a.edit, Edit::Delete(0))));
}
#[test]
fn numeric_bounds_are_accessible_and_limited_by_neighbouring_sections() {
    let mut monitors = vec![monitor("a", 0.0, 0.0)];
    monitors[0].resistance.right.sections = vec![section(0.0, 20.0), section(40.0, 90.0)];
    let mut editor = Editor::default();
    editor.selected = Some(Selection {
        monitor: "a".into(),
        side: Side::Right,
        index: 1,
    });
    let actions = RefCell::new(vec![]);
    let mut h = Harness::builder()
        .with_size(egui::vec2(420.0, 700.0))
        .build_ui(|ui| {
            if let Some(a) = editor.panel(ui, &monitors, "a", 200.0) {
                actions.borrow_mut().push(a);
            }
        });
    h.run();
    h.get_all_by_label("From")
        .find(|n| n.accesskit_node().role() != egui::accesskit::Role::Label)
        .unwrap()
        .focus();
    h.run();
    h.get_all_by_label("From")
        .find(|n| n.accesskit_node().role() == egui::accesskit::Role::SpinButton)
        .unwrap()
        .type_text("10");
    h.key_press(egui::Key::Enter);
    h.run();
    assert!(actions
        .borrow()
        .iter()
        .any(|a| matches!(a.edit,Edit::Replace{section:s,..} if s.from()==20.0 && s.to()==90.0)));
    assert!(h.get_by_label("Mirror").accesskit_node().is_disabled());
}
