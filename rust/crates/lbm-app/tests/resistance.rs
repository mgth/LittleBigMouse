use lbm_app::{
    resistance::{apply, monitors},
    saved::Reference,
    settings,
};
use lbm_layout::{
    geo::Point,
    linux::{LinuxEdid, LinuxMonitor},
    model::{BorderSection, Layout, LayoutOptions},
};
use lbm_ui::resistance::{Action, Edit, Side};

fn layout() -> Layout {
    let mut layout = Layout::new(LayoutOptions {
        allow_discontinuity: true,
        ..Default::default()
    });
    let outputs: Vec<_> = (0..2)
        .map(|i| LinuxMonitor {
            connector_name: format!("DP-{i}"),
            primary: i == 0,
            logical_x: f64::from(i * 1920),
            width_mm: 600.0,
            height_mm: 300.0,
            edid: Some(LinuxEdid {
                manufacturer_code: Some("AAA".into()),
                product_code: Some("0001".into()),
                serial: Some(format!("{i:08}")),
                physical_width: 600.0,
                physical_height: 300.0,
                ..Default::default()
            }),
            ..LinuxMonitor::fallback()
        })
        .collect();
    lbm_layout::linux::populate(&mut layout, &outputs, |_| Ok::<(), ()>(())).unwrap();
    let id = layout.monitors()[1].id.clone();
    layout.set_location(&id, Point::new(620.0, 60.0));
    layout
}
fn edit(
    layout: &mut Layout,
    side: Side,
    edit: Edit,
) -> Result<Option<lbm_ui::resistance::Selection>, String> {
    let action = Action::new(&monitors(layout)[0], side, edit);
    apply(layout, action)
}
fn section(from: f64, to: f64) -> BorderSection {
    BorderSection::new(from, to, 4.0, false, 9.0, true)
}

#[test]
fn unsectioned_crossings_use_independent_defaults_but_explicit_sections_always_win() {
    use lbm_layout::model::{ResistanceDefaults, ResistanceMode};
    for movement in [false, true] {
        for drag in [false, true] {
            for loops in [false, true] {
                let mut edited = layout();
                let saved = Reference::of(&edited);
                edited.edit_options(|o| {
                    o.resistance_defaults = ResistanceDefaults {
                        movement: 7.5,
                        drag: 18.,
                        move_without_section: movement,
                        drag_without_section: drag,
                    };
                    o.loop_x = loops;
                    o.loop_y = loops;
                });
                for (from, to, mode) in [
                    (100., 140., ResistanceMode::Free),
                    (160., 200., ResistanceMode::Custom),
                    (220., 240., ResistanceMode::Block),
                    (260., 280., ResistanceMode::Resist),
                ] {
                    let mut s = section(from, to);
                    s.set_move_mode(mode);
                    s.set_drag_mode(mode);
                    edit(&mut edited, Side::Right, Edit::Insert(s)).unwrap();
                }
                let document = serde_json::to_string(&settings::document(&edited)).unwrap();
                let document: lbm_store::LayoutDocument = serde_json::from_str(&document).unwrap();
                let mut landed = layout();
                // Loading an explicit false must also turn a previously enabled switch off.
                landed.options.resistance_defaults.move_without_section = !movement;
                landed.options.resistance_defaults.drag_without_section = !drag;
                document.apply(&mut landed);
                assert_eq!(settings::document(&landed), settings::document(&edited));
                let zones = lbm_layout::zoning::compute_zones(&landed);
                let source = landed.monitors()[0].active_source.as_ref().unwrap();
                let zone = zones
                    .zones
                    .iter()
                    .find(|z| z.device_id.as_ref() == Some(source))
                    .unwrap();
                let inherited = (
                    if movement { 7.5 } else { 0. },
                    if drag { 18. } else { 0. },
                    false,
                    false,
                );
                for (y, expected) in [
                    (80., inherited),
                    (120., (0., 0., false, false)),
                    (150., inherited),
                    (180., (4., 9., false, false)),
                    (210., inherited),
                    (230., (4., 9., true, true)),
                    (250., inherited),
                    (270., (7.5, 18., false, false)),
                    (290., inherited),
                ] {
                    let link = zone
                        .right_links
                        .iter()
                        .find(|l| l.from <= y && y < l.to)
                        .unwrap();
                    assert!(link.target.is_some());
                    assert_eq!(
                        (
                            link.border_resistance,
                            link.drag_resistance,
                            link.move_block,
                            link.drag_block
                        ),
                        expected,
                        "at {y}, Move={movement}, Drag={drag}, loops={loops}"
                    );
                }
                // The other screen has no section at all: all its crossings inherit.
                let other = landed.monitors()[1].active_source.as_ref().unwrap();
                let other = zones
                    .zones
                    .iter()
                    .find(|z| z.device_id.as_ref() == Some(other))
                    .unwrap();
                let links: Vec<_> = other
                    .left_links
                    .iter()
                    .filter(|l| l.target.is_some())
                    .collect();
                assert!(!links.is_empty());
                for link in links {
                    assert_eq!(
                        (
                            link.border_resistance,
                            link.drag_resistance,
                            link.move_block,
                            link.drag_block
                        ),
                        inherited
                    );
                }
                saved.restore(&mut edited);
                assert!(saved.holds(&edited));
                assert!(!edited.options.resistance_defaults.move_without_section);
                assert!(!edited.options.resistance_defaults.drag_without_section);
            }
        }
    }
}

