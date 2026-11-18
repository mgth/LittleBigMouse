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
fn coincident_references_keep_the_geometry_color_and_ignore_the_edited_section() {
    let mut monitors = vec![monitor("a", 0., 0.), monitor("b", 2000., 60.)];
    monitors[0]
        .resistance
        .right
        .sections
        .push(section(30., 70.));
    monitors[1]
        .resistance
        .left
        .sections
        .extend([section(0., 50.), section(90., 100.)]);
    let targets = g::references(&monitors, "a", Side::Right, Some(0));
    let kind = |mm| targets.iter().find(|t| t.mm == mm).unwrap().kind;
    assert_eq!(kind(0.), r::SnapKind::EdgeEnd);
    assert_eq!(kind(60.), r::SnapKind::ScreenEdge);
    assert_eq!(kind(110.), r::SnapKind::Section);
    assert_eq!(kind(150.), r::SnapKind::Middle);
    assert_eq!(targets.iter().filter(|t| t.mm == 150.).count(), 1);
    assert!(!targets.iter().any(|t| [30., 70.].contains(&t.mm)));
}

#[test]
fn the_active_boundary_publishes_a_physical_guide_and_ctrl_release_clear_it() {
    let editor = RefCell::new(Editor::default());
    let mut m = vec![monitor("a", 0., -75.), monitor("b", 2000., -15.)];
    m[1].resistance.left.sections.push(section(0., 50.));
    let monitors = RefCell::new(m);
    let actions = RefCell::new(vec![]);
    let mut h = bands(&editor, &monitors, &actions);
    h.run();
    drag(&mut h, egui::pos2(634., 90.), egui::pos2(634., 148.));
    let guide = editor.borrow().guide().unwrap();
    assert_eq!(guide.kind, r::SnapKind::Section);
    assert_eq!(guide.mm, 110.);
    assert_eq!(guide.layout_mm, 35.);
    assert!(actions.borrow().is_empty());
    h.event(egui::Event::ModifiersChanged(egui::Modifiers::CTRL));
    h.step();
    assert!(editor.borrow().guide().is_none());
    h.event(egui::Event::ModifiersChanged(egui::Modifiers::NONE));
    h.step();
    assert_eq!(editor.borrow().guide(), Some(guide));
    h.drop_at(egui::pos2(634., 148.));
    h.run();
    assert!(editor.borrow().guide().is_none());
    assert_eq!(actions.borrow().len(), 1);
}

#[test]
fn fixed_endpoints_do_not_claim_a_snap_and_moves_can_mark_the_far_end() {
    for (existing, from, to, expected) in [
        (None, 190., 240., None), // draw from the midpoint, moving end remains free
        (Some(section(50., 150.)), 91., 105., None), // resize start, fixed end is the midpoint
        (Some(section(50., 130.)), 130., 147., Some(150.)), // move's far end catches the midpoint
        (Some(section(50., 130.)), 91., 190., Some(150.)), // crossing a resize's fixed endpoint
    ] {
        let editor = RefCell::new(Editor::default());
        let mut m = monitor("a", 0., 0.);
        if let Some(s) = existing {
            m.resistance.right.sections.push(s);
        }
        let monitors = RefCell::new(vec![m]);
        let actions = RefCell::new(vec![]);
        let mut h = bands(&editor, &monitors, &actions);
        h.run();
        drag(&mut h, egui::pos2(634., from), egui::pos2(634., to));
        assert_eq!(
            editor.borrow().guide().map(|g| g.mm),
            expected,
            "from {from}, to {to}"
        );
    }
}

