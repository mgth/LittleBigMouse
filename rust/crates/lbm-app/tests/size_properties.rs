use lbm_app::properties::{apply, size};
use lbm_layout::{
    linux::{LinuxEdid, LinuxMonitor},
    model::{Layout, LayoutOptions, PER_MODEL, PER_MONITOR},
};
use lbm_ui::properties::Edit;

fn layout(policy: &str) -> Layout {
    let mut layout = Layout::new(LayoutOptions {
        border_values: policy.into(),
        ..Default::default()
    });
    let outputs: Vec<_> = (0..2)
        .map(|i| LinuxMonitor {
            connector_name: format!("DP-{i}"),
            primary: i == 0,
            logical_x: f64::from(i * 1920),
            edid: Some(LinuxEdid {
                manufacturer_code: Some("AAA".into()),
                product_code: Some("0001".into()),
                serial: Some(format!("{i:08}")),
                physical_width: 600.0,
                physical_height: 300.0,
                ..Default::default()
            }),
            width_mm: 600.0,
            height_mm: 300.0,
            ..LinuxMonitor::fallback()
        })
        .collect();
    lbm_layout::linux::populate(&mut layout, &outputs, |_| Ok::<(), ()>(())).unwrap();
    let model = layout.monitors()[0].model.clone();
    layout.edit_model(&model, |s, _| {
        s.set_left_border(10.0);
        s.set_top_border(11.0);
        s.set_right_border(12.0);
        s.set_bottom_border(13.0);
    });
    layout.mark_saved();
    layout
}

#[test]
fn portrait_dimensions_keep_the_ratio_and_update_every_screen_of_the_model() {
    let mut layout = layout(PER_MONITOR);
    let id = layout.monitors()[0].id.clone();
    let other = layout.monitors()[1].id.clone();
    let source = layout.monitors()[0].active_source.clone().unwrap();
    layout.set_source_orientation(&source, 1);
    apply(&mut layout, &id, Edit::Lock(true)).unwrap();
    apply(&mut layout, &id, Edit::Width(450.0)).unwrap();
    let portrait = size(&layout, &id).unwrap().physical;
    assert_eq!((portrait.width, portrait.height), (450.0, 900.0));
    let landscape = size(&layout, &other).unwrap().physical;
    assert_eq!((landscape.width, landscape.height), (900.0, 450.0));
}

#[test]
fn every_rotation_and_border_policy_edits_the_side_the_user_sees() {
    for policy in [PER_MODEL, PER_MONITOR] {
        for rotation in 0..4 {
            for edit in [
                Edit::Left(30.0),
                Edit::Top(31.0),
                Edit::Right(32.0),
                Edit::Bottom(33.0),
            ] {
                let mut layout = layout(policy);
                let id = layout.monitors()[0].id.clone();
                let other = layout.monitors()[1].id.clone();
                let source = layout.monitors()[0].active_source.clone().unwrap();
                layout.set_source_orientation(&source, rotation);
                let other_before = size(&layout, &other).unwrap().physical;
                apply(&mut layout, &id, edit).unwrap();
                let got = size(&layout, &id).unwrap().physical;
                match edit {
                    Edit::Left(v) => assert_eq!(got.left_border, v),
                    Edit::Top(v) => assert_eq!(got.top_border, v),
                    Edit::Right(v) => assert_eq!(got.right_border, v),
                    Edit::Bottom(v) => assert_eq!(got.bottom_border, v),
                    _ => unreachable!(),
                }
                if policy == PER_MONITOR {
                    assert_eq!(size(&layout, &other).unwrap().physical, other_before);
                    assert!(layout.monitor(&id).unwrap().borders_customized());
                } else {
                    assert_ne!(size(&layout, &other).unwrap().physical, other_before);
                }
            }
        }
    }
}

#[test]
fn outside_measurements_adjust_only_the_expected_bezels_and_survive_the_save_document() {
    for policy in [PER_MODEL, PER_MONITOR] {
        let mut edited = layout(policy);
        let id = edited.monitors()[0].id.clone();
        let before = size(&edited, &id).unwrap().physical;
        apply(
            &mut edited,
            &id,
            Edit::OutsideWidth(before.outside_width() + 20.0),
        )
        .unwrap();
        apply(
            &mut edited,
            &id,
            Edit::OutsideHeight(before.outside_height() + 15.0),
        )
        .unwrap();
        let after = size(&edited, &id).unwrap().physical;
        assert_eq!((after.left_border, after.right_border), (20.0, 22.0));
        assert_eq!((after.top_border, after.bottom_border), (11.0, 28.0));
        let (_, request) = lbm_app::settings::save_layout(&edited);
        let document: lbm_store::LayoutDocument =
            serde_json::from_value(request["Document"].clone()).unwrap();
        let mut landed = layout(policy);
        document.apply(&mut landed);
        assert_eq!(size(&landed, &id).unwrap().physical, after);
        let reference = lbm_app::saved::Reference::of(&landed);
        assert!(reference.holds(&edited));
    }
}

#[test]
fn invalid_measurements_are_rejected_without_changing_the_document() {
    let mut layout = layout(PER_MONITOR);
    let id = layout.monitors()[0].id.clone();
    let reference = lbm_app::saved::Reference::of(&layout);
    for edit in [
        Edit::Width(0.0),
        Edit::Width(f64::NAN),
        Edit::Height(f64::INFINITY),
        Edit::Left(-1.0),
        Edit::OutsideWidth(1.0),
        Edit::OutsideHeight(1.0),
    ] {
        assert!(apply(&mut layout, &id, edit).is_err());
        assert!(reference.holds(&layout));
    }
}

