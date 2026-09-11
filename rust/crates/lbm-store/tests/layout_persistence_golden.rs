//! Port of `LayoutPersistenceGoldenTests.cs`, one test per C# test, same names.
//!
//! End-to-end golden tests of the persistence engine over REAL files: the fixtures under
//! `TestData/Persistence` (read from the source tree, see `common`) are configuration
//! directories as the supported versions wrote them, read through the actual Linux store
//! ([`JsonLayoutStore`]) and mapped by [`LayoutPersistence`].
//!
//! What they lock, which the in-memory tests cannot: the JSON property NAMES, the shapes
//! a stored document may take across versions, and the exact bytes a save produces. A
//! DTO rename, a changed default or a migration silently dropped all fail here, with the
//! fixture showing what a user's file actually looks like.
//!
//! Windows stores the same DTOs in the registry (`RegistryLayoutStore`), which is not
//! ported; the engine, the mapping and the migrations under test are the shared ones, so
//! only the storage encoding is left uncovered here.
//!
//! The expected save output (`*-saved`) is what the C# writer produces. The C# helper
//! rewrites it under `LBM_UPDATE_GOLDEN=1` after an intentional format change; this port
//! has no such switch — the Rust writer must reproduce the C# bytes, never regenerate
//! them.
//!
//! As in `layout_persistence.rs`, C#'s `monitor.Saved` folds into the layout's saved
//! flag, which is asserted instead.

mod common;

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use common::{
    add_monitor_with_source, design_options, read_lines, read_normalized, single, test_persistence,
    write_lines, TestPlatform,
};
use lbm_layout::geo::{Point, Rect, Size};
use lbm_layout::model::{
    DisplaySize, DisplaySource, Layout, Monitor, MonitorModel, PhysicalSource,
};
use lbm_store::excluded_process_defaults::{contains_entry, ALL, HEADER, LEGACY_V0, VERSION};
use lbm_store::{JsonLayoutStore, LayoutPersistence, LayoutStore};
use tempfile::TempDir;

// The fixtures are named after these: a layout id built from the monitor combination,
// one monitor of model TST1234 with one attached source.
const LAYOUT_ID: &str = "TST1234_1920x1080";
const PNP: &str = "TST1234";
const MONITOR_ID: &str = "TST1234_0";
const SOURCE_ID: &str = "DISPLAY1";

//==================//
// Fixtures         //
//==================//

/// C# `_work`: this test's scratch directory, deleted afterwards (C# `Dispose`).
struct Work(TempDir);

impl Work {
    fn new() -> Self {
        Work(
            tempfile::Builder::new()
                .prefix("lbm-golden-tests")
                .tempdir()
                .unwrap(),
        )
    }

    fn path(&self) -> &Path {
        self.0.path()
    }

    /// C# `Fixture`: a writable copy of a fixture — loading may migrate and saving
    /// certainly writes, and the committed fixture must stay the file the old version
    /// wrote.
    fn fixture(&self, name: &str) -> PathBuf {
        common::fixture_copy(name, self.path())
    }

