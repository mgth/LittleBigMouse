//! Port of `LayoutPersistenceTests.cs`, one test per C# test, same names.
//!
//! Tests of the shared persistence engine over an in-memory store: everything here holds
//! identically for the registry backend (Windows) and the JSON backend (Linux), which is
//! the point of the abstraction.
//!
//! Saved flags: C# keeps one per reactive object, and an unsaved monitor, depth
//! projection or options object makes the layout unsaved. The Rust model keeps the
//! layout's flag and one per source, and its edit functions mark the layout directly.
//! The C# assertions on `monitor.Saved`, `monitor.DepthProjection.Saved` and
//! `layout.Options.Saved` have no Rust counterpart of their own: they fold into the
//! layout's flag, which is asserted wherever C# asserts it.

mod common;

use std::io;

use common::{
    add_monitor_with_source, design_options, read_lines, single, temp_excluded_file,
    test_persistence, write_lines, FakeStore, TestPlatform,
};
use indexmap::IndexMap;
use lbm_layout::geo::{Point, Rect, Size, Thickness};
use lbm_layout::model::{
    BorderSection, DisplaySize, DisplaySource, Layout, Monitor, MonitorModel, PhysicalSource, Ratio,
};
use lbm_store::excluded_process_defaults::{contains_entry, ALL, HEADER, LEGACY_V0, VERSION};
use lbm_store::layout_migrations::normalize_stored_size;
use lbm_store::{
    BorderResistanceDto, BorderSideDto, BordersDto, GlobalOptionsDto, LayoutDto, LayoutOptionsDto,
    LayoutPersistence, LayoutStore, LayoutStoreData, ModelDto, MonitorDto, SourceDto,
};
use tempfile::TempDir;

const LAYOUT_ID: &str = "TESTMON1";
const PNP: &str = "TST1234";
const MONITOR_ID: &str = "MON1";
const SOURCE_ID: &str = "SRC1";

/// C# `TempExcludedFile`'s directory, under the temp directory.
const TEMP_PREFIX: &str = "lbm-persistence-tests";

//==================//
// Fixtures         //
//==================//

/// C# `ThrowingStore`, the #589 store: the id-based operations fail, as the registry
/// did on a key name over 255 characters.
struct ThrowingStore;

/// C# `ThrowingStore.Message`, thrown as an `ArgumentException`.
fn argument_error() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidInput,
        "Registry key names should not be greater than 255 characters.",
    )
}

impl LayoutStore for ThrowingStore {
    fn read(&self, _layout_id: &str, _pnp_codes: &[&str]) -> io::Result<LayoutStoreData> {
        Err(argument_error())
    }

    fn write_global_options(&self, _options: &GlobalOptionsDto) -> io::Result<()> {
        Ok(())
    }

    fn write_layout(&self, _layout_id: &str, _layout: &LayoutDto) -> io::Result<()> {
        Err(argument_error())
    }

    fn write_models(&self, _models: &IndexMap<String, ModelDto>) -> io::Result<()> {
        Ok(())
    }
}

/// C# `NewPersistence`: the engine over `store`, its excluded list a fresh file whose
/// directory lives as long as the returned guard.
fn new_persistence(store: &FakeStore) -> (TempDir, LayoutPersistence<FakeStore, TestPlatform>) {
    let (dir, excluded) = temp_excluded_file(TEMP_PREFIX);
    (dir, test_persistence(store.clone(), &excluded))
}

/// C# `NewLayout`: one monitor of model TST1234 — 600x340, bezels 10/11/12/13 — with
/// one attached 1920x1080 source.
fn new_layout() -> Layout {
    let mut layout = Layout::new(design_options());
    layout.id = LAYOUT_ID.to_owned();

    let mut model = MonitorModel::new(PNP);
    let size = &mut model.physical_size;
    size.set_width(600.0);
    size.set_height(340.0);
    size.set_left_border(10.0);
    size.set_top_border(11.0);
    size.set_right_border(12.0);
    size.set_bottom_border(13.0);

    let monitor = Monitor::new(MONITOR_ID, &model);
    let mut source = DisplaySource::new(SOURCE_ID);
    source.attached_to_desktop = true;
    source.in_pixel = DisplaySize::from_rect(Rect::from_location_size(
        Point::new(0.0, 0.0),
        Size::new(1920.0, 1080.0),
    ));

    add_monitor_with_source(&mut layout, model, monitor, source, "DEV1");
    layout
}

