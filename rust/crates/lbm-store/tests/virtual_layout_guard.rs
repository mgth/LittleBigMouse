//! Port of `VirtualLayoutGuardTests.cs`, one test per C# test, same names.
//!
//! A virtual (foreign) layout must never touch this machine's state: the persistence
//! layer refuses to store or load it, and the zones sent to the daemon carry the
//! Virtual flag the daemon keys its own hook refusal on. These guards are what keeps
//! a client's exported configuration from leaking into the local store, the autostart
//! scheduling or the crash-recovery file.

mod common;

use common::{
    add_monitor_with_source, design_options, temp_excluded_file, test_persistence, FakeStore,
    TestPlatform,
};
use lbm_layout::geo::{Point, Rect, Size};
use lbm_layout::model::{DisplaySize, DisplaySource, Layout, LayoutSource, Monitor, MonitorModel};
use lbm_layout::zoning::compute_zones;
use lbm_store::{LayoutDto, LayoutOptionsDto, LayoutPersistence};
use tempfile::TempDir;

const LAYOUT_ID: &str = "CLIENTMON1";

/// C# `NewPersistence`: the engine over `store`, its excluded list a fresh file whose
/// directory lives as long as the returned guard.
fn new_persistence(store: &FakeStore) -> (TempDir, LayoutPersistence<FakeStore, TestPlatform>) {
    let (dir, excluded) = temp_excluded_file("lbm-virtual-guard-tests");
    (dir, test_persistence(store.clone(), &excluded))
}

/// C# `NewLayout(source)`: one monitor of model TST1234, 600x340, with one attached
/// 1920x1080 source, in a layout coming from `source_kind`. C# also names a virtual
/// layout's origin (`SourceOrigin = "client-export.json"`), which only the refusal's
/// log line reads; the Rust layout has no such field.
fn new_layout(source_kind: LayoutSource) -> Layout {
    let mut layout = Layout::new(design_options());
    layout.id = LAYOUT_ID.to_owned();
    layout.source_kind = source_kind;

    let mut model = MonitorModel::new("TST1234");
    model.physical_size.set_width(600.0);
    model.physical_size.set_height(340.0);

    let monitor = Monitor::new("MON1", &model);
    let mut display_source = DisplaySource::new("SRC1");
    display_source.attached_to_desktop = true;
    display_source.in_pixel = DisplaySize::from_rect(Rect::from_location_size(
        Point::new(0.0, 0.0),
        Size::new(1920.0, 1080.0),
    ));

    add_monitor_with_source(&mut layout, model, monitor, display_source, "DEV1");
    layout
}

/// C# `Save_VirtualLayout_RefusedAndStoreUntouched`.
#[test]
fn save_virtual_layout_refused_and_store_untouched() {
    let store = FakeStore::default();
    let (_dir, persistence) = new_persistence(&store);

    let mut layout = new_layout(LayoutSource::VirtualImport);
    assert!(!persistence.save(&mut layout).unwrap());

    assert!(store.layouts.borrow().is_empty());
    assert!(store.models.borrow().is_empty());
    assert!(store.global_options.borrow().is_none());
}

/// C# `SaveEnabled_VirtualLayout_RefusedAndCreatesNoStoreEntry`.
#[test]
fn save_enabled_virtual_layout_refused_and_creates_no_store_entry() {
    // The regression that motivated the guard: SaveEnabled runs on every Stop and
    // used to CREATE a store entry named after the client's monitor combination.
    let store = FakeStore::default();
    let (_dir, persistence) = new_persistence(&store);

    let mut layout = new_layout(LayoutSource::VirtualFile);
    layout.edit_options(|o| o.enabled = false);

    assert!(!persistence.save_enabled(&layout).unwrap());
    assert!(store.layouts.borrow().is_empty());
}

/// C# `Load_VirtualLayout_RefusedAndKeepsImportedState`.
#[test]
fn load_virtual_layout_refused_and_keeps_imported_state() {
    // A LOCAL layout may share the client's id (same monitor models): loading would
    // apply that stored state over the imported one. The import stays authoritative.
    let store = FakeStore::default();
    store.layouts.borrow_mut().insert(
        LAYOUT_ID.to_owned(),
        LayoutDto {
            options: Some(LayoutOptionsDto {
                algorithm: Some("Cross".to_owned()),
                loop_x: Some(true),
                ..LayoutOptionsDto::default()
            }),
            ..LayoutDto::default()
        },
    );
    let (_dir, mut persistence) = new_persistence(&store);

    let mut layout = new_layout(LayoutSource::VirtualImport);
    persistence.load(&mut layout).unwrap();

    assert_eq!(layout.options.algorithm, "Strait");
    assert!(!layout.options.loop_x);
}

/// C# `ComputeZones_CarriesTheVirtualFlagOnTheWire`.
#[test]
fn compute_zones_carries_the_virtual_flag_on_the_wire() {
    let virtual_zones = compute_zones(&new_layout(LayoutSource::VirtualImport));
    assert!(virtual_zones.virtual_layout);
    assert!(virtual_zones.serialize().contains(r#"Virtual="True""#));

    let system_zones = compute_zones(&new_layout(LayoutSource::System));
    assert!(!system_zones.virtual_layout);
    assert!(system_zones.serialize().contains(r#"Virtual="False""#));
}

/// C# `SystemLayout_IsNotVirtual_AndStillSaves`.
#[test]
fn system_layout_is_not_virtual_and_still_saves() {
    let store = FakeStore::default();
    let (_dir, persistence) = new_persistence(&store);

    let mut layout = new_layout(LayoutSource::System);
    assert!(!layout.is_virtual());
    assert!(persistence.save(&mut layout).unwrap());
    assert!(store.layouts.borrow().contains_key(LAYOUT_ID));
}
