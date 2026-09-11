//! The model's contracts, ported from the C# tests `LayoutIdTests`,
//! `PrimaryMonitorContractTests` and `LinuxLayoutMappingTests`.

mod common;

use common::design_options;
use lbm_layout::geo::Rect;
use lbm_layout::linux::{add_monitor, LinuxEdid, LinuxMonitor};
use lbm_layout::model::{
    DisplaySize, DisplaySource, Layout, Monitor, MonitorModel, PhysicalSource,
};

/// A 600 x 340 mm monitor with one 1920 x 1080 source, built the way the C#
/// tests build theirs: the monitor handed to the layout, then its source.
fn add(layout: &mut Layout, id: &str, pnp: &str, primary: bool, orientation: i32, register: bool) {
    layout.get_or_add_model(pnp, |code| {
        let mut m = MonitorModel::new(code);
        m.physical_size.set_width(600.0);
        m.physical_size.set_height(340.0);
        m
    });
    let model = layout.model(pnp).unwrap().clone();
    let mut monitor = Monitor::new(id, &model);
    let mut source = DisplaySource::new(format!("{id}-source"));
    source.attached_to_desktop = true;
    source.primary = primary;
    source.orientation = orientation;
    source.in_pixel = DisplaySize::from_rect(Rect::new(0.0, 0.0, 1920.0, 1080.0));
    let physical = PhysicalSource::new(format!("{id}-device"), id, source);
    monitor.active_source = Some(format!("{id}-source"));
    monitor.sources.push(format!("{id}-source"));
    layout.attach_source(physical.clone());
    layout.add_or_update_monitor(monitor);
    if register {
        layout.add_or_update_source(physical);
    }
}

/// `new MonitorsLayout(new ILayoutOptions.Design())`.
fn layout() -> Layout {
    Layout::new(design_options())
}

// LayoutIdTests

/// C#: `LayoutIdTests.ComputeId_Landscape_KeepsLegacyKey`.
#[test]
fn compute_id_landscape_keeps_legacy_key() {
    let mut l = layout();
    add(&mut l, "MONB", "TST1234", false, 0, true);
    add(&mut l, "MONA", "TST1234", false, 0, true);
    assert_eq!(l.compute_id(), "MONA+MONB");
}

/// C#: `LayoutIdTests.ComputeId_RotatedMonitor_GetsOrientationSuffix`.
#[test]
fn compute_id_rotated_monitor_gets_orientation_suffix() {
    let mut l = layout();
    add(&mut l, "MONA", "TST1234", false, 1, true);
    add(&mut l, "MONB", "TST1234", false, 0, true);
    assert_eq!(l.compute_id(), "MONA_1+MONB");
}

/// C#: `LayoutIdTests.ComputeId_ChangesWhenRotationChanges`.
#[test]
fn compute_id_changes_when_rotation_changes() {
    let mut l = layout();
    add(&mut l, "MONA", "TST1234", false, 0, true);
    add(&mut l, "MONB", "TST1234", false, 0, true);
    let before = l.compute_id();
    l.set_source_orientation("MONA-source", 1);
    assert_ne!(before, l.compute_id());
}

// PrimaryMonitorContractTests

/// C#: `PrimaryMonitorContractTests.EmptyLayoutHasNoPrimary`.
#[test]
fn empty_layout_has_no_primary() {
    let l = layout();
    assert!(l.monitors().is_empty());
    assert!(l.primary_monitor().is_none());
    assert!(l.primary_source().is_none());
    // `ComputePixelLocationsFromPhysical()`, `adjustScale` defaulting to false.
    assert!(l.compute_pixel_locations_from_physical(false).is_empty());
}

/// C#: `PrimaryMonitorContractTests.PartiallyBuiltLayoutHasNoPrimaryAndPlacementIsIgnored`.
#[test]
fn partially_built_layout_has_no_primary_and_placement_is_ignored() {
    let mut l = layout();
    add(&mut l, "PENDING", "PNP_PENDING", true, 0, false);
    let bounds = |l: &Layout| l.depth_projection(&l.monitors()[0]).unwrap().bounds();
    let before = bounds(&l);

    assert_eq!(l.monitors().len(), 1);
    assert_eq!(l.sources().count(), 0);
    assert!(l.primary_monitor().is_none());
    assert!(l.primary_source().is_none());

    l.anchor_on_primary();
    // `SetLocationsFromSystemConfiguration()`: `placeAll` defaults to true.
    l.set_locations_from_system_configuration(true);
    l.force_compact();

    assert_eq!(before, bounds(&l));
}