#[test]
fn reference_lifetime_follows_escape_focus_geometry_and_section_changes() {
    for reason in 0..4 {
        let editor = RefCell::new(Editor::default());
        let monitors = RefCell::new(vec![monitor("a", 0., 0.)]);
        let actions = RefCell::new(vec![]);
        let mut h = bands(&editor, &monitors, &actions);
        h.run();
        drag(&mut h, egui::pos2(634., 90.), egui::pos2(634., 188.));
        assert!(editor.borrow().guide().is_some());
        match reason {
            0 => h.key_press(egui::Key::Escape),
            1 => h.input_mut().focused = false,
            2 => monitors.borrow_mut()[0].bounds = Rect::new(0., 10., 600., 300.),
            _ => monitors.borrow_mut()[0]
                .resistance
                .right
                .sections
                .push(section(0., 20.)),
        }
        h.step();
        assert!(editor.borrow().guide().is_none());
        assert!(editor.borrow().preview_action().is_none());
        assert!(actions.borrow().is_empty());
    }
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
    bands_in(
        editor,
        monitors,
        out,
        egui::Rect::from_min_size(egui::pos2(40.0, 40.0), egui::vec2(600.0, 300.0)),
    )
}
fn bands_in<'a>(
    editor: &'a RefCell<Editor>,
    monitors: &'a RefCell<Vec<Monitor>>,
    out: &'a RefCell<Vec<Action>>,
    content: egui::Rect,
) -> Harness<'a> {
    Harness::builder()
        .with_size(egui::vec2(700.0, 420.0))
        .with_step_dt(1.0 / 60.0)
        .build_ui(move |ui| {
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
    let Edit::Insert(preview) = editor.borrow().preview_action().unwrap().edit else {
        panic!("expected overlay preview");
    };
    assert!((preview.from() - 50.).abs() < 0.001 && (preview.to() - 110.).abs() < 0.001);
    assert!(monitors.borrow()[0].resistance.right.sections.is_empty());
    h.drop_at(egui::pos2(634.0, 150.0));
    h.run();
    assert!(!editor.borrow().active());
    assert!(editor.borrow().preview_action().is_none());
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
        assert!(editor.borrow().preview_action().is_none());
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

#[test]
fn the_panel_keeps_stored_modes_unless_their_own_value_is_edited() {
    use lbm_layout::model::ResistanceMode::{Custom, Free, Resist};
    for (field, expected_span, expected_modes) in [
        ("From", (10.0, 90.0), (Resist, Free)),
        ("Move resistance", (40.0, 90.0), (Custom, Free)),
        ("Drag resistance", (40.0, 90.0), (Resist, Custom)),
    ] {
        let mut stored = BorderSection::new(40.0, 90.0, 4.0, false, 9.0, false);
        stored.set_move_mode(Resist);
        stored.set_drag_mode(Free);
        let mut monitors = vec![monitor("a", 0.0, 0.0)];
        monitors[0].resistance.right.sections = vec![stored];
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
        h.get_all_by_label(field)
            .find(|n| n.accesskit_node().role() != egui::accesskit::Role::Label)
            .unwrap()
            .focus();
        h.run();
        h.get_all_by_label(field)
            .find(|n| n.accesskit_node().role() == egui::accesskit::Role::SpinButton)
            .unwrap()
            .type_text("10");
        h.key_press(egui::Key::Enter);
        h.run();
        let actions = actions.borrow();
        let Some(Edit::Replace { section, .. }) = actions.last().map(|a| &a.edit) else {
            panic!("{field}: expected a replacement, got {:?}", actions.last());
        };
        assert_eq!((section.from(), section.to()), expected_span, "{field}");
        assert_eq!(
            (section.move_mode(), section.drag_mode()),
            expected_modes,
            "{field}"
        );
    }
}

fn click(h: &mut Harness, at: egui::Pos2) {
    h.drag_at(at);
    h.step();
    h.drop_at(at);
    h.run();
}
/// Where `[from, to]` mm of `side` is drawn, on a 600 x 300 mm panel shown in `content`.
fn centre(content: egui::Rect, side: Side, from: f64, to: f64) -> egui::Pos2 {
    let length = if side.vertical() { 300.0 } else { 600.0 };
    r::segment(r::band(content, side), side, g::Span::new(from, to), length).center()
}
#[test]
fn a_short_section_at_either_end_of_any_edge_is_selected_through_the_crossing_band() {
    // Full scale as in the review's reproduction, then a map shrunk to a quarter, where
    // the corner covers a larger share of the edge.
    for content in [
        egui::Rect::from_min_size(egui::pos2(40.0, 40.0), egui::vec2(600.0, 300.0)),
        egui::Rect::from_min_size(egui::pos2(40.0, 40.0), egui::vec2(150.0, 75.0)),
    ] {
        for side in Side::ALL {
            let length = if side.vertical() { 300.0 } else { 600.0 };
            for (from, to) in [(0.0, 10.0), (length - 10.0, length)] {
                let editor = RefCell::new(Editor::default());
                let monitors = RefCell::new(vec![monitor("a", 0.0, 0.0)]);
                side.sections_mut(&mut monitors.borrow_mut()[0].resistance)
                    .push(section(from, to));
                let actions = RefCell::new(vec![]);
                let mut h = bands_in(&editor, &monitors, &actions, content);
                h.run();
                let at = centre(content, side, from, to);
                assert_eq!(
                    r::owner(content, &monitors.borrow()[0], at),
                    Some(side),
                    "{side:?} [{from}, {to}] in {content:?}"
                );
                click(&mut h, at);
                assert_eq!(
                    editor.borrow().selected,
                    Some(Selection {
                        monitor: "a".into(),
                        side,
                        index: 0,
                    }),
                    "{side:?} [{from}, {to}] in {content:?}"
                );
            }
        }
    }
}
#[test]
fn a_corner_held_by_both_edges_is_cut_along_its_diagonal_and_a_gesture_keeps_its_edge() {
    let content = egui::Rect::from_min_size(egui::pos2(40.0, 40.0), egui::vec2(600.0, 300.0));
    let editor = RefCell::new(Editor::default());
    let monitors = RefCell::new(vec![monitor("a", 0.0, 0.0)]);
    monitors.borrow_mut()[0]
        .resistance
        .right
        .sections
        .push(section(250.0, 300.0));
    monitors.borrow_mut()[0]
        .resistance
        .bottom
        .sections
        .push(section(590.0, 600.0));
    let actions = RefCell::new(vec![]);
    let mut h = bands_in(&editor, &monitors, &actions, content);
    h.run();
    // Bottom-right corner is (628..640, 328..340): nearer the right edge, then the bottom.
    for (at, side) in [
        (egui::pos2(638.0, 333.0), Side::Right),
        (egui::pos2(633.0, 338.0), Side::Bottom),
    ] {
        assert_eq!(r::owner(content, &monitors.borrow()[0], at), Some(side));
        click(&mut h, at);
        assert_eq!(
            editor.borrow().selected.as_ref().map(|s| s.side),
            Some(side)
        );
    }

    // A gesture started in the corner stays on its edge until release, even when the
    // pointer is let go over a part only the bottom band covers.
    let editor = RefCell::new(Editor::default());
    let monitors = RefCell::new(vec![monitor("a", 0.0, 0.0)]);
    monitors.borrow_mut()[0]
        .resistance
        .right
        .sections
        .push(section(250.0, 300.0));
    let actions = RefCell::new(vec![]);
    let mut h = bands_in(&editor, &monitors, &actions, content);
    h.run();
    // 299 mm on the right edge: that section's end handle, in the corner.
    let (press, release) = (egui::pos2(638.0, 339.0), egui::pos2(600.0, 328.0));
    assert_eq!(
        r::owner(content, &monitors.borrow()[0], press),
        Some(Side::Right)
    );
    assert_eq!(
        r::owner(content, &monitors.borrow()[0], release),
        Some(Side::Bottom)
    );
    drag(&mut h, press, release);
    assert!(editor.borrow().active());
    h.drop_at(release);
    h.run();
    let log = actions.borrow();
    assert_eq!(log.len(), 1);
    assert_eq!(log[0].side, Side::Right);
    let Edit::Replace { index: 0, section } = log[0].edit else {
        panic!(
            "expected the right section to be resized, got {:?}",
            log[0].edit
        );
    };
    assert_eq!((section.from(), section.to()), (250.0, 289.0));
}
