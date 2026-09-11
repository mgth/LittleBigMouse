//! Helpers shared by the integration tests.
//!
//! The fixtures are read from the SOURCE TREE, never copied: the C# suite reads the
//! very same files, so both languages are held to one set of bytes.

// Every test crate compiles this module, and none uses all of it.
#![allow(dead_code)]

use std::cell::RefCell;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use indexmap::IndexMap;
use lbm_layout::model::{
    DisplaySource, Layout, LayoutOptions, Monitor, MonitorModel, PhysicalSource,
};
use lbm_store::{
    GlobalOptionsDto, LayoutDto, LayoutPersistence, LayoutStore, LayoutStoreData, ModelDto,
    PersistencePlatform,
};
use tempfile::TempDir;

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

/// xUnit `Assert.Single`: the one item of `items`.
#[track_caller]
pub fn single<T: std::fmt::Debug>(items: &[T]) -> &T {
    assert_eq!(items.len(), 1, "expected a single item: {items:?}");
    &items[0]
}

/// C# `File.WriteAllLines`: every line ended by a new line.
pub fn write_lines(file: &Path, lines: &[&str]) {
    let text: String = lines.iter().map(|line| format!("{line}\n")).collect();
    fs::write(file, text).unwrap();
}

/// C# `File.ReadAllLines`, for files with ordinary line ends.
pub fn read_lines(file: &Path) -> Vec<String> {
    read_normalized(file).lines().map(str::to_owned).collect()
}

//==================//
// Engine           //
//==================//

/// C# `ILayoutOptions.Design`, the options object the C# persistence tests build their
/// layouts on. Its defaults are not `LbmOptions`' ([`LayoutOptions::default`]):
/// `Enabled`, `AutoUpdate` and `Elevated` start true, and the excluded list holds two
/// sample entries.
///
/// (`Design`'s setters raise no change notification, so in those C# tests the monitors'
/// `MonitorBorderPolicy` never sees "Border values" change and keeps building the
/// geometry on the model's borders; the Rust layout reads the option live. The borders
/// only move the outside bounds, which none of the ported assertions reads.)
pub fn design_options() -> LayoutOptions {
    LayoutOptions {
        enabled: true,
        auto_update: true,
        elevated: true,
        excluded_list: vec!["/game/".to_owned(), "/another/game/".to_owned()],
        ..LayoutOptions::default()
    }
}

/// How the C# tests' `NewLayout` puts a monitor with one source in a layout:
/// `monitor.ActiveSource = physicalSource; monitor.Sources.Add(physicalSource);
/// layout.AddOrUpdatePhysicalMonitor(monitor); layout.AddOrUpdatePhysicalSource(physicalSource)`
/// — the order of `LinuxLayoutMapping.AddMonitor`, which `lbm_layout::linux::add_monitor`
/// follows too. The model joins the layout first: C# hands the model object to the
/// monitor, the Rust monitor names it by PnP code.
pub fn add_monitor_with_source(
    layout: &mut Layout,
    model: MonitorModel,
    mut monitor: Monitor,
    source: DisplaySource,
    device_id: &str,
) {
    let pnp_code = model.pnp_code.clone();
    layout.get_or_add_model(&pnp_code, move |_| model);

    let source_id = source.id.clone();
    let physical_source = PhysicalSource::new(device_id, monitor.id.clone(), source);
    monitor.active_source = Some(source_id.clone());
    monitor.sources.push(source_id);
    layout.attach_source(physical_source.clone());
    layout.add_or_update_monitor(monitor);
    layout.add_or_update_source(physical_source);
}

/// The platform hooks of the C# tests' `TestPersistence`: the base class's. Elevation is
/// C#'s real `Environment.IsPrivilegedProcess`, which no test reads; the process counts
/// as not elevated, as in the domain oracle.
pub struct TestPlatform;

impl PersistencePlatform for TestPlatform {
    fn is_elevated(&self) -> bool {
        false
    }
}

/// C# `TestPersistence(store, excludedFile)`: the engine with its excluded-processes
/// file redirected — never the user's real one.
pub fn test_persistence<S: LayoutStore>(
    store: S,
    excluded_file: &Path,
) -> LayoutPersistence<S, TestPlatform> {
    let file = excluded_file.to_path_buf();
    LayoutPersistence::with_excluded_list_file(store, TestPlatform, move || file.clone())
}

/// C# `TempExcludedFile`: `Excluded.txt` in a fresh directory (created, the file is
/// not), deleted with the returned guard.
pub fn temp_excluded_file(prefix: &str) -> (TempDir, PathBuf) {
    let dir = tempfile::Builder::new().prefix(prefix).tempdir().unwrap();
    let file = dir.path().join("Excluded.txt");
    (dir, file)
}

/// C# `FakeStore` of `LayoutPersistenceTests` and `VirtualLayoutGuardTests` (the same
/// class twice): an in-memory store. The engine takes its store by value, so the
/// documents are shared: every clone sees them, and a test keeps a clone to seed and
/// inspect them — C# keeps a reference.
#[derive(Clone, Default)]
pub struct FakeStore {
    pub global_options: Rc<RefCell<Option<GlobalOptionsDto>>>,
    pub layouts: Rc<RefCell<IndexMap<String, LayoutDto>>>,
    pub models: Rc<RefCell<IndexMap<String, ModelDto>>>,
}

impl LayoutStore for FakeStore {
    fn read(&self, layout_id: &str, pnp_codes: &[&str]) -> io::Result<LayoutStoreData> {
        Ok(LayoutStoreData {
            global_options: self.global_options.borrow().clone(),
            layout: self.layouts.borrow().get(layout_id).cloned(),
            models: self
                .models
                .borrow()
                .iter()
                .filter(|(pnp_code, _)| pnp_codes.contains(&pnp_code.as_str()))
                .map(|(pnp_code, model)| (pnp_code.clone(), model.clone()))
                .collect(),
        })
    }

    fn write_global_options(&self, options: &GlobalOptionsDto) -> io::Result<()> {
        *self.global_options.borrow_mut() = Some(options.clone());
        Ok(())
    }

    fn write_layout(&self, layout_id: &str, layout: &LayoutDto) -> io::Result<()> {
        self.layouts
            .borrow_mut()
            .insert(layout_id.to_owned(), layout.clone());
        Ok(())
    }

    fn write_models(&self, models: &IndexMap<String, ModelDto>) -> io::Result<()> {
        let mut stored = self.models.borrow_mut();
        for (pnp_code, model) in models {
            stored.insert(pnp_code.clone(), model.clone());
        }
        Ok(())
    }
}
