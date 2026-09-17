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