/// C# `monitor` (`NewLayout`'s first out parameter).
fn monitor(layout: &Layout) -> &Monitor {
    layout.monitor(MONITOR_ID).unwrap()
}

/// C# `monitor.Model`.
fn model(layout: &Layout) -> &MonitorModel {
    layout.model(&monitor(layout).model).unwrap()
}

/// C# `monitor.DepthProjection`.
fn depth_projection(layout: &Layout) -> DisplaySize {
    layout.depth_projection(monitor(layout)).unwrap()
}

/// C# `source` (`NewLayout`'s second out parameter), with the saved flag the Rust model
/// keeps per source.
fn source(layout: &Layout) -> &PhysicalSource {
    layout.source(SOURCE_ID).unwrap()
}

/// C# `source.AttachedToDesktop = false`: a plain notification nothing in the domain
/// subscribes to. The Rust model has no edit function for it, so the source is replaced
/// in place by the same one detached, which has no side effect either.
fn detach(layout: &mut Layout) {
    let mut detached = source(layout).clone();
    detached.source.attached_to_desktop = false;
    layout.attach_source(detached);
}

/// `ExcludedDefaultsVersion` of the options the store holds.
fn stored_version(store: &FakeStore) -> Option<i32> {
    store
        .global_options
        .borrow()
        .as_ref()
        .and_then(|o| o.excluded_defaults_version)
}

/// C# `NewDefaults`: current defaults that are NOT legacy ones (the entries a top-up
/// would add).
fn new_defaults() -> Vec<&'static str> {
    ALL.iter()
        .copied()
        .filter(|entry| !contains_entry(LEGACY_V0, entry))
        .collect()
}

/// The store's layout document holding `monitor` alone.
fn layout_with_monitor(monitor: MonitorDto) -> LayoutDto {
    LayoutDto {
        monitors: IndexMap::from([(MONITOR_ID.to_owned(), monitor)]),
        ..LayoutDto::default()
    }
}

//==================//
// Tests            //
//==================//

/// C# `SaveThenLoad_RoundTripsMonitorGeometry`.
#[test]
fn save_then_load_round_trips_monitor_geometry() {
    let store = FakeStore::default();
    let (_dir, mut persistence) = new_persistence(&store);

    let mut layout = new_layout();
    // C#: DepthProjection.X then .Y, DepthRatio.X then .Y.
    layout.set_location(MONITOR_ID, Point::new(123.5, -42.25));
    layout.set_depth_ratio(MONITOR_ID, Ratio::new(1.25, 1.5));
    layout.edit_border_resistance(MONITOR_ID, |br| {
        br.right
            .sections
            .push(BorderSection::new(10.0, 60.0, 5.0, true, 6.0, false));
    });

    assert!(persistence.save(&mut layout).unwrap());

    let mut restored = new_layout();
    persistence.load(&mut restored).unwrap();

    let restored_monitor = monitor(&restored);
    assert_eq!(depth_projection(&restored).x, 123.5);
    assert_eq!(depth_projection(&restored).y, -42.25);
    assert_eq!(restored_monitor.depth_ratio.x, 1.25);
    assert_eq!(restored_monitor.depth_ratio.y, 1.5);
    assert!(restored_monitor.border_resistance.left.sections.is_empty());

    let section = single(&restored_monitor.border_resistance.right.sections);
    assert_eq!(section.from(), 10.0);
    assert_eq!(section.to(), 60.0);
    assert_eq!(section.move_resistance(), 5.0);
    assert!(section.move_block());
    assert_eq!(section.drag(), 6.0);
    assert!(!section.drag_block());
    assert!(restored_monitor.placed);
    // restoredMonitor.Saved folds into the layout's flag (see the module doc).
    assert!(restored.saved());
}

