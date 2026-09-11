//! The excluded-processes file: the C# tests of it, then what the port adds.
//!
//! In C# these run through `LayoutPersistence.Load`/`SaveLive`
//! (`LayoutPersistenceTests.cs`, `LayoutPersistenceGoldenTests.cs`); the engine is not
//! ported here, so each one drives [`ExcludedListPersistence`] the way the engine
//! does: `load` with the options document the store read, `write` for a save. Same
//! names as the C# tests; the assertions that belong to the options mapping are left
//! to its port.

mod common;

use std::cell::RefCell;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use indexmap::IndexMap;
use lbm_store::excluded_process_defaults::{contains_entry, ALL, HEADER, LEGACY_V0, VERSION};
use lbm_store::{
    ExcludedListPersistence, GlobalOptionsDto, JsonLayoutStore, LayoutDto, LayoutStore,
    LayoutStoreData, ModelDto,
};
use serde_json::json;
use tempfile::TempDir;

/// The fixtures' layout id.
const LAYOUT_ID: &str = "TST1234_1920x1080";

/// C# `LayoutPersistenceTests.FakeStore`, as far as the list is concerned: the
/// global options, in memory.
#[derive(Default)]
struct FakeStore {
    global_options: RefCell<Option<GlobalOptionsDto>>,
}

impl FakeStore {
    fn with_version(version: i32) -> Self {
        FakeStore {
            global_options: RefCell::new(Some(GlobalOptionsDto {
                excluded_defaults_version: Some(version),
                ..GlobalOptionsDto::default()
            })),
        }
    }

    fn stored_version(&self) -> Option<i32> {
        self.global_options
            .borrow()
            .as_ref()
            .and_then(|o| o.excluded_defaults_version)
    }
}

impl LayoutStore for FakeStore {
    fn read(&self, _layout_id: &str, _pnp_codes: &[&str]) -> io::Result<LayoutStoreData> {
        Ok(LayoutStoreData {
            global_options: self.global_options.borrow().clone(),
            ..LayoutStoreData::default()
        })
    }

    fn write_global_options(&self, options: &GlobalOptionsDto) -> io::Result<()> {
        *self.global_options.borrow_mut() = Some(options.clone());
        Ok(())
    }

    fn write_layout(&self, _layout_id: &str, _layout: &LayoutDto) -> io::Result<()> {
        Ok(())
    }

    fn write_models(&self, _models: &IndexMap<String, ModelDto>) -> io::Result<()> {
        Ok(())
    }
}

/// C# `TempExcludedFile`: a path in a fresh directory, no file yet.
fn temp_excluded_file() -> (TempDir, PathBuf) {
    let dir = tempfile::Builder::new()
        .prefix("lbm-persistence-tests")
        .tempdir()
        .unwrap();
    let file = dir.path().join("Excluded.txt");
    (dir, file)
}

/// C# `File.WriteAllLines`.
fn write_lines(file: &Path, lines: &[&str]) {
    let text: String = lines.iter().map(|line| format!("{line}\n")).collect();
    fs::write(file, text).unwrap();
}

/// C# `File.ReadAllLines`, for files with ordinary line ends.
fn read_lines(file: &Path) -> Vec<String> {
    common::read_normalized(file)
        .lines()
        .map(str::to_owned)
        .collect()
}

/// What `LayoutPersistence.Load` does with the list: read the store, load the file
/// with the options document read.
fn load<S: LayoutStore>(
    store: &S,
    file: &Path,
) -> (ExcludedListPersistence<impl Fn() -> PathBuf>, Vec<String>) {
    let path = file.to_path_buf();
    let mut persistence = ExcludedListPersistence::new(move || path.clone());
    let mut data = store.read(LAYOUT_ID, &[]).unwrap();
    let mut list = Vec::new();
    persistence
        .load(store, &mut list, data.global_options.as_mut())
        .unwrap();
    (persistence, list)
}

/// C# `NewDefaults`: current defaults that are NOT legacy ones (the entries a top-up
/// would add).
fn new_defaults() -> Vec<&'static str> {
    ALL.iter()
        .copied()
        .filter(|entry| !contains_entry(LEGACY_V0, entry))
        .collect()
}

//=== LayoutPersistenceTests.cs ===//

