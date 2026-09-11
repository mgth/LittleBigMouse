//! Helpers shared by the integration tests.
//!
//! The fixtures are read from the SOURCE TREE, never copied: the C# suite reads the
//! very same files, so both languages are held to one set of bytes.

// Every test crate compiles this module, and none uses all of it.
#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};

/// The repository root.
pub fn repo_root() -> PathBuf {
    // rust/crates/lbm-store -> repository root.
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .expect("crate lives at rust/crates/lbm-store")
        .to_path_buf()
}

/// `TestData/Persistence` of the C# test project: configuration directories as the
/// supported versions wrote them.
pub fn persistence_fixtures() -> PathBuf {
    repo_root().join("LittleBigMouse.Core/LittleBigMouse.DisplayLayout.Tests/TestData/Persistence")
}

/// One fixture directory, read-only.
pub fn fixture(name: &str) -> PathBuf {
    persistence_fixtures().join(name)
}

/// C# `LayoutPersistenceGoldenTests.Fixture`: a writable copy of a fixture, since
/// loading may migrate and saving certainly writes, and the committed fixture must
/// stay the file the old version wrote.
pub fn fixture_copy(name: &str, work: &Path) -> PathBuf {
    let target = work.join(name);
    copy_dir(&fixture(name), &target);
    target
}

fn copy_dir(source: &Path, target: &Path) {
    fs::create_dir_all(target).unwrap();
    for entry in fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let to = target.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir(&entry.path(), &to);
        } else {
            fs::copy(entry.path(), &to).unwrap();
        }
    }
}

/// The domain oracle's scenarios: what the C# pipeline did, recorded.
pub fn oracle_scenarios() -> PathBuf {
    repo_root().join("domain-oracle/scenarios")
}

/// C# `LayoutStoreKeyTests.MonitorId`: one Windows monitor id — PnP code, EDID serial,
/// week, year, checksum — 29 characters.
pub fn monitor_id(i: usize) -> String {
    format!("GSM5B09{i:02}NTMX5X567_0C_07E5_A{}", i % 10)
}

/// C# `LayoutStoreKeyTests.LayoutId`: the layout id of that many monitors, as
/// `ComputeId` joins them.
pub fn layout_id(monitors: usize) -> String {
    (1..=monitors).map(monitor_id).collect::<Vec<_>>().join("+")
}

/// A file's text with CRLF normalised to LF: the fixtures are `text=auto`, so a
/// Windows checkout may have rewritten them. Mirrors the C# `Normalize`.
pub fn read_normalized(path: &Path) -> String {
    fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
        .replace("\r\n", "\n")
}
