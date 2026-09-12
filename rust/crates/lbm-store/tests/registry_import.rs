//! The v5 Windows registry, read as C#'s `RegistryLayoutStore` reads it and imported
//! into the JSON store (decision D2), on regedit-format fixtures — and on Windows,
//! through the real registry as well.
//!
//! The fixtures in `tests/data/registry` are the registry twins of the C# JSON fixtures
//! of `TestData/Persistence`: importing one must give the documents the JSON store
//! reads from the other.

mod common;

use std::fs;
use std::path::Path;

use lbm_store::layout_store_key::key_for;
use lbm_store::reg_file::{self, MemoryKey, RegValue};
use lbm_store::registry_import::import_registry;
use lbm_store::registry_layout_store::{self as registry, RegistryKey, DEFAULT_ROOT_KEY};
use lbm_store::{JsonLayoutStore, LayoutStore, LayoutStoreData};

const LAYOUT_ID: &str = "TST1234_1920x1080";
const PNP: &str = "TST1234";

fn registry_fixture(name: &str) -> MemoryKey {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data/registry")
        .join(name);
    reg_file::parse(&fs::read(&path).unwrap()).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// `HKCU\SOFTWARE\Mgth\LittleBigMouse` in a parsed export.
fn lbm_root(tree: &MemoryKey) -> &MemoryKey {
    tree.open(&format!(r"HKEY_CURRENT_USER\{DEFAULT_ROOT_KEY}"))
        .expect("the export holds the LittleBigMouse key")
}

/// What the JSON store holds for the fixture layout, without the members the DTOs do
/// not know (the JSON fixtures still carry the long-gone `DaemonPort`, which the
/// registry twin does not).
fn json_data(dir: &Path) -> LayoutStoreData {
    let mut data = JsonLayoutStore::new(dir).read(LAYOUT_ID, &[PNP]).unwrap();
    if let Some(global) = &mut data.global_options {
        global.extra.clear();
    }
    data
}

/// Imports `fixture` and checks it against the JSON fixture `twin`, both through the
/// C# read of the registry and through the JSON store the import wrote.
fn check_twin(fixture: &str, twin: &str) {
    let tree = registry_fixture(fixture);
    let root = lbm_root(&tree);
    let expected = json_data(&common::fixture(twin));

    assert_eq!(
        registry::read(Some(&root), LAYOUT_ID, &[PNP]),
        expected,
        "RegistryLayoutStore.Read of {fixture}"
    );

    let work = tempfile::tempdir().unwrap();
    let report = import_registry(&root, LAYOUT_ID, &JsonLayoutStore::new(work.path())).unwrap();
    assert_eq!(report.layouts, [LAYOUT_ID]);
    assert_eq!(report.models, [PNP]);
    assert!(report.skipped.is_empty());
    assert_eq!(json_data(work.path()), expected, "import of {fixture}");
}

#[test]
fn the_current_registry_imports_as_the_current_json_store() {
    check_twin("v5.6-current.reg", "v5.6-current");
}

#[test]
fn a_pre_sections_registry_imports_as_the_pre_sections_json_store() {
    // Whole-edge VALUES read as the legacy JSON numbers do (move = drag = the value),
    // left for the engine's migration to turn into sections at load.
    check_twin("v5.2-pre-sections.reg", "v5.2-pre-sections");
}

#[test]
fn legacy_locations_and_odd_values_read_as_in_csharp() {
    let mut tree = MemoryKey::new("");
    {
        let root = tree.create(DEFAULT_ROOT_KEY);
        // The former name of ShowMonitorActionWarning.
        root.set_string("ShowAttachDetachWarning", "0");
        // Not a REG_SZ: `GetValue(name) as string` is null.
        root.set_value("DebugTools", RegValue::Dword(1));
        root.set_string("ExcludedDefaultsVersion", "two");
    }
    {
        // Options that once lived in the layout key, the root having none.
        let layout = tree.create(&format!(r"{DEFAULT_ROOT_KEY}\Layouts\{LAYOUT_ID}"));
        layout.set_string("Priority", "BelowNormal");
        layout.set_string("HomeCinema", "1");
        layout.set_string("Pinned", "true"); // anything but "1" is false
    }
    let monitor_path = format!(r"{DEFAULT_ROOT_KEY}\Layouts\{LAYOUT_ID}\PhysicalMonitors\M1");
    {
        let monitor = tree.create(&monitor_path);
        monitor.set_string("XLocationInMm", "1,5"); // a comma decimal: unreadable
        monitor.set_string("YLocationInMm", " 2.5e1 ");
    }
    // A pre-split VALUE wins over the subkey of the same name.
    tree.create(&format!(r"{monitor_path}\BorderResistance"))
        .set_string("Left", "15");
    tree.create(&format!(r"{monitor_path}\BorderResistance\Left"))
        .set_string("Move", "99");
    // Sections come in index order, whatever the registry's alphabetical order.
    for (name, from) in [
        ("10", "10"),
        ("2", "2"),
        ("0", "0"),
        ("x", "-1"),
        ("1", "1"),
    ] {
        tree.create(&format!(
            r"{monitor_path}\BorderResistance\Top\Sections\{name}"
        ))
        .set_string("From", from);
    }
    // Borders without Left: the monitor does not own its borders.
    tree.create(&format!(r"{monitor_path}\Borders"))
        .set_string("Top", "3");
    tree.create(&format!(r"{monitor_path}\DISPLAY2"))
        .set_string("Orientation", "3");

    let root = (&tree).open(DEFAULT_ROOT_KEY).unwrap();
    let data = registry::read(Some(&root), LAYOUT_ID, &[PNP]);

    let global = data.global_options.unwrap();
    assert_eq!(global.show_monitor_action_warning, Some(false));
    assert_eq!(global.debug_tools, None);
    assert_eq!(global.excluded_defaults_version, None);
    assert_eq!(global.priority.as_deref(), Some("BelowNormal"));
    assert_eq!(global.home_cinema, Some(true));
    assert_eq!(global.pinned, Some(false));
    assert_eq!(global.auto_update, None);

    let layout = data.layout.unwrap();
    assert_eq!(
        layout.options.as_ref().unwrap().priority.as_deref(),
        Some("BelowNormal")
    );
    let monitor = &layout.monitors["M1"];
    assert_eq!(monitor.x_location_in_mm, None);
    assert_eq!(monitor.y_location_in_mm, Some(25.0));
    assert_eq!(monitor.borders, None);
    let resistance = monitor.border_resistance.as_ref().unwrap();
    let left = resistance.left.as_ref().unwrap();
    assert_eq!(
        (left.r#move, left.drag, &left.sections),
        (Some(15.0), Some(15.0), &None)
    );
    let froms: Vec<Option<f64>> = resistance
        .top
        .as_ref()
        .unwrap()
        .sections
        .as_ref()
        .unwrap()
        .iter()
        .map(|s| s.from)
        .collect();
    assert_eq!(
        froms,
        [Some(0.0), Some(1.0), Some(2.0), Some(10.0), Some(-1.0)]
    );
    assert_eq!(resistance.right, None);
    let sources = monitor.sources.as_ref().unwrap();
    assert_eq!(sources.keys().collect::<Vec<_>>(), ["DISPLAY2"]);
    assert_eq!(sources["DISPLAY2"].orientation, Some(3));

    // No model stored: none read, and the registry is not consulted for others.
    assert!(data.models.is_empty());
}

/// C# `LayoutStoreKeyTests.MonitorId`-style ids: 29 characters each, nine of them make
/// a layout id too long for a registry key name (#589).
fn nine_monitor_layout(tree: &mut MemoryKey, key: &str, turned: usize) {
    for i in 1..=9 {
        let monitor = format!(
            r"{DEFAULT_ROOT_KEY}\Layouts\{key}\PhysicalMonitors\{}",
            common::monitor_id(i)
        );
        tree.create(&monitor).set_string("ActiveSource", "S");
        let orientation = if i == turned { "1" } else { "0" };
        tree.create(&format!(r"{monitor}\S"))
            .set_string("Orientation", orientation);
    }
}

#[test]
fn layouts_stored_under_a_digest_import_under_their_rebuilt_id() {
    // What ComputeId gave: the ids in invariant order (here their numeric order), the
    // turned monitor suffixed with its quarter turns.
    let id: String = (1..=9)
        .map(|i| match i {
            4 => format!("{}_1", common::monitor_id(i)),
            _ => common::monitor_id(i),
        })
        .collect::<Vec<_>>()
        .join("+");
    let key = key_for(&id);
    assert_ne!(key, id, "the id must be too long for a key name");

    // A digest-shaped name nothing rebuilds.
    let orphan = format!("{}~{}", "X".repeat(190), "0".repeat(64));

    let mut tree = MemoryKey::new("");
    nine_monitor_layout(&mut tree, &key, 4);
    nine_monitor_layout(&mut tree, &orphan, 0);
    let root = (&tree).open(DEFAULT_ROOT_KEY).unwrap();

    let work = tempfile::tempdir().unwrap();
    let store = JsonLayoutStore::new(work.path());
    let report = import_registry(&root, &id, &store).unwrap();

    assert_eq!(report.layouts, std::slice::from_ref(&id));
    assert_eq!(report.skipped.len(), 1);
    assert_eq!(report.skipped[0].0, orphan);

    // Found again by its id, under the JSON store's own (shorter) name for it.
    let layout = store.read(&id, &[]).unwrap().layout.expect("imported");
    assert_eq!(layout.monitors.len(), 9);
    assert_ne!(store.layout_path(&id).file_stem().unwrap(), key.as_str());
}

#[test]
fn an_empty_registry_imports_nothing_but_empty_options() {
    let mut tree = MemoryKey::new("");
    tree.create(DEFAULT_ROOT_KEY);
    let root = (&tree).open(DEFAULT_ROOT_KEY).unwrap();

    let work = tempfile::tempdir().unwrap();
    let report = import_registry(&root, LAYOUT_ID, &JsonLayoutStore::new(work.path())).unwrap();

    assert_eq!(report, Default::default());
    let files: Vec<_> = fs::read_dir(work.path())
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(files, ["options.json"]);
    assert_eq!(
        fs::read_to_string(work.path().join("options.json")).unwrap(),
        "{}"
    );
}

/// The same import through the real registry: the fixture is written under a throwaway
/// key next to the real one, imported from there and from the export, and both JSON
/// stores must hold the same bytes.
#[cfg(windows)]
mod real_registry {
    use std::fs;
    use std::path::Path;

    use windows::core::PCWSTR;
    use windows::Win32::Foundation::ERROR_SUCCESS;
    use windows::Win32::System::Registry::{
        RegCloseKey, RegCreateKeyExW, RegDeleteTreeW, RegSetValueExW, HKEY, HKEY_CURRENT_USER,
        KEY_ALL_ACCESS, REG_OPTION_NON_VOLATILE, REG_SZ,
    };

    use lbm_store::reg_file::{MemoryKey, RegValue};
    use lbm_store::registry_import::import_registry;
    use lbm_store::windows_registry::WindowsKey;
    use lbm_store::JsonLayoutStore;

    use super::{lbm_root, registry_fixture, LAYOUT_ID};

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(Some(0)).collect()
    }

    /// Deletes the throwaway tree, even when the test fails.
    struct TestTree(String);

    impl Drop for TestTree {
        fn drop(&mut self) {
            let path = wide(&self.0);
            unsafe {
                let _ = RegDeleteTreeW(HKEY_CURRENT_USER, PCWSTR(path.as_ptr()));
            }
        }
    }

    fn create(parent: HKEY, name: &str) -> HKEY {
        let name = wide(name);
        let mut key = HKEY::default();
        let status = unsafe {
            RegCreateKeyExW(
                parent,
                PCWSTR(name.as_ptr()),
                0,
                PCWSTR::null(),
                REG_OPTION_NON_VOLATILE,
                KEY_ALL_ACCESS,
                None,
                &mut key,
                None,
            )
        };
        assert_eq!(status, ERROR_SUCCESS);
        key
    }

    fn write(key: HKEY, tree: &MemoryKey) {
        for (name, value) in tree.values() {
            let RegValue::String(text) = value else {
                continue;
            };
            let name = wide(name);
            let data: Vec<u8> = wide(text).iter().flat_map(|u| u.to_le_bytes()).collect();
            let status =
                unsafe { RegSetValueExW(key, PCWSTR(name.as_ptr()), 0, REG_SZ, Some(&data)) };
            assert_eq!(status, ERROR_SUCCESS);
        }
        for sub in tree.subkeys() {
            let child = create(key, sub.name());
            write(child, sub);
            unsafe {
                let _ = RegCloseKey(child);
            }
        }
    }

    fn files(dir: &Path) -> Vec<(String, Vec<u8>)> {
        let mut out = Vec::new();
        for entry in walk(dir) {
            let relative = entry
                .strip_prefix(dir)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            out.push((relative, fs::read(&entry).unwrap()));
        }
        out.sort();
        out
    }

    fn walk(dir: &Path) -> Vec<std::path::PathBuf> {
        let mut out = Vec::new();
        for entry in fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                out.extend(walk(&path));
            } else {
                out.push(path);
            }
        }
        out
    }

    #[test]
    fn the_real_registry_imports_like_its_export() {
        let unique = format!(
            "{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        // Next to the real key rather than somewhere else in HKCU: a leaked test tree is
        // obvious, and deleting it can never reach the user's own settings.
        let path = format!(r"SOFTWARE\Mgth\LittleBigMouse-Tests\{unique}");
        let _cleanup = TestTree(path.clone());

        let tree = registry_fixture("v5.6-current.reg");
        let exported = lbm_root(&tree);
        let key = create(HKEY_CURRENT_USER, &path);
        write(key, exported);
        unsafe {
            let _ = RegCloseKey(key);
        }

        let real = WindowsKey::open_current_user(&path).expect("just written");
        let from_registry = tempfile::tempdir().unwrap();
        let from_export = tempfile::tempdir().unwrap();
        let a = import_registry(
            &real,
            LAYOUT_ID,
            &JsonLayoutStore::new(from_registry.path()),
        )
        .unwrap();
        let b = import_registry(
            &exported,
            LAYOUT_ID,
            &JsonLayoutStore::new(from_export.path()),
        )
        .unwrap();

        assert_eq!(a, b);
        assert_eq!(files(from_registry.path()), files(from_export.path()));
    }
}
