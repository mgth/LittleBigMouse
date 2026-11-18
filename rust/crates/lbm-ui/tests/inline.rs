use egui_kittest::{
    kittest::{NodeT, Queryable},
    Harness,
};
use lbm_layout::{geo::Rect, model::DisplaySize};
use lbm_ui::{
    inline,
    map::{self, MapMonitor},
    properties::{Edit, Location, Size},
};
use std::cell::RefCell;

fn location() -> Location {
    Location {
        x: -25.,
        y: 10.,
        scale_x: 100.,
        scale_y: 100.,
    }
}
fn size() -> Size {
    Size {
        physical: DisplaySize {
            x: 0.,
            y: 0.,
            width: 600.,
            height: 340.,
            left_border: 10.,
            top_border: 10.,
            right_border: 10.,
            bottom_border: 15.,
        },
        locked: false,
        per_monitor_borders: true,
        model: "Monitor".into(),
    }
}
fn focus(h: &mut Harness, label: &str) {
    h.get_all_by_label(label)
        .find(|n| n.accesskit_node().role() != egui::accesskit::Role::Label)
        .unwrap()
        .focus();
    h.run();
}
fn type_value(h: &Harness, label: &str, text: &str) {
    h.get_all_by_label(label)
        .find(|n| n.accesskit_node().role() == egui::accesskit::Role::SpinButton)
        .unwrap()
        .type_text(text);
}

#[test]
fn typed_values_commit_on_enter_or_focus_loss_and_escape_cancels() {
    let edits = RefCell::new(vec![]);
    let mut h = Harness::builder()
        .with_size(egui::vec2(650., 400.))
        .build_ui(|ui| {
            let r = inline::location(ui, "a", ui.max_rect(), &location());
            edits.borrow_mut().extend(r.edits);
        });
    focus(&mut h, "X position");
    type_value(&h, "X position", "-123,45");
    h.run();
    assert!(edits.borrow().is_empty(), "no layout jump while typing");
    h.key_press(egui::Key::Enter);
    h.run();
    assert!(edits.borrow().contains(&Edit::X(-123.45)));
    edits.borrow_mut().clear();
    focus(&mut h, "Y position");
    type_value(&h, "Y position", "222");
    h.key_press(egui::Key::Escape);
    h.run();
    assert!(edits.borrow().is_empty(), "{:?}", edits.borrow());
    focus(&mut h, "X position");
    type_value(&h, "X position", "NaN");
    h.key_press(egui::Key::Enter);
    h.run();
    assert!(edits.borrow().is_empty(), "{:?}", edits.borrow());
    focus(&mut h, "X position");
    type_value(&h, "X position", "75");
    focus(&mut h, "Y position");
    assert!(edits.borrow().contains(&Edit::X(75.)));
}

#[test]
fn field_drag_changes_the_value_without_dragging_the_monitor() {
    let edits = RefCell::new(vec![]);
    let movements = RefCell::new(vec![]);
    let mut h = Harness::builder()
        .with_size(egui::vec2(700., 500.))
        .build_ui(|ui| {
            let monitors = [MapMonitor {
                id: "a",
                name: "Monitor",
                mm_outside: Rect::new(0., 0., 620., 360.),
                mm_content: Rect::new(10., 10., 600., 340.),
                logo: None,
                wallpaper: None,
                details: &[],
                probe: None,
            }];
            let fit = map::fit(map::extent(&monitors), ui.max_rect());
            let gesture = map::draw(ui, &monitors, &fit, None);
            let result = inline::location(ui, "a", fit.place(&monitors[0]).content, &location());
            if !result.engaged {
                if let Some(map::Gesture::Dragged { by, .. }) = gesture {
                    movements.borrow_mut().push(by);
                }
            }
            edits.borrow_mut().extend(result.edits);
        });
    h.run();
    let b = h
        .get_all_by_label("X position")
        .find(|n| n.accesskit_node().role() != egui::accesskit::Role::Label)
        .unwrap()
        .accesskit_node()
        .bounding_box()
        .unwrap();
    let start = egui::pos2(((b.x0 + b.x1) / 2.) as f32, ((b.y0 + b.y1) / 2.) as f32);
    h.event(egui::Event::PointerMoved(start));
    h.event(egui::Event::PointerButton {
        pos: start,
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: Default::default(),
    });
    h.run();
    let end = start + egui::vec2(40., 0.);
    h.event(egui::Event::PointerMoved(end));
    h.run();
    h.event(egui::Event::PointerButton {
        pos: end,
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: Default::default(),
    });
    h.run();
    assert!(
        edits
            .borrow()
            .iter()
            .any(|e| matches!(e,Edit::X(v) if *v > -25.)),
        "{:?}",
        edits.borrow()
    );
    assert!(movements.borrow().is_empty());
}