/// C#: `PrimaryMonitorContractTests.DesignatingPrimaryUpdatesBothProperties`.
#[test]
fn designating_primary_updates_both_properties() {
    let mut l = layout();
    add(&mut l, "MONITOR", "PNP_MONITOR", false, 0, true);
    l.set_source_primary("MONITOR-source", true);
    assert_eq!(l.primary_monitor().unwrap().id, "MONITOR");
    assert_eq!(l.primary_source().unwrap().source.id, "MONITOR-source");
}

/// C#: `PrimaryMonitorContractTests.ReplacingPrimaryAllowsTheTransientStateAndSelectsTheReplacement`.
#[test]
fn replacing_primary_allows_the_transient_state_and_selects_the_replacement() {
    let mut l = layout();
    add(&mut l, "OLD", "PNP_OLD", true, 0, true);
    add(&mut l, "NEW", "PNP_NEW", false, 0, true);
    assert_eq!(l.primary_monitor().unwrap().id, "OLD");
    assert_eq!(l.primary_source().unwrap().source.id, "OLD-source");
    l.set_source_primary("OLD-source", false);
    assert!(l.primary_monitor().is_none());
    assert!(l.primary_source().is_none());
    l.set_source_primary("NEW-source", true);
    assert_eq!(l.primary_monitor().unwrap().id, "NEW");
    assert_eq!(l.primary_source().unwrap().source.id, "NEW-source");
}

// LinuxLayoutMappingTests

fn portrait(edid: Option<LinuxEdid>) -> LinuxMonitor {
    // Oriented values, as both sources report them for a left-rotated 27" QHD.
    LinuxMonitor {
        connector_name: "DP-1".to_owned(),
        logical_x: 0.0,
        logical_y: 0.0,
        logical_width: 1440.0,
        logical_height: 2560.0,
        pixel_width: 1440,
        pixel_height: 2560,
        scale: 1.0,
        width_mm: 336.0,
        height_mm: 597.0,
        primary: true,
        enabled: true,
        orientation: 1,
        frequency: 0,
        edid,
    }
}

fn single_sizes(l: &Layout) -> ((f64, f64), (f64, f64)) {
    assert_eq!(l.monitors().len(), 1);
    let m = &l.monitors()[0];
    let model = l.model(&m.model).unwrap();
    let rotated = l.physical_rotated(m).unwrap();
    (
        (model.physical_size.width(), model.physical_size.height()),
        (rotated.width, rotated.height),
    )
}

/// C#: `LinuxLayoutMappingTests.PortraitMonitorWithEdid_ModelKeepsIntrinsicSize_AndRotatedIsPortrait`.
#[test]
fn portrait_monitor_with_edid_model_keeps_intrinsic_size_and_rotated_is_portrait() {
    let mut l = layout();
    add_monitor(
        &mut l,
        &portrait(Some(LinuxEdid {
            manufacturer_code: Some("SAM".to_owned()),
            product_code: Some("1234".to_owned()),
            serial: Some("S1".to_owned()),
            physical_width: 597.0,
            physical_height: 336.0,
            ..LinuxEdid::default()
        })),
    );
    assert_eq!(single_sizes(&l), ((597.0, 336.0), (336.0, 597.0)));
}

/// C#: `LinuxLayoutMappingTests.PortraitMonitorWithoutEdid_OrientedSourceSizeIsUnrotated`.
#[test]
fn portrait_monitor_without_edid_oriented_source_size_is_unrotated() {
    let mut l = layout();
    add_monitor(&mut l, &portrait(None));
    assert_eq!(single_sizes(&l), ((597.0, 336.0), (336.0, 597.0)));
}

/// C#: `LinuxLayoutMappingTests.LandscapeMonitor_SizeIsUntouched`.
#[test]
fn landscape_monitor_size_is_untouched() {
    let mut l = layout();
    add_monitor(
        &mut l,
        &LinuxMonitor {
            connector_name: "DP-2".to_owned(),
            orientation: 0,
            pixel_width: 2560,
            pixel_height: 1440,
            logical_width: 2560.0,
            logical_height: 1440.0,
            width_mm: 597.0,
            height_mm: 336.0,
            ..portrait(None)
        },
    );
    assert_eq!(single_sizes(&l), ((597.0, 336.0), (597.0, 336.0)));
}