/// C# `Load_StoreReadFailure_DoesNotThrowAndLoadsAsAFirstRun`.
#[test]
fn load_store_read_failure_does_not_throw_and_loads_as_a_first_run() {
    // #589: nine monitors made the layout id longer than a registry key name may be,
    // the read threw through Load, and the UI never booted. Whatever the store does,
    // Load has to come back with a usable layout, marked saved as on a first run.
    let (_dir, excluded) = temp_excluded_file(TEMP_PREFIX);
    let mut persistence = test_persistence(ThrowingStore, &excluded);
    let mut layout = new_layout();

    persistence
        .load(&mut layout)
        .expect("a store that cannot be read does not fail the load");

    assert!(layout.saved());
    // monitor.Saved folds into the layout's flag (see the module doc).
    assert!(!persistence.is_loading());
}

/// C# `Load_ConvertsAStoredPerEdgeResistanceIntoAWholeEdgeSection`.
#[test]
fn load_converts_a_stored_per_edge_resistance_into_a_whole_edge_section() {
    // Every layout saved before the section editor holds one resistance per edge.
    // The notion is gone, so the value has to arrive as the section that says the
    // same thing — otherwise upgrading would silently unblock people's borders.
    let store = FakeStore::default();
    store.layouts.borrow_mut().insert(
        LAYOUT_ID.to_owned(),
        layout_with_monitor(MonitorDto {
            border_resistance: Some(BorderResistanceDto {
                left: Some(BorderSideDto {
                    r#move: Some(20.0),
                    drag: Some(20.0),
                    ..BorderSideDto::default()
                }),
                top: Some(BorderSideDto {
                    r#move: Some(0.0),
                    drag: Some(0.0),
                    ..BorderSideDto::default()
                }),
                ..BorderResistanceDto::default()
            }),
            ..MonitorDto::default()
        }),
    );

    let mut layout = new_layout();
    let (_dir, mut persistence) = new_persistence(&store);
    persistence.load(&mut layout).unwrap();

    let m = monitor(&layout);
    let section = single(&m.border_resistance.left.sections);
    assert_eq!(section.from(), 0.0);
    // The left edge spans the monitor's height.
    assert_eq!(section.to(), depth_projection(&layout).height);
    assert_eq!(section.move_resistance(), 20.0);
    assert_eq!(section.drag(), 20.0);

    // A zero edge is what "no resistance" always looked like: converting it would
    // litter every layout with meaningless full-edge sections.
    assert!(m.border_resistance.top.sections.is_empty());
}

/// C# `Load_EmptyStore_LeavesDefaultsAndMarksSaved`.
#[test]
fn load_empty_store_leaves_defaults_and_marks_saved() {
    let (_dir, mut persistence) = new_persistence(&FakeStore::default());

    let mut layout = new_layout();
    persistence.load(&mut layout).unwrap();

    // Nothing stored: not placed (that flag means "the user placed it"), but the whole
    // subtree must still be flagged saved so the next edit is observable.
    let m = monitor(&layout);
    assert!(!m.placed);
    assert!(!m.borders_customized());
    // monitor.Saved, monitor.DepthProjection.Saved and layout.Options.Saved fold into
    // the layout's flag (see the module doc).
    assert!(source(&layout).saved);
    assert!(layout.saved());
}

/// C# `Borders_StoredOnlyWhenCustomized_AndRestoreTheFlag`.
#[test]
fn borders_stored_only_when_customized_and_restore_the_flag() {
    let store = FakeStore::default();
    let (_dir, mut persistence) = new_persistence(&store);

    let mut layout = new_layout();
    persistence.save(&mut layout).unwrap();

    // Uncustomized: the monitor mirrors its model, nothing per-monitor is stored.
    assert!(store.layouts.borrow()[LAYOUT_ID].monitors[MONITOR_ID]
        .borders
        .is_none());

    layout.set_borders_customized(MONITOR_ID, true);
    // C#: Borders.Left, .Top, .Right, .Bottom, one after the other.
    layout.set_monitor_borders(MONITOR_ID, Thickness::new(5.0, 6.0, 7.0, 8.0));
    persistence.save(&mut layout).unwrap();

    assert!(store.layouts.borrow()[LAYOUT_ID].monitors[MONITOR_ID]
        .borders
        .is_some());

    let mut restored = new_layout();
    persistence.load(&mut restored).unwrap();

    let restored_monitor = monitor(&restored);
    assert!(restored_monitor.borders_customized());
    assert_eq!(restored_monitor.borders().left, 5.0);
    assert_eq!(restored_monitor.borders().bottom, 8.0);
}