#[test]
fn compact_dimensions_remain_editable_and_disabled_views_do_not_write() {
    for enabled in [true, false] {
        let edits = RefCell::new(vec![]);
        let mut h = Harness::builder()
            .with_size(egui::vec2(280., 320.))
            .build_ui(|ui| {
                if !enabled {
                    ui.disable();
                }
                edits.borrow_mut().extend(
                    inline::size(
                        ui,
                        "portrait",
                        &lbm_ui::frame::Drawn {
                            outside: ui.max_rect(),
                            content: ui.max_rect(),
                            name_band: egui::Rect::NOTHING,
                            name_height: None,
                            logo_band: egui::Rect::NOTHING,
                        },
                        &size(),
                    )
                    .edits,
                );
            });
        if enabled {
            focus(&mut h, "Panel width");
            type_value(&h, "Panel width", "650");
            h.key_press(egui::Key::Enter);
            h.run();
            assert!(edits.borrow().contains(&Edit::Width(650.)));
        } else {
            let n = h
                .get_all_by_label("Panel width")
                .find(|n| n.accesskit_node().role() != egui::accesskit::Role::Label)
                .unwrap();
            assert!(n.accesskit_node().is_disabled());
            assert!(edits.borrow().is_empty(), "{:?}", edits.borrow());
        }
    }
}

#[cfg(not(windows))]
#[test]
fn inline_measurement_snapshots() {
    let mut results = egui_kittest::SnapshotResults::new();
    for (w, h, level) in [
        (760., 500., "wide"),
        (760., 500., "linked"),
        (330., 290., "compact"),
        (240., 190., "readonly"),
    ] {
        for dimensions in [false, true] {
            if level == "linked" && !dimensions {
                continue;
            }
            let mut harness = Harness::builder()
                .with_size(egui::vec2(w, h))
                .build_ui(|ui| {
                    lbm_ui::theme::install(ui.ctx());
                    egui::CentralPanel::default().show(ui, |ui| {
                        let m = MapMonitor {
                            id: "a",
                            name: "Studio Monitor",
                            mm_outside: Rect::new(0., 0., 620., 360.),
                            mm_content: Rect::new(10., 10., 600., 340.),
                            logo: None,
                            wallpaper: None,
                            details: &[],
                            probe: None,
                        };
                        let fit = map::fit(m.mm_outside, ui.max_rect());
                        map::draw(ui, &[m], &fit, Some("a"));
                        if dimensions {
                            let mut value = size();
                            value.locked = level == "linked";
                            inline::size(ui, "a", &fit.place(&m), &value);
                        } else {
                            inline::location(ui, "a", fit.place(&m).content, &location());
                        }
                    });
                });
            harness.run();
            let name = format!(
                "inline-{}-{}",
                if dimensions { "size" } else { "location" },
                level
            );
            harness.snapshot(&name);
            results.extend(harness.take_snapshot_results());
        }
    }
}