#[test]
fn resistance_modes_survive_saving_and_defaults_reach_links_and_real_screen_bands() {
    use lbm_layout::model::{ResistanceDefaults, ResistanceMode::*};
    for (movement, move_value, move_block) in [
        (Block, 4., true),
        (Resist, 11., false),
        (Free, 0., false),
        (Custom, 4., false),
    ] {
        for (drag, drag_value, drag_block) in [
            (Block, 9., true),
            (Resist, 23., false),
            (Free, 0., false),
            (Custom, 9., false),
        ] {
            let mut edited = layout();
            let saved = Reference::of(&edited);
            let mut section = section(80., 180.);
            section.set_move_mode(movement);
            section.set_drag_mode(drag);
            edit(&mut edited, Side::Right, Edit::Insert(section)).unwrap();
            edited.mark_saved();
            edited.edit_options(|o| {
                o.resistance_defaults = ResistanceDefaults {
                    movement: 11.,
                    drag: 23.,
                    ..Default::default()
                }
            });
            assert!(!edited.saved());
            assert!(!saved.holds(&edited));
            let document = serde_json::to_string(&settings::document(&edited)).unwrap();
            let document: lbm_store::LayoutDocument = serde_json::from_str(&document).unwrap();
            let mut landed = layout();
            document.apply(&mut landed);
            assert_eq!(settings::document(&landed), settings::document(&edited));
            let monitor = &landed.monitors()[0];
            assert_eq!(monitor.border_resistance.right.sections[0], section);
            let source = monitor.active_source.as_ref().unwrap();
            let zones = lbm_layout::zoning::compute_zones(&landed);
            let zone = zones
                .zones
                .iter()
                .find(|z| z.device_id.as_ref() == Some(source))
                .unwrap();
            assert!(
                zone.right_links.iter().any(|l| l.target.is_some()
                    && l.border_resistance == move_value
                    && l.drag_resistance == drag_value
                    && l.move_block == move_block
                    && l.drag_block == drag_block),
                "{movement:?}/{drag:?}"
            );
            let bands =
                lbm_overlay::scene::from_layout(&landed, lbm_overlay::Mode::Resistance, None)
                    .unwrap();
            let band = bands
                .iter()
                .find(|b| b.monitor == monitor.id && b.side == lbm_overlay::scene::Side::Right)
                .unwrap();
            let lbm_overlay::scene::Content::Resistance(sections) = &band.content else {
                panic!("resistance band")
            };
            assert_eq!(
                (
                    sections[0].move_resistance(),
                    sections[0].drag(),
                    sections[0].move_block(),
                    sections[0].drag_block()
                ),
                (move_value, drag_value, move_block, drag_block)
            );
            // Custom values remain available after Free, Block or inherited resistance.
            assert_eq!((section.move_resistance(), section.drag()), (4., 9.));
            saved.restore(&mut edited);
            assert!(saved.holds(&edited));
        }
    }
}
#[test]
fn reference_feedback_is_projected_without_entering_the_saved_document() {
    use lbm_overlay::{
        scene::{self, Content, GuideKind},
        Mode,
    };
    use lbm_ui::resistance::{Guide, SnapKind};
    let layout = layout();
    let document = settings::document(&layout);
    for side in Side::ALL {
        for (kind, native) in [
            (SnapKind::Middle, GuideKind::Middle),
            (SnapKind::ScreenEdge, GuideKind::ScreenEdge),
            (SnapKind::Section, GuideKind::Section),
        ] {
            let guide = Guide {
                monitor: layout.monitors()[0].id.clone(),
                side,
                mm: 120.,
                layout_mm: 120.,
                kind,
            };
            let r = lbm_app::resistance::overlay_reference(guide.clone()).unwrap();
            assert_eq!(r.horizontal, side.vertical());
            assert_eq!(r.kind, native);
            let bands = scene::with_reference(&layout, Mode::Resistance, None, Some(r)).unwrap();
            assert!(bands
                .iter()
                .any(|b| matches!(b.content,Content::Guide{kind,..} if kind==native)));
            assert_eq!(
                side.vertical(),
                bands.last().unwrap().side == scene::Side::Top
            );
            let colors = kind.color().to_array();
            assert_eq!(&colors[..3], &native.rgba()[..3]);
            assert!(lbm_app::resistance::overlay_reference(Guide {
                kind: SnapKind::EdgeEnd,
                ..guide
            })
            .is_none());
        }
    }
    assert_eq!(settings::document(&layout), document);
}

