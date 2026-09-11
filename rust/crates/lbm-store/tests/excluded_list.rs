//! The excluded-processes file: what the port adds to the C# tests of it.
//!
//! In C# the list is tested through `LayoutPersistence.Load`/`SaveLive`, and so are its
//! ports (`layout_persistence.rs`, `layout_persistence_golden.rs`). The tests here drive
//! [`ExcludedListPersistence`] directly, the way the engine does: `load` with the
//! options document the store read, `write` for a save.

mod common;

use std::fs;
use std::path::{Path, PathBuf};

use common::{read_lines, write_lines, FakeStore};
use lbm_store::excluded_process_defaults::{ALL, LEGACY_V0, VERSION};
use lbm_store::{ExcludedListPersistence, GlobalOptionsDto, JsonLayoutStore, LayoutStore};
use serde_json::json;
use tempfile::TempDir;

/// The fixtures' layout id.
const LAYOUT_ID: &str = "TST1234_1920x1080";

/// A store whose options record `version` as the top-up already applied.
fn store_with_version(version: i32) -> FakeStore {
    let store = FakeStore::default();
    *store.global_options.borrow_mut() = Some(GlobalOptionsDto {
        excluded_defaults_version: Some(version),
        ..GlobalOptionsDto::default()
    });
    store
}

/// C# `TempExcludedFile`: a path in a fresh directory, no file yet.
fn temp_excluded_file() -> (TempDir, PathBuf) {
    common::temp_excluded_file("lbm-persistence-tests")
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
    let store = store_with_version(VERSION);
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