    /// C# `NewPersistence`: a persistence whose excluded list is a fresh file outside
    /// the fixture.
    ///
    /// The C# never creates that file's directory, which goes unnoticed because
    /// `ExcludedListPersistence.Write` swallows the failure. The Rust engine reports it
    /// — a save then fails and leaves the layout unsaved, the one deliberate difference
    /// — so the directory is created here. Nothing these tests observe depends on it.
    fn new_persistence<S: LayoutStore>(&self, store: S) -> LayoutPersistence<S, TestPlatform> {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let dir = self
            .path()
            .join(format!("fresh-{}", NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::create_dir_all(&dir).unwrap();
        test_persistence(store, &dir.join("Excluded.txt"))
    }

    /// C# `Load`: the layout this machine would build, loaded from a copy of `fixture`.
    fn load(&self, fixture: &str) -> Layout {
        let mut layout = new_layout();
        self.new_persistence(JsonLayoutStore::new(self.fixture(fixture)))
            .load(&mut layout)
            .unwrap();
        layout
    }
}

/// C# `NewLayout`: the layout this machine would build for the fixtures — one landscape
/// monitor, intrinsic 600x340 with 10/11/12/13 bezels, one attached 1920x1080 source.
fn new_layout() -> Layout {
    let mut layout = Layout::new(design_options());
    layout.id = LAYOUT_ID.to_owned();

    let mut model = MonitorModel::new(PNP);
    model.pnp_device_name = Some("Live name".to_owned());
    let size = &mut model.physical_size;
    size.set_width(600.0);
    size.set_height(340.0);
    size.set_left_border(10.0);
    size.set_top_border(11.0);
    size.set_right_border(12.0);
    size.set_bottom_border(13.0);

    let mut monitor = Monitor::new(MONITOR_ID, &model);
    monitor.serial_number = Some("SN-0001".to_owned());

    let mut source = DisplaySource::new(SOURCE_ID);
    source.attached_to_desktop = true;
    source.display_name = Some(r"\\.\DISPLAY1".to_owned());
    source.primary = true;
    source.in_pixel = DisplaySize::from_rect(Rect::from_location_size(
        Point::new(0.0, 0.0),
        Size::new(1920.0, 1080.0),
    ));

    add_monitor_with_source(&mut layout, model, monitor, source, "DEV1");
    layout
}

/// C# `monitor` (`Load`'s second item).
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

/// C# `source` (`Load`'s third item).
fn source(layout: &Layout) -> &PhysicalSource {
    layout.source(SOURCE_ID).unwrap()
}

/// The layout document's path in a store directory.
fn layout_file(dir: &Path) -> PathBuf {
    dir.join("layouts").join(format!("{LAYOUT_ID}.json"))
}

/// C# `AssertGolden`: `actual_file` holds the committed expectation, line ends aside.
#[track_caller]
fn assert_golden(golden_relative_path: &str, actual_file: &Path) {
    let golden = common::fixture(golden_relative_path);
    let actual = read_normalized(actual_file);

    assert!(
        golden.is_file(),
        "missing golden file: {golden_relative_path}"
    );
    assert_eq!(
        read_normalized(&golden),
        actual,
        "{golden_relative_path} differs"
    );
}

/// C# `ReadFixtureGlobalPriority`: the app-level priority of the committed fixture.
fn read_fixture_global_priority(fixture: &str) -> Option<String> {
    JsonLayoutStore::new(common::fixture(fixture))
        .read(LAYOUT_ID, &[])
        .unwrap()
        .global_options
        .and_then(|o| o.priority)
}

//==================//
// Supported versions
//==================//

/// C# `Load_v52_MigratesWholeEdgeResistanceAndOrientedModelSize`.
#[test]
fn load_v52_migrates_whole_edge_resistance_and_oriented_model_size() {
    let work = Work::new();
    let layout = work.load("v5.2-pre-sections");
    let o = &layout.options;

    // Options that lived in the store before 5.5 arrive unchanged...
    assert_eq!(o.priority, "High");
    assert_eq!(o.priority_unhooked, "Idle");
    assert!(o.pinned);
    assert_eq!(o.border_values, "PerModel");
    // ...and options that did not exist yet keep their current default.
    assert!(!o.vcp_control);
    assert!(!o.hide_tray_icon);
    assert_eq!(o.rescue_shortcut, "Ctrl+Alt+Shift+M");
    assert!(o.freelook_enabled);

    // Pre-5.4.1 stored the size ORIENTED to the rotation at save time (#507): the
    // portrait 338x598 contradicts the intrinsic landscape panel and is transposed.
    let size = &model(&layout).physical_size;
    assert_eq!(size.width(), 598.0);
    assert_eq!(size.height(), 338.0);
    assert_eq!(size.borders().left, 8.0);
    assert_eq!(size.borders().bottom, 14.0);
    assert_eq!(
        model(&layout).pnp_device_name.as_deref(),
        Some("Test Monitor 24")
    );

    let depth_projection = depth_projection(&layout);
    assert_eq!(depth_projection.x, 320.5);
    assert_eq!(depth_projection.y, -12.0);
    let m = monitor(&layout);
    assert!(m.placed);

    // One resistance per edge, the only shape that existed before the section
    // editor: it becomes the section that says the same thing, spanning the edge.
    let br = &m.border_resistance;
    for (side, length) in [
        (&br.left, depth_projection.height),
        (&br.right, depth_projection.height),
    ] {
        let section = single(&side.sections);
        assert_eq!(section.from(), 0.0);
        assert_eq!(section.to(), length);
        assert_eq!(section.move_resistance(), 20.0);
        assert_eq!(section.drag(), 20.0);
        assert!(!section.move_block());
        assert!(!section.drag_block());
    }

    // A zero edge is what "no resistance" always looked like: no section for it.
    assert!(br.top.sections.is_empty());
    assert!(br.bottom.sections.is_empty());

    // No per-monitor Borders in the document: the monitor still mirrors its model.
    assert!(!m.borders_customized());
    assert_eq!(m.borders().left, 8.0);
}

/// C# `Load_v55_ReadsSectionsAndPerMonitorBorders`.
#[test]
fn load_v55_reads_sections_and_per_monitor_borders() {
    let work = Work::new();
    let layout = work.load("v5.5-sections");
    let o = &layout.options;

    assert!(o.vcp_control);
    assert_eq!(o.border_values, "PerMonitor");
    // The layout document overrides the app-level priorities, and only those.
    assert_eq!(o.priority, "Realtime");
    assert_eq!(o.priority_unhooked, "Idle");
    assert_eq!(o.algorithm, "Cross");
    assert_eq!(o.max_travel_distance, 150.0);
    assert!(o.loop_x);

    // Post-5.4.1: the stored size is intrinsic already, nothing to transpose.
    assert_eq!(model(&layout).physical_size.width(), 598.0);
    assert_eq!(model(&layout).physical_size.height(), 338.0);

    let m = monitor(&layout);
    let left = single(&m.border_resistance.left.sections);
    assert_eq!(left.from(), 0.0);
    assert_eq!(left.to(), 338.0);
    assert_eq!(left.move_resistance(), 20.0);

    let right = single(&m.border_resistance.right.sections);
    assert_eq!(right.from(), 50.0);
    assert_eq!(right.to(), 150.0);
    assert_eq!(right.move_resistance(), 5.0);
    assert!(right.move_block());
    assert_eq!(right.drag(), 0.0);

    // Sections present: the sibling Move/Drag of the same edge are NOT migrated on
    // top of them (they are what the section list replaced).
    assert_eq!(m.border_resistance.left.sections.len(), 1);

    assert!(m.borders_customized());
    assert_eq!(m.borders().left, 5.0);
    assert_eq!(m.borders().bottom, 8.0);
    assert!(m.excluded_from_layout);
}

/// C# `Load_v56_ReadsEveryCurrentField`.
#[test]
fn load_v56_reads_every_current_field() {
    let work = Work::new();
    let layout = work.load("v5.6-current");
    let o = &layout.options;

    assert!(o.hide_tray_icon);
    assert!(o.pinned);
    assert!(!o.auto_update);
    assert_eq!(o.rescue_shortcut, "Ctrl+Alt+Shift+M");

    let m = monitor(&layout);
    assert_eq!(m.border_resistance.left.sections.len(), 2);
    let second = &m.border_resistance.left.sections[1];
    assert_eq!(second.from(), 120.0);
    assert_eq!(second.to(), 338.0);
    assert!(second.move_block());
    assert!(second.drag_block());

    assert!(m.border_resistance.top.sections.is_empty());
    assert!(!m.excluded_from_layout);

    // The source is attached: the live geometry wins over the stored backup.
    assert_eq!(source(&layout).source.in_pixel.width, 1920.0);
    assert_eq!(m.active_source.as_deref(), Some(SOURCE_ID));
}

//==================//
// Round trip       //
//==================//

/// C# `SaveAfterLoad_v56_ReproducesTheStoredFiles`.
#[test]
fn save_after_load_v56_reproduces_the_stored_files() {
    let work = Work::new();
    let dir = work.fixture("v5.6-current");
    let store = JsonLayoutStore::new(&dir);
    let mut persistence = work.new_persistence(store);

    let mut layout = new_layout();
    persistence.load(&mut layout).unwrap();
    assert!(persistence.save(&mut layout).unwrap());

    // A save right after a load must not change what the user has: every byte the
    // current version writes is compared against the committed expectation.
    //
    // Two differences from the loaded document are visible in the golden:
    //  - the layout's Priority/PriorityUnhooked moved to options.json, which is the
    //    one place they are stored now — see
    //    save_after_load_promotes_the_layout_priority_to_the_app_level_and_drops_it.
    //  - RescueShortcut comes back with every '+' written as a unicode escape: that
    //    is the default JSON encoder, and the string parses back identically, so a
    //    hand-edited file keeps working. Only the bytes differ.
    assert_golden("v5.6-current-saved/options.json", &dir.join("options.json"));
    assert_golden("v5.6-current-saved/models.json", &dir.join("models.json"));
    assert_golden(
        &format!("v5.6-current-saved/layouts/{LAYOUT_ID}.json"),
        &layout_file(&dir),
    );
}

/// C# `SaveAfterLoad_v52_UpgradesTheDocumentToTheCurrentShape`.
#[test]
fn save_after_load_v52_upgrades_the_document_to_the_current_shape() {
    let work = Work::new();
    let dir = work.fixture("v5.2-pre-sections");
    let store = JsonLayoutStore::new(&dir);
    let mut persistence = work.new_persistence(store);

    let mut layout = new_layout();
    persistence.load(&mut layout).unwrap();
    persistence.save(&mut layout).unwrap();

    // The migrations only reach the store here — and once written, re-reading them
    // is a no-op: the bare per-edge numbers are gone, replaced by the sections that
    // carry the same setting, and the transposed size is now intrinsic.
    assert_golden(
        "v5.2-pre-sections-saved/models.json",
        &dir.join("models.json"),
    );
    assert_golden(
        &format!("v5.2-pre-sections-saved/layouts/{LAYOUT_ID}.json"),
        &layout_file(&dir),
    );

    // Reloading the upgraded document yields the same model.
    let mut reloaded = new_layout();
    work.new_persistence(JsonLayoutStore::new(&dir))
        .load(&mut reloaded)
        .unwrap();

    let section = single(&monitor(&reloaded).border_resistance.left.sections);
    assert_eq!(section.from(), 0.0);
    assert_eq!(section.to(), depth_projection(&reloaded).height);
    assert_eq!(section.move_resistance(), 20.0);
    assert_eq!(model(&reloaded).physical_size.width(), 598.0);
}

/// C# `SaveAfterLoad_PromotesTheLayoutPriorityToTheAppLevelAndDropsIt`.
#[test]
fn save_after_load_promotes_the_layout_priority_to_the_app_level_and_drops_it() {
    // A layout carrying its own Priority still wins at load — that is what keeps an
    // upgrade from changing anybody's setting. The save then stores it once, at the
    // app level, and the layout copy is gone: the two locations were never two
    // settings (both load into the same options property), and keeping the copy is
    // what used to hand one layout's value to all the others.
    let work = Work::new();
    let dir = work.fixture("v5.6-current");
    let store = JsonLayoutStore::new(&dir);

    let mut layout = new_layout();
    let mut persistence = work.new_persistence(store);
    persistence.load(&mut layout).unwrap();

    assert_eq!(
        read_fixture_global_priority("v5.6-current").as_deref(),
        Some("High")
    );
    assert_eq!(layout.options.priority, "Realtime");

    persistence.save(&mut layout).unwrap();

    let saved = persistence.store().read(LAYOUT_ID, &[]).unwrap();
    let global = saved.global_options.unwrap();
    assert_eq!(global.priority.as_deref(), Some("Realtime"));
    assert_eq!(global.priority_unhooked.as_deref(), Some("Idle"));
    let layout_options = saved.layout.unwrap().options.unwrap();
    assert_eq!(layout_options.priority, None);
    assert_eq!(layout_options.priority_unhooked, None);

    // And it stays there: a reload finds one value, in one place.
    let mut reloaded = new_layout();
    work.new_persistence(JsonLayoutStore::new(&dir))
        .load(&mut reloaded)
        .unwrap();
    assert_eq!(reloaded.options.priority, "Realtime");
}

/// C# `SaveEnabled_DoesNotPutTheLayoutPriorityBack`.
#[test]
fn save_enabled_does_not_put_the_layout_priority_back() {
    // SaveEnabled rewrites the document it just read, so without care it would
    // restore the legacy keys a full save had migrated away.
    let work = Work::new();
    let dir = work.fixture("v5.6-current");
    let store = JsonLayoutStore::new(&dir);

    let mut layout = new_layout();
    let mut persistence = work.new_persistence(store);
    persistence.load(&mut layout).unwrap();

    layout.edit_options(|o| o.enabled = false);
    assert!(persistence.save_enabled(&layout).unwrap());

    let saved = persistence
        .store()
        .read(LAYOUT_ID, &[])
        .unwrap()
        .layout
        .unwrap();
    let options = saved.options.as_ref().unwrap();
    assert_eq!(options.enabled, Some(false));
    assert_eq!(options.priority, None);
    // Everything else the document held is still there.
    assert_eq!(options.algorithm.as_deref(), Some("Cross"));
    assert!(saved.monitors.contains_key(MONITOR_ID));
}

//==================//
// Incomplete data  //
//==================//

/// C# `Load_EmptyDocuments_KeepEveryDefault`.
#[test]
fn load_empty_documents_keep_every_default() {
    let work = Work::new();
    let layout = work.load("incomplete-empty");

    // "{}" everywhere is indistinguishable from a missing file: the live model wins.
    assert_eq!(layout.options.priority, "Normal");
    assert_eq!(layout.options.algorithm, "Strait");
    assert!(layout.options.enabled);

    assert_eq!(model(&layout).physical_size.width(), 600.0);
    assert_eq!(model(&layout).physical_size.height(), 340.0);
    assert_eq!(model(&layout).pnp_device_name.as_deref(), Some("Live name"));

    let m = monitor(&layout);
    assert!(!m.placed);
    assert!(!m.borders_customized());
    assert!(m.border_resistance.left.sections.is_empty());
    assert_eq!(source(&layout).source.in_pixel.width, 1920.0);

    // Nothing stored is still a complete load: the subtree must be flagged saved so
    // the next edit is an observable transition. (monitor.Saved folds into the
    // layout's flag, see the module doc.)
    assert!(layout.saved());
}

/// C# `Load_PartialDocuments_FillWhatIsThereAndKeepTheRest`.
#[test]
fn load_partial_documents_fill_what_is_there_and_keep_the_rest() {
    let work = Work::new();
    let layout = work.load("incomplete-partial");

    // An explicit null is "absent", exactly like a missing property, and an unknown
    // property is ignored rather than fatal (a file written by a newer version).
    assert_eq!(layout.options.priority, "Normal");
    assert_eq!(layout.options.border_values, "PerMonitor");
    assert!(!layout.options.enabled);

    // Stored 0x0: the EDID-less placeholder older versions persisted (#419) must
    // never override the freshly computed size. An empty PnpName is not a name.
    assert_eq!(model(&layout).physical_size.width(), 600.0);
    assert_eq!(model(&layout).physical_size.height(), 340.0);
    assert_eq!(model(&layout).pnp_device_name.as_deref(), Some("Live name"));

    // Half a location is still a placement; the missing half keeps the live value.
    let m = monitor(&layout);
    assert_eq!(depth_projection(&layout).x, 12.5);
    assert!(m.placed);

    // An explicitly empty section list clears the edge; a zero, unblocked legacy
    // pair migrates to nothing; an absent edge is left as the model has it.
    assert!(m.border_resistance.right.sections.is_empty());
    assert!(m.border_resistance.bottom.sections.is_empty());
    assert!(m.border_resistance.left.sections.is_empty());

    // Presence of Borders is the "monitor owns them" flag, even partial: the sides
    // it does not name keep mirroring the model.
    assert!(m.borders_customized());
    assert_eq!(m.borders().left, 5.0);
    assert_eq!(m.borders().top, 11.0);
    assert_eq!(m.borders().bottom, 13.0);

    // A stored source this machine no longer has, and a stored monitor absent from
    // the layout, are both ignored — the live geometry is untouched.
    assert_eq!(source(&layout).source.in_pixel.x, 0.0);
    assert_eq!(source(&layout).source.in_pixel.width, 1920.0);
    assert_eq!(layout.monitors().len(), 1);
}

/// C# `Load_UnknownModelsInTheStore_AreNotApplied`.
#[test]
fn load_unknown_models_in_the_store_are_not_applied() {
    // models.json holds a second model (OTHER99) belonging to another machine's
    // monitor: the store is asked for the PnP codes present, and nothing else.
    let work = Work::new();
    let layout = work.load("incomplete-partial");

    assert_eq!(model(&layout).pnp_code, PNP);
    assert_ne!(model(&layout).physical_size.width(), 700.0);
}

//==================//
// Excluded list    //
//==================//

/// C# `Load_ExcludedDefaultsTopUp_RewritesTheStoredOptionsWithoutLosingThem`.
#[test]
fn load_excluded_defaults_top_up_rewrites_the_stored_options_without_losing_them() {
    let work = Work::new();
    let dir = work.fixture("v5.5-sections");
    let excluded = work.path().join("excluded").join("Excluded.txt");
    fs::create_dir_all(excluded.parent().unwrap()).unwrap();
    write_lines(&excluded, LEGACY_V0);

    let mut layout = new_layout();
    test_persistence(JsonLayoutStore::new(&dir), &excluded)
        .load(&mut layout)
        .unwrap();

    // The stored version (1) is behind: the top-up runs and records itself, which is
    // a WRITE during a load. It goes through the document as read, so everything
    // else stored keeps its value.
    let options = JsonLayoutStore::new(&dir)
        .read(LAYOUT_ID, &[])
        .unwrap()
        .global_options
        .unwrap();
    assert_eq!(options.excluded_defaults_version, Some(VERSION));
    assert_eq!(options.vcp_control, Some(true));
    assert_eq!(options.start_minimized, Some(true));
    assert_eq!(options.border_values.as_deref(), Some("PerMonitor"));

    let file = read_lines(&excluded);
    for entry in ALL {
        assert!(
            contains_entry(&layout.options.excluded_list, entry),
            "missing default: {entry}"
        );
        assert!(
            contains_entry(&file, entry),
            "missing default in the file the daemon reads: {entry}"
        );
    }
}

/// C# `Load_ExcludedFileCommentLines_StayOutOfTheListAndSurviveASave`.
#[test]
fn load_excluded_file_comment_lines_stay_out_of_the_list_and_survive_a_save() {
    // A real file as CreateExcludedFile seeds it, plus a line the user added by hand.
    // The daemon skips ':' lines and empty ones (daemon::load_excluded), so neither is
    // an exclusion — but neither may be lost when the app rewrites the file either.
    let work = Work::new();
    let dir = work.fixture("v5.6-current"); // ExcludedDefaultsVersion 2: no top-up interference
    let excluded = work.path().join("commented").join("Excluded.txt");
    fs::create_dir_all(excluded.parent().unwrap()).unwrap();
    let lines: Vec<&str> = [HEADER]
        .into_iter()
        .chain(ALL.iter().copied())
        .chain(["", ":my own note"])
        .collect();
    write_lines(&excluded, &lines);

    let mut layout = new_layout();
    let mut persistence = test_persistence(JsonLayoutStore::new(&dir), &excluded);
    persistence.load(&mut layout).unwrap();

    assert_eq!(layout.options.excluded_list, ALL);

    persistence.save_live(&layout.options).unwrap();

    let written = read_lines(&excluded);
    assert_eq!(written[0], HEADER);
    assert_eq!(written[1], ":my own note");
    assert_eq!(&written[2..], ALL);
}