#[test]
fn sections_survive_save_and_preview_documents_and_reach_the_compiled_links() {
    let mut edited = layout();
    let saved = Reference::of(&edited);
    edit(&mut edited, Side::Right, Edit::Insert(section(80.0, 180.0))).unwrap();
    assert!(!saved.holds(&edited));
    let (_, request) = settings::save_layout(&edited);
    let document: lbm_store::LayoutDocument =
        serde_json::from_value(request["Document"].clone()).unwrap();
    assert_eq!(document, settings::document(&edited));
    let mut landed = layout();
    document.apply(&mut landed);
    assert!(Reference::of(&landed).holds(&edited));
    let source = landed.monitors()[0].active_source.clone().unwrap();
    let zones = lbm_layout::zoning::compute_zones(&landed);
    let zone = zones
        .zones
        .iter()
        .find(|z| z.device_id.as_ref() == Some(&source))
        .unwrap();
    assert!(zone.right_links.iter().any(|l| l.target.is_some()
        && l.border_resistance == 4.0
        && l.drag_resistance == 9.0
        && l.drag_block
        && !l.move_block));
    saved.restore(&mut edited);
    assert!(saved.holds(&edited));
    assert!(monitors(&edited)[0].resistance.right.sections.is_empty());
}
#[test]
fn mirror_projects_into_the_facing_screen_and_preserves_all_four_values() {
    let mut layout = layout();
    edit(&mut layout, Side::Right, Edit::Insert(section(80.0, 180.0))).unwrap();
    edit(&mut layout, Side::Right, Edit::Mirror(0)).unwrap();
    let got = monitors(&layout)[1].resistance.left.sections[0];
    assert_eq!(got, section(20.0, 120.0));
    assert!(edit(&mut layout, Side::Right, Edit::Mirror(0)).is_err());
}
#[test]
fn stale_selection_and_invalid_intervals_cannot_modify_another_section() {
    let mut layout = layout();
    edit(&mut layout, Side::Top, Edit::Insert(section(20.0, 80.0))).unwrap();
    let stale = Action::new(&monitors(&layout)[0], Side::Top, Edit::Delete(0));
    edit(
        &mut layout,
        Side::Top,
        Edit::Replace {
            index: 0,
            section: section(30.0, 80.0),
        },
    )
    .unwrap();
    let saved = Reference::of(&layout);
    assert!(apply(&mut layout, stale).is_err());
    for s in [
        section(40.0, 100.0),
        section(1.0, 2.0),
        section(0.0, 601.0),
        section(90.0, f64::INFINITY),
        section(f64::NAN, 100.0),
        BorderSection::new(100.0, 120.0, f64::NAN, false, 0.0, false),
    ] {
        assert!(edit(&mut layout, Side::Top, Edit::Insert(s)).is_err());
        assert!(saved.holds(&layout));
    }
}
#[test]
fn deleting_a_section_keeps_the_other_sections_and_clears_selection() {
    let mut layout = layout();
    edit(&mut layout, Side::Bottom, Edit::Insert(section(20.0, 80.0))).unwrap();
    edit(
        &mut layout,
        Side::Bottom,
        Edit::Insert(section(80.0, 100.0)),
    )
    .unwrap();
    assert_eq!(
        edit(&mut layout, Side::Bottom, Edit::Delete(0)).unwrap(),
        None
    );
    assert_eq!(
        monitors(&layout)[0].resistance.bottom.sections,
        [section(80.0, 100.0)]
    );
}
#[test]
fn section_count_is_bounded_and_rotation_uses_the_projected_edge_length() {
    let mut layout = layout();
    let source = layout.monitors()[0].active_source.clone().unwrap();
    layout.set_source_orientation(&source, 1);
    assert_eq!(Side::Right.length(monitors(&layout)[0].bounds), 600.0);
    edit(
        &mut layout,
        Side::Right,
        Edit::Insert(section(400.0, 500.0)),
    )
    .unwrap();
    for i in 0..16 {
        edit(
            &mut layout,
            Side::Top,
            Edit::Insert(section(f64::from(i * 10), f64::from(i * 10 + 5))),
        )
        .unwrap();
    }
    assert!(edit(&mut layout, Side::Top, Edit::Insert(section(200.0, 210.0))).is_err());
}