/// C# `Load_StoredNonPositiveModelSize_DoesNotOverrideComputedSize`.
#[test]
fn load_stored_non_positive_model_size_does_not_override_computed_size() {
    let store = FakeStore::default();
    store.models.borrow_mut().insert(
        PNP.to_owned(),
        ModelDto {
            width: Some(0.0),
            height: Some(0.0),
            pnp_name: Some("Stored name".to_owned()),
            ..ModelDto::default()
        },
    );
    let (_dir, mut persistence) = new_persistence(&store);

    let mut layout = new_layout();
    persistence.load(&mut layout).unwrap();

    assert_eq!(model(&layout).physical_size.width(), 600.0);
    assert_eq!(model(&layout).physical_size.height(), 340.0);
    assert_eq!(
        model(&layout).pnp_device_name.as_deref(),
        Some("Stored name")
    );
}

/// C# `Load_StoredModel_AppliesSizeAndBorders`.
#[test]
fn load_stored_model_applies_size_and_borders() {
    let store = FakeStore::default();
    store.models.borrow_mut().insert(
        PNP.to_owned(),
        ModelDto {
            width: Some(700.0),
            height: Some(400.0),
            borders: Some(BordersDto {
                left: Some(20.0),
                top: Some(21.0),
                right: Some(22.0),
                bottom: Some(23.0),
                ..BordersDto::default()
            }),
            ..ModelDto::default()
        },
    );
    let (_dir, mut persistence) = new_persistence(&store);

    let mut layout = new_layout();
    persistence.load(&mut layout).unwrap();

    let size = &model(&layout).physical_size;
    assert_eq!(size.width(), 700.0);
    assert_eq!(size.height(), 400.0);
    assert_eq!(size.borders().left, 20.0);
    assert_eq!(size.borders().bottom, 23.0);
}

/// C# `SaveEnabled_TogglesEnabledAndPreservesTheRest`.
#[test]
fn save_enabled_toggles_enabled_and_preserves_the_rest() {
    let store = FakeStore::default();
    let (_dir, persistence) = new_persistence(&store);

    let mut layout = new_layout();
    layout.edit_options(|o| {
        o.algorithm = "Cross".to_owned();
        o.enabled = true;
    });
    persistence.save(&mut layout).unwrap();

    layout.edit_options(|o| o.enabled = false);
    assert!(persistence.save_enabled(&layout).unwrap());

    let layouts = store.layouts.borrow();
    let dto = &layouts[LAYOUT_ID];
    let options = dto.options.as_ref().unwrap();
    assert_eq!(options.enabled, Some(false));
    assert_eq!(options.algorithm.as_deref(), Some("Cross"));
    assert!(dto.monitors.contains_key(MONITOR_ID));
}

/// C# `Load_LayoutPriority_OverridesGlobalPriority`.
#[test]
fn load_layout_priority_overrides_global_priority() {
    let store = FakeStore::default();
    *store.global_options.borrow_mut() = Some(GlobalOptionsDto {
        priority: Some("High".to_owned()),
        priority_unhooked: Some("Idle".to_owned()),
        ..GlobalOptionsDto::default()
    });
    store.layouts.borrow_mut().insert(
        LAYOUT_ID.to_owned(),
        LayoutDto {
            options: Some(LayoutOptionsDto {
                priority: Some("Realtime".to_owned()),
                ..LayoutOptionsDto::default()
            }),
            ..LayoutDto::default()
        },
    );
    let (_dir, mut persistence) = new_persistence(&store);

    let mut layout = new_layout();
    persistence.load(&mut layout).unwrap();

    assert_eq!(layout.options.priority, "Realtime");
    // Not overridden per-layout: the global value applies.
    assert_eq!(layout.options.priority_unhooked, "Idle");
}

/// C# `Load_FirstRun_SeedsExcludedDefaultsFile`.
#[test]
fn load_first_run_seeds_excluded_defaults_file() {
    let store = FakeStore::default();
    let (_dir, excluded) = temp_excluded_file(TEMP_PREFIX);
    let mut persistence = test_persistence(store, &excluded);

    let mut layout = new_layout();
    persistence.load(&mut layout).unwrap();

    // The header is a comment for the daemon, so it is not an entry of the list — but
    // the seeded file carries it, exactly like the one CreateExcludedFile writes.
    assert_eq!(layout.options.excluded_list, ALL);
    let expected_file: Vec<&str> = [HEADER].into_iter().chain(ALL.iter().copied()).collect();
    assert_eq!(read_lines(&excluded), expected_file);
}

