use lbm_app::{screen_input::Controller, settings};
use lbm_layout::{
    geo::Point,
    linux::{add_monitor, LinuxMonitor},
    model::{Layout, LayoutOptions},
};
use lbm_overlay::{
    input::{Input, Kind},
    scene::{self, Band, Side},
    Mode,
};
use lbm_ui::resistance::Editor;
fn layout() -> Layout {
    let mut layout = Layout::new(LayoutOptions {
        allow_discontinuity: true,
        ..Default::default()
    });
    for (id, x) in [("A", 0.), ("B", 1920.)] {
        add_monitor(
            &mut layout,
            &LinuxMonitor {
                connector_name: id.into(),
                logical_x: x,
                width_mm: 600.,
                height_mm: 300.,
                primary: id == "A",
                ..LinuxMonitor::fallback()
            },
        );
    }
    layout.set_location("B", Point::new(620., 60.));
    layout
}
fn band(layout: &Layout, mode: Mode, side: Side) -> Band {
    scene::from_layout(layout, mode, Some("A"))
        .unwrap()
        .into_iter()
        .find(|b| b.monitor == "B" && b.side == side)
        .unwrap()
}
fn input(band: &Band, kind: Kind, at: f64, time: u32) -> Input {
    let mut input = Input::new(
        band,
        kind,
        if band.side.vertical() {
            (10., at)
        } else {
            (at, 10.)
        },
        1.,
        time,
        true,
        false,
    );
    // Logical surface coordinates, at 2 points/mm. Test in millimetres below.
    input.length = band.axis_mm * 2.;
    input.at = at * 2.;
    input.thickness = 40.;
    input
}
#[test]
fn rulers_drag_each_axis_inversely_and_commit_only_on_release() {
    for side in Side::ALL {
        let mut layout = layout();
        let original = settings::document(&layout);
        let band = band(&layout, Mode::Rulers, side);
        let location = layout.monitor("B").unwrap().location();
        let mut control = Controller::default();
        let mut editor = Editor::default();
        control
            .handle(
                &mut layout,
                &mut editor,
                input(&band, Kind::Press, 100., 10),
            )
            .unwrap();
        control
            .handle(&mut layout, &mut editor, input(&band, Kind::Move, 120., 20))
            .unwrap();
        assert_eq!(settings::document(&layout), original);
        let preview = control.preview().unwrap().monitor("B").unwrap().location();
        let delta = 40. * band.axis_mm / (band.axis_mm * 2. - 16.5);
        if side.vertical() {
            assert!((preview.y - location.y + delta).abs() < 1e-9);
            assert_eq!(preview.x, location.x);
        } else {
            assert!((preview.x - location.x + delta).abs() < 1e-9);
            assert_eq!(preview.y, location.y);
        }
        let out = control
            .handle(
                &mut layout,
                &mut editor,
                input(&band, Kind::Release, 120., 30),
            )
            .unwrap();
        assert!(out.changed);
        assert!(control.preview().is_none());
        assert_ne!(settings::document(&layout), original);
    }
}
#[test]
fn ruler_wheel_scales_around_pointer_with_ctrl_and_shift_precision() {
    for (ctrl, shift, step) in [
        (false, false, 0.01),
        (true, false, 0.001),
        (false, true, 0.1),
        (true, true, 0.01),
    ] {
        for side in [Side::Top, Side::Right] {
            let mut layout = layout();
            let band = band(&layout, Mode::Rulers, side);
            let before = layout
                .depth_projection(layout.monitor("B").unwrap())
                .unwrap()
                .bounds();
            let mut event = input(&band, Kind::Wheel(1.), 100., 10);
            event.ctrl = ctrl;
            event.shift = shift;
            let fraction = event.at / (event.length - 16.5);
            let mut control = Controller::default();
            let mut editor = Editor::default();
            assert!(
                control
                    .handle(&mut layout, &mut editor, event)
                    .unwrap()
                    .changed
            );
            let m = layout.monitor("B").unwrap();
            let after = layout.depth_projection(m).unwrap().bounds();
            let (old, new, ratio) = if side.vertical() {
                (
                    before.top() + before.height() * fraction,
                    after.top() + after.height() * fraction,
                    m.depth_ratio.y,
                )
            } else {
                (
                    before.left() + before.width() * fraction,
                    after.left() + after.width() * fraction,
                    m.depth_ratio.x,
                )
            };
            assert!((ratio - 1. - step).abs() < 1e-9);
            assert!((old - new).abs() < 1e-9);
        }
    }
}
#[test]
fn resistance_draw_resize_move_double_click_and_delete_share_the_document() {
    let mut layout = layout();
    let original = settings::document(&layout);
    let b = band(&layout, Mode::Resistance, Side::Right);
    let mut control = Controller::default();
    let mut editor = Editor::default();
    for (kind, at, time) in [(Kind::Press, 40., 10), (Kind::Move, 120., 20)] {
        control
            .handle(&mut layout, &mut editor, input(&b, kind, at, time))
            .unwrap();
    }
    assert!(editor.external_active());
    assert!(editor.preview_action().is_some());
    assert_eq!(settings::document(&layout), original);
    control
        .handle(&mut layout, &mut editor, input(&b, Kind::Release, 120., 30))
        .unwrap();
    assert_eq!(
        layout
            .monitor("B")
            .unwrap()
            .border_resistance
            .right
            .sections[0]
            .from(),
        40.
    );
    // Resize from, then move the middle. The band snapshot needn't be recreated mid-gesture.
    for (from, to, time) in [(41., 51., 1000), (80., 100., 2000)] {
        control
            .handle(&mut layout, &mut editor, input(&b, Kind::Press, from, time))
            .unwrap();
        control
            .handle(
                &mut layout,
                &mut editor,
                input(&b, Kind::Move, to, time + 10),
            )
            .unwrap();
        control
            .handle(
                &mut layout,
                &mut editor,
                input(&b, Kind::Release, to, time + 20),
            )
            .unwrap();
    }
    let s = layout
        .monitor("B")
        .unwrap()
        .border_resistance
        .right
        .sections[0];
    assert_eq!((s.from(), s.to()), (70., 140.));
    for time in [3000, 3100] {
        control
            .handle(&mut layout, &mut editor, input(&b, Kind::Press, 100., time))
            .unwrap();
        control
            .handle(
                &mut layout,
                &mut editor,
                input(&b, Kind::Release, 100., time + 10),
            )
            .unwrap();
    }
    let s = layout
        .monitor("B")
        .unwrap()
        .border_resistance
        .right
        .sections[0];
    assert_eq!((s.from(), s.to()), (0., b.axis_mm));
    control
        .handle(
            &mut layout,
            &mut editor,
            input(&b, Kind::Delete, 100., 4000),
        )
        .unwrap();
    assert!(layout
        .monitor("B")
        .unwrap()
        .border_resistance
        .right
        .sections
        .is_empty());
}
#[test]
fn escape_and_capture_loss_discard_gestures_without_saving_or_hiding_on_first_escape() {
    for mode in [Mode::Rulers, Mode::Resistance] {
        for cancel in [Kind::Escape, Kind::Cancel] {
            let mut layout = layout();
            let original = settings::document(&layout);
            let b = band(&layout, mode, Side::Bottom);
            let mut control = Controller::default();
            let mut editor = Editor::default();
            control
                .handle(&mut layout, &mut editor, input(&b, Kind::Press, 50., 10))
                .unwrap();
            control
                .handle(&mut layout, &mut editor, input(&b, Kind::Move, 100., 20))
                .unwrap();
            assert!(
                !control
                    .handle(&mut layout, &mut editor, input(&b, cancel, 100., 30))
                    .unwrap()
                    .hide
            );
            control
                .handle(&mut layout, &mut editor, input(&b, Kind::Release, 100., 40))
                .unwrap();
            assert_eq!(settings::document(&layout), original);
            assert!(control.preview().is_none());
            assert!(!editor.active());
            assert!(
                control
                    .handle(&mut layout, &mut editor, input(&b, Kind::Escape, 100., 50))
                    .unwrap()
                    .hide
            );
        }
    }
}
#[test]
fn another_edit_invalidates_a_ruler_drag_instead_of_overwriting_it() {
    let mut layout = layout();
    let b = band(&layout, Mode::Rulers, Side::Left);
    let mut control = Controller::default();
    let mut editor = Editor::default();
    control
        .handle(&mut layout, &mut editor, input(&b, Kind::Press, 50., 10))
        .unwrap();
    layout.set_location("A", Point::new(100., 100.));
    let edited = settings::document(&layout);
    control
        .handle(&mut layout, &mut editor, input(&b, Kind::Release, 100., 20))
        .unwrap();
    assert_eq!(settings::document(&layout), edited);
    assert!(!control.active());
}