#[test]
fn load_first_run_seeds_excluded_defaults_file() {
    let store = FakeStore::default();
    let (_dir, excluded) = temp_excluded_file();

    let (_, list) = load(&store, &excluded);

    // The header is a comment for the daemon, so it is not an entry of the list — but
    // the seeded file carries it, exactly like the one CreateExcludedFile writes.
    assert_eq!(list, ALL);
    let expected: Vec<&str> = [HEADER].into_iter().chain(ALL.iter().copied()).collect();
    assert_eq!(read_lines(&excluded), expected);
}

#[test]
fn excluded_list_comments_stay_out_of_the_list_and_survive_a_save() {
    // Current version: the top-up must not interfere with what is asserted here.
    let store = FakeStore::with_version(VERSION);
    let (_dir, excluded) = temp_excluded_file();
    write_lines(&excluded, &[HEADER, r"\steamapps\", "", ":my own note"]);

    let (persistence, mut list) = load(&store, &excluded);

    // Only the exclusions reach the model: the daemon skips ':' lines and empty ones,
    // and the options list is what the user edits — it must hold the same thing.
    assert_eq!(list, [r"\steamapps\"]);

    list.push(r"\my\game\".into());
    persistence.write(&list).unwrap();

    // The comments are still on disk, above the entries: they were never the list's to
    // drop, and one of them may be the user's own annotation.
    assert_eq!(
        read_lines(&excluded),
        [HEADER, ":my own note", r"\steamapps\", r"\my\game\"]
    );
}

#[test]
fn load_excluded_defaults_topped_up_once() {
    let store = FakeStore::default();
    let (_dir, excluded) = temp_excluded_file();
    write_lines(&excluded, LEGACY_V0);

    let (_, list) = load(&store, &excluded);

    // The list kept all previous defaults: new defaults are added, file included, and
    // the applied version is recorded in the store. Separator-insensitive: on Linux the
    // current defaults are slash-style while LegacyV0 is Windows-style.
    let file = read_lines(&excluded);
    for entry in ALL {
        assert!(contains_entry(&list, entry), "missing default: {entry}");
        assert!(
            contains_entry(&file, entry),
            "missing default in file: {entry}"
        );
    }
    assert_eq!(store.stored_version(), Some(VERSION));
}

#[test]
fn load_excluded_defaults_customized_list_left_untouched() {
    let store = FakeStore::default();
    let (_dir, excluded) = temp_excluded_file();
    // A legacy default was removed by the user: no top-up, but the version is recorded
    // so the migration never runs again.
    write_lines(&excluded, &[r"\steamapps\", r"\my\own\path\"]);

    let (_, list) = load(&store, &excluded);

    for entry in new_defaults() {
        assert!(
            !contains_entry(&list, entry),
            "top-up ran on a customized list: {entry}"
        );
    }
    assert_eq!(store.stored_version(), Some(VERSION));
}

#[test]
fn load_excluded_defaults_already_applied_does_nothing() {
    let store = FakeStore::with_version(VERSION);
    let (_dir, excluded) = temp_excluded_file();
    write_lines(&excluded, LEGACY_V0);

    let (_, list) = load(&store, &excluded);

    for entry in new_defaults() {
        assert!(!contains_entry(&list, entry), "top-up ran twice: {entry}");
    }
}

/// The list half of the C# test; `HideTrayIcon` and the version landing in
/// `options.json` are the options mapping's, which writes
/// `applied_defaults_version()` into every global-options write.
#[test]
fn save_live_writes_global_options_and_excluded_file() {
    let store = FakeStore::default();
    let (_dir, excluded) = temp_excluded_file();

    let (persistence, mut list) = load(&store, &excluded);

    list.push(r"\my\game\".into());
    persistence.write(&list).unwrap();

    // The version survives a live save even though it is not part of the options model.
    assert_eq!(persistence.applied_defaults_version(), Some(VERSION));
    assert!(read_lines(&excluded)
        .iter()
        .any(|line| line == r"\my\game\"));
}

//=== LayoutPersistenceGoldenTests.cs ===//

#[test]
fn load_excluded_defaults_top_up_rewrites_the_stored_options_without_losing_them() {
    let work = tempfile::tempdir().unwrap();
    let dir = common::fixture_copy("v5.5-sections", work.path());
    let excluded = work.path().join("excluded").join("Excluded.txt");
    fs::create_dir_all(excluded.parent().unwrap()).unwrap();
    write_lines(&excluded, LEGACY_V0);

    let (_, list) = load(&JsonLayoutStore::new(&dir), &excluded);

    // The stored version (1) is behind: the top-up runs and records itself, which is a
    // WRITE during a load. It goes through the document as read, so everything else
    // stored keeps its value.
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
        assert!(contains_entry(&list, entry), "missing default: {entry}");
        assert!(
            contains_entry(&file, entry),
            "missing default in the file the daemon reads: {entry}"
        );
    }
}

#[test]
fn load_excluded_file_comment_lines_stay_out_of_the_list_and_survive_a_save() {
    // A real file as CreateExcludedFile seeds it, plus a line the user added by hand.
    // The daemon skips ':' lines and empty ones (daemon::load_excluded), so neither is
    // an exclusion — but neither may be lost when the app rewrites the file either.
    let work = tempfile::tempdir().unwrap();
    let dir = common::fixture_copy("v5.6-current", work.path()); // ExcludedDefaultsVersion 2
    let excluded = work.path().join("commented").join("Excluded.txt");
    fs::create_dir_all(excluded.parent().unwrap()).unwrap();
    let lines: Vec<&str> = [HEADER]
        .into_iter()
        .chain(ALL.iter().copied())
        .chain(["", ":my own note"])
        .collect();
    write_lines(&excluded, &lines);

    let (persistence, list) = load(&JsonLayoutStore::new(&dir), &excluded);

    assert_eq!(list, ALL);

    persistence.write(&list).unwrap();

    let written = read_lines(&excluded);
    assert_eq!(written[0], HEADER);
    assert_eq!(written[1], ":my own note");
    assert_eq!(&written[2..], ALL);
}

//=== Not in C# ===//

#[test]
fn top_up_keeps_the_unknown_members_of_the_stored_options() {
    // C# rewrites options.json without the members it does not know (DaemonPort here);
    // the Rust top-up writes the document it read, unknown members included.
    let work = tempfile::tempdir().unwrap();
    let dir = common::fixture_copy("v5.5-sections", work.path());
    let (_excluded_dir, excluded) = temp_excluded_file();
    write_lines(&excluded, LEGACY_V0);

    load(&JsonLayoutStore::new(&dir), &excluded);

    let options = JsonLayoutStore::new(&dir)
        .read(LAYOUT_ID, &[])
        .unwrap()
        .global_options
        .unwrap();
    assert_eq!(options.excluded_defaults_version, Some(VERSION));
    assert_eq!(options.extra.get("DaemonPort"), Some(&json!(25196)));
}

#[test]
fn a_top_up_without_stored_options_writes_the_version_alone() {
    let store = FakeStore::default();
    let (_dir, excluded) = temp_excluded_file();
    write_lines(&excluded, LEGACY_V0);

    load(&store, &excluded);

    assert_eq!(
        *store.global_options.borrow(),
        Some(GlobalOptionsDto {
            excluded_defaults_version: Some(VERSION),
            ..GlobalOptionsDto::default()
        })
    );
}

#[test]
fn the_file_is_split_like_file_read_all_lines() {
    // A byte-order mark, CRLF, a lone CR, a blank-looking line: .NET drops the mark and
    // ends a line at any of the three terminators; only a truly empty line is nothing.
    let store = FakeStore::with_version(VERSION);
    let (_dir, excluded) = temp_excluded_file();
    fs::write(&excluded, "\u{feff}:first\r\n  \r\nA\rB\n\n:second\nC").unwrap();

    let (persistence, list) = load(&store, &excluded);
    assert_eq!(list, ["  ", "A", "B", "C"]);

    persistence.write(&list).unwrap();
    assert_eq!(
        read_lines(&excluded),
        [":first", ":second", "  ", "A", "B", "C"]
    );
}

#[test]
fn writes_during_a_load_are_best_effort() {
    // A seed that cannot be written must not fail the load: the in-memory list is
    // correct either way, and the version is remembered for the next options write.
    let store = FakeStore::default();
    let (dir, _) = temp_excluded_file();
    let excluded = dir.path().join("missing-dir").join("Excluded.txt");

    let (persistence, list) = load(&store, &excluded);

    assert_eq!(list, ALL);
    assert_eq!(persistence.applied_defaults_version(), Some(VERSION));
    assert!(!excluded.exists());
    assert!(
        persistence.write(&list).is_err(),
        "an explicit write reports"
    );
}