/// C# `ExcludedList_CommentsStayOutOfTheListAndSurviveASave`.
#[test]
fn excluded_list_comments_stay_out_of_the_list_and_survive_a_save() {
    let store = FakeStore::default();
    // Current version: the top-up must not interfere with what is asserted here.
    *store.global_options.borrow_mut() = Some(GlobalOptionsDto {
        excluded_defaults_version: Some(VERSION),
        ..GlobalOptionsDto::default()
    });
    let (_dir, excluded) = temp_excluded_file(TEMP_PREFIX);
    write_lines(&excluded, &[HEADER, r"\steamapps\", "", ":my own note"]);
    let mut persistence = test_persistence(store, &excluded);

    let mut layout = new_layout();
    persistence.load(&mut layout).unwrap();

    // Only the exclusions reach the model: the daemon skips ':' lines and empty ones,
    // and the options list is what the user edits — it must hold the same thing.
    assert_eq!(layout.options.excluded_list, [r"\steamapps\"]);

    layout.edit_options(|o| o.excluded_list.push(r"\my\game\".to_owned()));
    persistence.save_live(&layout.options).unwrap();

    // The comments are still on disk, above the entries: they were never the list's
    // to drop, and one of them may be the user's own annotation.
    assert_eq!(
        read_lines(&excluded),
        [HEADER, ":my own note", r"\steamapps\", r"\my\game\"]
    );
}

/// C# `Load_ExcludedDefaults_ToppedUpOnce`.
#[test]
fn load_excluded_defaults_topped_up_once() {
    let store = FakeStore::default();
    let (_dir, excluded) = temp_excluded_file(TEMP_PREFIX);
    write_lines(&excluded, LEGACY_V0);
    let mut persistence = test_persistence(store.clone(), &excluded);

    let mut layout = new_layout();
    persistence.load(&mut layout).unwrap();

    // The list kept all previous defaults: new defaults are added, file included,
    // and the applied version is recorded in the store. Separator-insensitive: on
    // Linux the current defaults are slash-style while LegacyV0 is Windows-style.
    let file = read_lines(&excluded);
    for entry in ALL {
        assert!(
            contains_entry(&layout.options.excluded_list, entry),
            "missing default: {entry}"
        );
        assert!(
            contains_entry(&file, entry),
            "missing default in file: {entry}"
        );
    }
    assert_eq!(stored_version(&store), Some(VERSION));
}

/// C# `Load_ExcludedDefaults_CustomizedListLeftUntouched`.
#[test]
fn load_excluded_defaults_customized_list_left_untouched() {
    let store = FakeStore::default();
    let (_dir, excluded) = temp_excluded_file(TEMP_PREFIX);
    // A legacy default was removed by the user: no top-up, but the version is
    // recorded so the migration never runs again.
    write_lines(&excluded, &[r"\steamapps\", r"\my\own\path\"]);
    let mut persistence = test_persistence(store.clone(), &excluded);

    let mut layout = new_layout();
    persistence.load(&mut layout).unwrap();

    for entry in new_defaults() {
        assert!(
            !contains_entry(&layout.options.excluded_list, entry),
            "top-up ran on a customized list: {entry}"
        );
    }
    assert_eq!(stored_version(&store), Some(VERSION));
}

/// C# `Load_ExcludedDefaults_AlreadyApplied_DoesNothing`.
#[test]
fn load_excluded_defaults_already_applied_does_nothing() {
    let store = FakeStore::default();
    *store.global_options.borrow_mut() = Some(GlobalOptionsDto {
        excluded_defaults_version: Some(VERSION),
        ..GlobalOptionsDto::default()
    });
    let (_dir, excluded) = temp_excluded_file(TEMP_PREFIX);
    write_lines(&excluded, LEGACY_V0);
    let mut persistence = test_persistence(store, &excluded);

    let mut layout = new_layout();
    persistence.load(&mut layout).unwrap();

    for entry in new_defaults() {
        assert!(
            !contains_entry(&layout.options.excluded_list, entry),
            "top-up ran twice: {entry}"
        );
    }
}

/// C# `Load_DetachedSource_RestoresStoredGeometry`.
#[test]
fn load_detached_source_restores_stored_geometry() {
    let store = FakeStore::default();
    store.layouts.borrow_mut().insert(
        LAYOUT_ID.to_owned(),
        layout_with_monitor(MonitorDto {
            active_source: Some(SOURCE_ID.to_owned()),
            sources: Some(IndexMap::from([(
                SOURCE_ID.to_owned(),
                SourceDto {
                    pixel_x: Some(100.0),
                    pixel_y: Some(200.0),
                    pixel_width: Some(800.0),
                    pixel_height: Some(600.0),
                    orientation: Some(1),
                    ..SourceDto::default()
                },
            )])),
            ..MonitorDto::default()
        }),
    );
    let (_dir, mut persistence) = new_persistence(&store);

    let mut layout = new_layout();
    detach(&mut layout);
    persistence.load(&mut layout).unwrap();

    let s = &source(&layout).source;
    assert_eq!(s.in_pixel.x, 100.0);
    assert_eq!(s.in_pixel.y, 200.0);
    assert_eq!(s.in_pixel.width, 800.0);
    assert_eq!(s.in_pixel.height, 600.0);
    assert_eq!(s.orientation, 1);
}

/// C# `Load_AttachedSource_KeepsLiveGeometry`.
#[test]
fn load_attached_source_keeps_live_geometry() {
    let store = FakeStore::default();
    store.layouts.borrow_mut().insert(
        LAYOUT_ID.to_owned(),
        layout_with_monitor(MonitorDto {
            sources: Some(IndexMap::from([(
                SOURCE_ID.to_owned(),
                SourceDto {
                    pixel_x: Some(100.0),
                    pixel_y: Some(200.0),
                    pixel_width: Some(800.0),
                    pixel_height: Some(600.0),
                    ..SourceDto::default()
                },
            )])),
            ..MonitorDto::default()
        }),
    );
    let (_dir, mut persistence) = new_persistence(&store);

    let mut layout = new_layout();
    persistence.load(&mut layout).unwrap();

    // Attached: the live geometry wins, the store is just a backup for re-attach.
    let s = &source(&layout).source;
    assert_eq!(s.in_pixel.x, 0.0);
    assert_eq!(s.in_pixel.width, 1920.0);
}

/// C# `Save_StoresAttachedSourcesOnly`.
#[test]
fn save_stores_attached_sources_only() {
    let store = FakeStore::default();
    let (_dir, persistence) = new_persistence(&store);

    let mut layout = new_layout();
    detach(&mut layout);
    persistence.save(&mut layout).unwrap();

    assert!(store.layouts.borrow()[LAYOUT_ID].monitors[MONITOR_ID]
        .sources
        .as_ref()
        .expect("the sources are written, even none")
        .is_empty());
}

/// C# `SaveLive_WritesGlobalOptionsAndExcludedFile`.
#[test]
fn save_live_writes_global_options_and_excluded_file() {
    let store = FakeStore::default();
    let (_dir, excluded) = temp_excluded_file(TEMP_PREFIX);
    let mut persistence = test_persistence(store.clone(), &excluded);

    let mut layout = new_layout();
    persistence.load(&mut layout).unwrap();

    layout.edit_options(|o| {
        o.hide_tray_icon = true;
        o.excluded_list.push(r"\my\game\".to_owned());
    });
    persistence.save_live(&layout.options).unwrap();

    let global = store.global_options.borrow();
    assert_eq!(global.as_ref().and_then(|o| o.hide_tray_icon), Some(true));
    // The version survives a live save even though it is not part of the options model.
    assert_eq!(stored_version(&store), Some(VERSION));
    assert!(read_lines(&excluded)
        .iter()
        .any(|line| line == r"\my\game\"));
}

/// C# `ExperimentalFeatures_SurviveARestart`.
#[test]
fn experimental_features_survive_a_restart() {
    let store = FakeStore::default();

    let mut layout = new_layout();
    layout.edit_options(|o| o.experimental_features = true);
    let (_first_dir, first) = new_persistence(&store);
    first.save_live(&layout.options).unwrap();

    assert_eq!(
        store
            .global_options
            .borrow()
            .as_ref()
            .and_then(|o| o.experimental_features),
        Some(true)
    );

    let mut restored = new_layout();
    let (_dir, mut persistence) = new_persistence(&store);
    persistence.load(&mut restored).unwrap();

    assert!(restored.options.experimental_features);
}

/// C# `ExperimentalFeatures_AbsentFromTheStore_KeepsTheDefault`.
#[test]
fn experimental_features_absent_from_the_store_keeps_the_default() {
    // Every installation predating this option: the value is not there, and off is
    // what it has always been at start.
    let store = FakeStore::default();
    *store.global_options.borrow_mut() = Some(GlobalOptionsDto {
        pinned: Some(true),
        ..GlobalOptionsDto::default()
    });

    let mut layout = new_layout();
    let (_dir, mut persistence) = new_persistence(&store);
    persistence.load(&mut layout).unwrap();

    assert!(!layout.options.experimental_features);
}

/// C# `Load_TransposesPre541OrientedStoredSize`.
#[test]
fn load_transposes_pre541_oriented_stored_size() {
    // Pre-5.4.1 the model persisted the size ORIENTED to the rotation at save time:
    // a monitor portrait at save time stored a transposed size. Read as intrinsic
    // (5.4.1 semantics) it gets the rotation applied twice — the #507 follow-up where
    // the previously-portrait monitor renders inverted after the upgrade. The stored
    // value must be transposed back, keeping its (possibly user-customized) magnitudes.
    let store = FakeStore::default();
    store.models.borrow_mut().insert(
        PNP.to_owned(),
        ModelDto {
            width: Some(338.0),
            height: Some(598.0),
            ..ModelDto::default()
        },
    );

    let mut layout = new_layout(); // fresh intrinsic model: 600x340
    let (_dir, mut persistence) = new_persistence(&store);
    persistence.load(&mut layout).unwrap();

    assert_eq!(model(&layout).physical_size.width(), 598.0);
    assert_eq!(model(&layout).physical_size.height(), 338.0);
}

/// C# `Load_KeepsStoredSizeWhenOrientationMatches`.
#[test]
fn load_keeps_stored_size_when_orientation_matches() {
    let store = FakeStore::default();
    store.models.borrow_mut().insert(
        PNP.to_owned(),
        ModelDto {
            width: Some(598.0),
            height: Some(338.0),
            ..ModelDto::default()
        },
    );

    let mut layout = new_layout(); // fresh intrinsic model: 600x340
    let (_dir, mut persistence) = new_persistence(&store);
    persistence.load(&mut layout).unwrap();

    assert_eq!(model(&layout).physical_size.width(), 598.0);
    assert_eq!(model(&layout).physical_size.height(), 338.0);
}

/// C# `NormalizeStoredSize_Cases`, a theory: its five `InlineData` in order.
#[test]
fn normalize_stored_size_cases() {
    // (intrinsicW, intrinsicH, storedW, storedH, expectedW, expectedH)
    let cases: [(f64, f64, f64, f64, f64, f64); 5] = [
        // Square intrinsic or stored size: orientation is undecidable, keep as stored.
        (500.0, 500.0, 340.0, 600.0, 340.0, 600.0),
        (600.0, 340.0, 500.0, 500.0, 500.0, 500.0),
        // Invalid intrinsic reference (EDID-less placeholder): keep as stored.
        (0.0, 0.0, 340.0, 600.0, 340.0, 600.0),
        // Contradicting orientation: transpose, magnitudes preserved.
        (600.0, 340.0, 340.0, 600.0, 600.0, 340.0),
        // Intrinsically-portrait panel, stored landscape (saved while rotated): transpose too.
        (340.0, 600.0, 600.0, 340.0, 340.0, 600.0),
    ];
    for (intrinsic_w, intrinsic_h, stored_w, stored_h, expected_w, expected_h) in cases {
        let (w, h) = normalize_stored_size(intrinsic_w, intrinsic_h, stored_w, stored_h);
        let case = format!("intrinsic {intrinsic_w}x{intrinsic_h}, stored {stored_w}x{stored_h}");
        assert_eq!(w, expected_w, "{case}");
        assert_eq!(h, expected_h, "{case}");
    }
}