#[test]
fn reported_dimensions_remain_distinct_from_the_edited_layout() {
    let detected = serde_json::json!({"Size":{"Width":600},"Edid":null});
    let report = lbm_app::properties::report(&detected);
    assert!(report.contains(&("Size / Width".into(), "600".into())));
    assert!(report.contains(&("Edid".into(), "Not reported".into())));
    let mut layout = layout(PER_MODEL);
    let id = layout.monitors()[0].id.clone();
    apply(&mut layout, &id, Edit::Width(650.0)).unwrap();
    assert!(lbm_app::properties::layout_info(&layout, &id)
        .iter()
        .any(|(_, v)| v == "650.00 × 300.00"));
    assert_eq!(report, lbm_app::properties::report(&detected));
}

#[test]
fn undo_without_a_stored_profile_restores_size_placement_and_border_ownership() {
    let mut layout = layout(PER_MONITOR);
    let id = layout.monitors()[0].id.clone();
    let reference = lbm_app::saved::Reference::of(&layout);
    let original = size(&layout, &id).unwrap().physical;
    let positions: Vec<_> = layout.monitors().iter().map(|m| m.location()).collect();
    apply(&mut layout, &id, Edit::Width(800.0)).unwrap();
    apply(&mut layout, &id, Edit::Left(40.0)).unwrap();
    assert!(!reference.holds(&layout));
    assert!(layout.monitor(&id).unwrap().borders_customized());
    reference.restore(&mut layout);
    assert!(reference.holds(&layout));
    assert_eq!(size(&layout, &id).unwrap().physical, original);
    assert!(!layout.monitor(&id).unwrap().borders_customized());
    assert_eq!(
        layout
            .monitors()
            .iter()
            .map(|m| m.location())
            .collect::<Vec<_>>(),
        positions
    );
}

#[test]
fn a_size_across_the_detected_shape_is_accepted_and_survives_the_save_document() {
    // Landscape 600 x 300 as detected, aspect lock off: 250 wide makes it portrait. The
    // store's pre-5.4.1 migration used to read that back transposed on every load, and
    // this editor refused it; the store now marks what it saves intrinsic and applies
    // it as stored (#728, #730), whatever the rotation it was entered under.
    for rotation in 0..4 {
        for (narrow, tall) in [
            (Edit::Width(250.0), Edit::Height(700.0)),
            (Edit::Width(300.0), Edit::Width(200.0)),
        ] {
            let mut edited = layout(PER_MODEL);
            let id = edited.monitors()[0].id.clone();
            let source = edited.monitors()[0].active_source.clone().unwrap();
            edited.set_source_orientation(&source, rotation);
            let swap = |e: Edit| match (e, rotation % 2) {
                (Edit::Width(v), 1) => Edit::Height(v),
                (Edit::Height(v), 1) => Edit::Width(v),
                (e, _) => e,
            };
            apply(&mut edited, &id, swap(narrow)).unwrap();
            apply(&mut edited, &id, swap(tall)).unwrap();
            let shown = size(&edited, &id).unwrap().physical;

            let (_, request) = lbm_app::settings::save_layout(&edited);
            let document: lbm_store::LayoutDocument =
                serde_json::from_value(request["Document"].clone()).unwrap();
            let mut landed = layout(PER_MODEL);
            let source = landed.monitors()[0].active_source.clone().unwrap();
            landed.set_source_orientation(&source, rotation);
            document.apply(&mut landed);
            assert_eq!(
                size(&landed, &id).unwrap().physical,
                shown,
                "rotation {rotation}, {narrow:?} then {tall:?}"
            );
        }
    }
}

#[test]
fn position_and_scale_edits_share_the_document_and_preserve_other_axes() {
    let mut layout = layout(PER_MONITOR);
    let id = layout.monitors()[0].id.clone();
    let initial = lbm_app::properties::location(&layout, &id).unwrap();
    apply(&mut layout, &id, Edit::X(-123.45)).unwrap();
    let p = lbm_app::properties::location(&layout, &id).unwrap();
    assert_eq!((p.x, p.y), (-123.45, initial.y));
    apply(&mut layout, &id, Edit::Y(67.8)).unwrap();
    apply(&mut layout, &id, Edit::ScaleX(125.)).unwrap();
    apply(&mut layout, &id, Edit::ScaleY(90.)).unwrap();
    let p = lbm_app::properties::location(&layout, &id).unwrap();
    assert_eq!((p.x, p.y, p.scale_x, p.scale_y), (-123.45, 67.8, 125., 90.));
    let (_, request) = lbm_app::settings::save_layout(&layout);
    let document: lbm_store::LayoutDocument =
        serde_json::from_value(request["Document"].clone()).unwrap();
    let mut restored = self::layout(PER_MONITOR);
    document.apply(&mut restored);
    assert!(lbm_app::saved::Reference::of(&layout).holds(&restored));
    let reference = lbm_app::saved::Reference::of(&layout);
    for edit in [
        Edit::X(f64::NAN),
        Edit::Y(f64::INFINITY),
        Edit::ScaleX(0.),
        Edit::ScaleY(-5.),
    ] {
        assert!(apply(&mut layout, &id, edit).is_err());
        assert!(reference.holds(&layout));
    }
}