#[test]
fn wheel_uses_csharp_precision_modifiers_without_scrolling_the_fields() {
    let edits = RefCell::new(vec![]);
    let mut h = Harness::builder()
        .with_size(egui::vec2(250., 180.))
        .build_ui(|ui| {
            edits
                .borrow_mut()
                .extend(inline::location(ui, "a", ui.max_rect(), &location()).edits);
        });
    h.run();
    let bounds = |h: &Harness| {
        h.get_all_by_label("X position")
            .find(|n| n.accesskit_node().role() != egui::accesskit::Role::Label)
            .unwrap()
            .accesskit_node()
            .bounding_box()
            .unwrap()
    };
    let b = bounds(&h);
    let at = egui::pos2(((b.x0 + b.x1) / 2.) as f32, ((b.y0 + b.y1) / 2.) as f32);
    h.event(egui::Event::PointerMoved(at));
    h.step();
    for (modifiers, expected) in [
        (egui::Modifiers::NONE, -24.),
        (egui::Modifiers::CTRL, -24.9),
        (egui::Modifiers::SHIFT, -15.),
    ] {
        edits.borrow_mut().clear();
        h.event(egui::Event::MouseWheel {
            phase: egui::TouchPhase::Move,
            unit: egui::MouseWheelUnit::Line,
            delta: egui::vec2(0., 1.),
            modifiers,
        });
        h.step();
        assert!(
            edits.borrow().contains(&Edit::X(expected)),
            "{:?}",
            edits.borrow()
        );
        assert_eq!(bounds(&h), b);
    }
}

fn drawing(content: egui::Rect) -> lbm_ui::frame::Drawn {
    lbm_ui::frame::Drawn {
        content,
        outside: content.expand(5.),
        name_band: egui::Rect::NOTHING,
        name_height: None,
        logo_band: egui::Rect::NOTHING,
    }
}

#[test]
fn tiny_dimensions_keep_their_arrows_but_cannot_edit() {
    let edits = RefCell::new(vec![]);
    let content = egui::Rect::from_min_size(egui::pos2(20., 20.), egui::vec2(150., 90.));
    let mut h = Harness::builder()
        .with_size(egui::vec2(200., 140.))
        .build_ui(|ui| {
            edits
                .borrow_mut()
                .extend(inline::size(ui, "a", &drawing(content), &size()).edits);
        });
    h.run();
    assert!(h.query_by_role(egui::accesskit::Role::SpinButton).is_none());
    for (name, ..) in size().fields() {
        assert!(h.query_by_label(name).is_some());
    }
    let has_width_arrow = h.output().shapes.iter().any(|shape| match &shape.shape {
        egui::Shape::LineSegment { points, .. } => {
            points[0].x == content.left()
                && points[1].x == content.right()
                && points[0].y == points[1].y
        }
        _ => false,
    });
    assert!(
        has_width_arrow,
        "the panel-width arrow must still reach both real edges"
    );
    h.event(egui::Event::PointerMoved(content.center()));
    h.event(egui::Event::MouseWheel {
        phase: egui::TouchPhase::Move,
        unit: egui::MouseWheelUnit::Point,
        delta: egui::vec2(0., 20.),
        modifiers: Default::default(),
    });
    h.run();
    assert!(edits.borrow().is_empty());
}

#[test]
fn the_axis_link_toggles_by_click_and_keyboard_and_follows_external_changes() {
    for window in [egui::vec2(760., 500.), egui::vec2(330., 290.)] {
        let mut h = Harness::builder().with_size(window).build_ui_state(
            |ui, value| {
                let result = inline::size(ui, "a", &drawing(ui.max_rect()), value);
                for edit in result.edits {
                    let Edit::Lock(locked) = edit else {
                        panic!("the link edited a dimension")
                    };
                    assert!(result.engaged, "the link must capture its gesture");
                    value.locked = locked;
                }
            },
            size(),
        );
        h.run();
        h.get_by_label("Keep aspect ratio").click();
        h.run();
        assert!(h.state().locked);
        h.get_by_label("Keep aspect ratio").focus();
        h.key_press(egui::Key::Space);
        h.run();
        assert!(!h.state().locked);
        h.state_mut().locked = true;
        h.run();
        h.get_by_label("Keep aspect ratio").click();
        h.run();
        assert!(
            !h.state().locked,
            "the link must follow the properties panel's state"
        );
    }
}

#[test]
fn highlights_link_hover_and_focus_to_the_matching_monitor_only() {
    highlights(true);
}

#[test]
fn location_highlights_link_hover_and_focus_to_the_matching_monitor_only() {
    highlights(false);
}