/// Each edge is its own window, cut along the corner's diagonals. The pointer reaches a
/// screen's edge first, so the outermost row must always belong to that edge's section.
#[test]
fn a_short_section_at_either_end_of_every_edge_can_be_selected_on_the_screen() {
    for side in Side::ALL {
        let length = band(&layout(), Mode::Resistance, side).axis_mm;
        for (from, to) in [(0., 10.), (length - 10., length)] {
            let mut layout = layout();
            let editing = lbm_app::screen_input::side(side);
            layout.edit_border_resistance("B", |borders| {
                editing
                    .sections_mut(borders)
                    .push(lbm_layout::model::BorderSection::new(
                        from, to, 4., false, 9., false,
                    ));
            });
            let b = band(&layout, Mode::Resistance, side);
            let along = b.pixel_at((from + to) / 2.) as i32;
            let outermost = b.thickness() - 1;
            let (x, y) = match side {
                Side::Top => (along, 0),
                Side::Bottom => (along, outermost),
                Side::Left => (0, along),
                Side::Right => (outermost, along),
            };
            assert!(
                lbm_overlay::input::regions(&b)
                    .iter()
                    .any(|r| x >= r.x && x < r.x + r.width && y >= r.y && y < r.y + r.height),
                "{side:?} [{from}, {to}] has no input at ({x}, {y})"
            );
            let mut control = Controller::default();
            let mut editor = Editor::default();
            let at = (from + to) / 2.;
            for (kind, time) in [(Kind::Press, 10), (Kind::Release, 20)] {
                control
                    .handle(&mut layout, &mut editor, input(&b, kind, at, time))
                    .unwrap();
            }
            let selected = editor.selected.expect("the section is selected");
            assert_eq!(
                (selected.monitor.as_str(), selected.side, selected.index),
                ("B", editing, 0),
                "{side:?} [{from}, {to}]"
            );
        }
    }
}