fn highlights(dimensions: bool) {
    let edge_name = if dimensions {
        "Left bezel"
    } else {
        "X position"
    };
    let focus_name = if dimensions {
        "Panel width"
    } else {
        "Vertical scale"
    };
    use lbm_ui::properties;
    let mut h = Harness::builder()
        .with_size(egui::vec2(800., 480.))
        .build_ui(|ui| {
            lbm_ui::theme::install(ui.ctx());
            ui.scope_builder(
                egui::UiBuilder::new().max_rect(egui::Rect::from_min_size(
                    egui::pos2(10., 10.),
                    egui::vec2(300., 450.),
                )),
                |ui| {
                    if dimensions {
                        properties::size(ui, "a", &size());
                    } else {
                        properties::location(ui, "a", &location());
                    }
                },
            );
            for (id, x) in [("a", 340.), ("b", 560.)] {
                let content =
                    egui::Rect::from_min_size(egui::pos2(x, 130.), egui::vec2(170., 100.));
                if dimensions {
                    inline::size(ui, id, &drawing(content), &size());
                } else {
                    inline::location(ui, id, content, &location());
                }
            }
            properties::paint_measurement_highlights(ui.ctx());
        });
    h.run();
    let field_center = |h: &Harness, name: &str, left: f64, right: f64| {
        let b = h
            .get_all_by_label(name)
            .find_map(|n| {
                let b = n.accesskit_node().bounding_box()?;
                (b.x0 >= left
                    && b.x1 <= right
                    && (left > 300.
                        || n.accesskit_node().role() == egui::accesskit::Role::SpinButton))
                    .then_some(b)
            })
            .unwrap();
        egui::pos2(((b.x0 + b.x1) / 2.) as f32, ((b.y0 + b.y1) / 2.) as f32)
    };
    let outlined = |h: &Harness, point| {
        h.output().shapes.iter().any(|s| {
            matches!(&s.shape,
        egui::Shape::Rect(r) if r.stroke.color == lbm_ui::theme::ACCENT && r.rect.contains(point))
        })
    };
    let a = field_center(&h, edge_name, 340., 520.);
    let b = field_center(&h, edge_name, 560., 740.);
    let panel = field_center(&h, edge_name, 0., 320.);
    h.event(egui::Event::PointerMoved(a));
    h.run();
    assert!(outlined(&h, a) && outlined(&h, panel));
    assert!(
        !outlined(&h, b),
        "same field name on another screen must not light up"
    );
    h.event(egui::Event::PointerMoved(egui::pos2(790., 470.)));
    h.get_all_by_label(focus_name)
        .find(|n| n.accesskit_node().role() == egui::accesskit::Role::SpinButton)
        .unwrap()
        .focus();
    h.run();
    assert!(outlined(&h, field_center(&h, focus_name, 340., 520.)));
    assert!(!outlined(&h, field_center(&h, focus_name, 560., 740.)));
    assert!(
        !outlined(&h, a),
        "the former hovered field must lose its outline"
    );
}

#[test]
fn location_compact_scales_stay_editable_and_tiny_values_are_read_only() {
    for (width, height, editable) in [(280., 170., true), (170., 100., false)] {
        let edits = RefCell::new(vec![]);
        let mut h = Harness::builder()
            .with_size(egui::vec2(width, height))
            .build_ui(|ui| {
                edits
                    .borrow_mut()
                    .extend(inline::location(ui, "a", ui.max_rect(), &location()).edits);
            });
        h.run();
        if editable {
            focus(&mut h, "Horizontal scale");
            type_value(&h, "Horizontal scale", "125,5");
            h.key_press(egui::Key::Enter);
            h.run();
            assert!(edits.borrow().contains(&Edit::ScaleX(125.5)));
        } else {
            assert!(h.query_by_role(egui::accesskit::Role::SpinButton).is_none());
            for (name, ..) in location().fields() {
                assert!(h.query_by_label(name).is_some());
            }
            h.get_by_label("Horizontal scale").click();
            h.step();
            h.event(egui::Event::MouseWheel {
                phase: egui::TouchPhase::Move,
                unit: egui::MouseWheelUnit::Line,
                delta: egui::vec2(0., 1.),
                modifiers: Default::default(),
            });
            h.step();
            assert!(edits.borrow().is_empty());
        }
    }
}
