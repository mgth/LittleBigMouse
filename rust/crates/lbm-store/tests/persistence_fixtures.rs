//! DTO-level tests over the C# golden fixtures (`TestData/Persistence/*/`), read from
//! the source tree. Not a port: the C# suite exercises these files through the mapper
//! (`LayoutPersistenceGoldenTests`), which lives above this crate.
//!
//! What they lock here: every document any supported version wrote parses; a read →
//! write → read changes nothing; the files the C# writer produced (`*-saved`) come
//! back byte for byte; the pre-5.5 bare-number edges read as `{Move, Drag}`; unknown
//! members survive.

mod common;

use std::fmt::Debug;
use std::fs;
use std::path::{Path, PathBuf};

use indexmap::IndexMap;
use lbm_store::json_format::{from_slice, to_string};
use lbm_store::{GlobalOptionsDto, JsonLayoutStore, LayoutDto, LayoutStore, ModelDto};
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::{json, Value};

/// The layout id the fixtures are stored under.
const LAYOUT_ID: &str = "TST1234_1920x1080";

type Models = IndexMap<String, ModelDto>;

/// Which DTO a store file holds.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Kind {
    Options,
    Models,
    Layout,
}

/// Every file of every fixture directory, with the DTO it holds.
fn fixture_files() -> Vec<(PathBuf, Kind)> {
    let mut files = Vec::new();
    for dir in sorted_entries(&common::persistence_fixtures()) {
        for path in sorted_entries(&dir) {
            match path.file_name().and_then(|n| n.to_str()) {
                Some("options.json") => files.push((path, Kind::Options)),
                Some("models.json") => files.push((path, Kind::Models)),
                Some("layouts") => {
                    files.extend(sorted_entries(&path).into_iter().map(|p| (p, Kind::Layout)))
                }
                _ => panic!("unexpected fixture file {}", path.display()),
            }
        }
    }
    files
}

fn sorted_entries(dir: &Path) -> Vec<PathBuf> {
    let mut entries: Vec<PathBuf> = fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("cannot list {}: {e}", dir.display()))
        .map(|entry| entry.unwrap().path())
        .collect();
    entries.sort();
    entries
}

fn parse<T: DeserializeOwned>(path: &Path) -> T {
    from_slice(&fs::read(path).unwrap())
        .unwrap_or_else(|e| panic!("{} does not parse: {e}", path.display()))
}

fn relative(path: &Path) -> String {
    path.strip_prefix(common::persistence_fixtures())
        .unwrap()
        .display()
        .to_string()
}

/// read → write → read, asserting the written form is also a fixed point.
fn round_trip<T: Serialize + DeserializeOwned + Debug>(path: &Path) -> (T, T) {
    let read: T = parse(path);
    let written = to_string(&read).unwrap();
    let reread: T = from_slice(written.as_bytes()).unwrap();
    assert_eq!(
        to_string(&reread).unwrap(),
        written,
        "{}: a second write differs",
        relative(path)
    );
    (read, reread)
}

#[test]
fn every_fixture_file_parses() {
    let files = fixture_files();
    assert_eq!(files.len(), 20, "fixture set changed: {files:#?}");

    for (path, kind) in &files {
        match kind {
            Kind::Options => drop(parse::<GlobalOptionsDto>(path)),
            Kind::Models => drop(parse::<Models>(path)),
            Kind::Layout => drop(parse::<LayoutDto>(path)),
        }
    }

    // And through the store, which turns a parse failure into "absent".
    for dir in sorted_entries(&common::persistence_fixtures()) {
        let data = JsonLayoutStore::new(&dir)
            .read(LAYOUT_ID, &["TST1234"])
            .unwrap();
        let name = dir.file_name().unwrap().to_string_lossy();
        assert_eq!(
            data.global_options.is_some(),
            dir.join("options.json").exists(),
            "{name}"
        );
        assert!(data.layout.is_some(), "{name}");
        assert!(data.models.contains_key("TST1234") || name == "incomplete-empty");
    }
}

#[test]
fn read_write_read_is_the_identity() {
    for (path, kind) in fixture_files() {
        let name = relative(&path);
        match kind {
            Kind::Options => {
                let (read, reread) = round_trip::<GlobalOptionsDto>(&path);
                assert_eq!(read, reread, "{name}");
            }
            Kind::Models => {
                let (read, reread) = round_trip::<Models>(&path);
                assert_eq!(read, reread, "{name}");
            }
            Kind::Layout => {
                let (mut read, reread) = round_trip::<LayoutDto>(&path);
                if name.starts_with("incomplete-partial") {
                    // The one change a round trip makes, faithful to the C# converter:
                    // an empty section list is not written, so `"Sections": []` comes
                    // back absent. Both mean "no section" to the mapper.
                    let right = read.monitors["TST1234_0"]
                        .border_resistance
                        .as_mut()
                        .and_then(|r| r.right.as_mut())
                        .unwrap();
                    assert_eq!(right.sections, Some(Vec::new()));
                    right.sections = None;
                }
                assert_eq!(read, reread, "{name}");
            }
        }
    }
}

#[test]
fn csharp_written_files_are_reproduced_byte_for_byte() {
    // The `*-saved` fixtures are what the C# store wrote (LayoutPersistenceGoldenTests
    // regenerates them); writing what they parse to must give the same bytes back —
    // the `+` escapes of RescueShortcut included.
    let saved: Vec<_> = fixture_files()
        .into_iter()
        .filter(|(path, _)| relative(path).contains("-saved"))
        .collect();
    assert_eq!(saved.len(), 5);

    for (path, kind) in saved {
        let written = match kind {
            Kind::Options => to_string(&parse::<GlobalOptionsDto>(&path)),
            Kind::Models => to_string(&parse::<Models>(&path)),
            Kind::Layout => to_string(&parse::<LayoutDto>(&path)),
        }
        .unwrap();
        assert_eq!(
            written,
            common::read_normalized(&path),
            "{}",
            relative(&path)
        );
    }
}

#[test]
fn legacy_bare_number_sides_read_as_move_and_drag() {
    let path = common::fixture("v5.2-pre-sections")
        .join("layouts")
        .join(format!("{LAYOUT_ID}.json"));
    let layout: LayoutDto = parse(&path);
    let resistance = layout.monitors["TST1234_0"]
        .border_resistance
        .as_ref()
        .unwrap();

    for (side, value) in [
        (&resistance.left, 20.0),
        (&resistance.top, 0.0),
        (&resistance.right, 20.0),
        (&resistance.bottom, 0.0),
    ] {
        let side = side.as_ref().unwrap();
        assert_eq!(side.r#move, Some(value));
        assert_eq!(side.drag, Some(value));
        assert_eq!(side.move_block, None);
        assert_eq!(side.drag_block, None);
        assert_eq!(side.sections, None);
    }

    // Written back in the current shape, never as a bare number.
    let written: Value = serde_json::from_str(&to_string(&layout).unwrap()).unwrap();
    assert_eq!(
        written["Monitors"]["TST1234_0"]["BorderResistance"],
        json!({
            "Left": { "Move": 20, "Drag": 20 },
            "Top": { "Move": 0, "Drag": 0 },
            "Right": { "Move": 20, "Drag": 20 },
            "Bottom": { "Move": 0, "Drag": 0 }
        })
    );
}

#[test]
fn unknown_members_of_the_fixtures_survive_a_round_trip() {
    // DaemonPort: the removed TCP transport's port, still in every pre-5.6 options.json.
    for fixture in ["v5.2-pre-sections", "v5.5-sections", "v5.6-current"] {
        let options: GlobalOptionsDto = parse(&common::fixture(fixture).join("options.json"));
        assert_eq!(options.extra.get("DaemonPort"), Some(&json!(25196)));
        let written = to_string(&options).unwrap();
        assert!(written.contains("\n  \"DaemonPort\": 25196"), "{written}");
    }

    let dir = common::fixture("incomplete-partial");
    let options: GlobalOptionsDto = parse(&dir.join("options.json"));
    assert_eq!(options.priority, None, "an explicit null is absent");
    assert_eq!(
        to_string(&options).unwrap(),
        "{\n  \"BorderValues\": \"PerMonitor\",\n  \"UnknownFutureOption\": \"ignored\"\n}"
    );

    let layout: LayoutDto = parse(&dir.join("layouts").join(format!("{LAYOUT_ID}.json")));
    let monitor = &layout.monitors["TST1234_0"];
    assert_eq!(monitor.extra.get("UnknownFutureField"), Some(&json!(42)));
    let written: Value = serde_json::from_str(&to_string(&layout).unwrap()).unwrap();
    assert_eq!(
        written["Monitors"]["TST1234_0"]["UnknownFutureField"],
        json!(42)
    );
}

#[test]
fn unknown_members_survive_at_every_level_in_order() {
    // Written in the writer's own layout — known members in the C# order, unknown ones
    // after them in file order — so one byte comparison checks presence and order.
    let layout = r#"{
  "Options": {
    "Enabled": true,
    "OptZ": 1,
    "OptA": [
      1,
      "x"
    ]
  },
  "Monitors": {
    "M2": {
      "XLocationInMm": 1.5,
      "BorderResistance": {
        "Left": {
          "Move": 2,
          "Sections": [
            {
              "From": 0,
              "To": 10,
              "SecZ": null
            }
          ],
          "SideZ": {
            "Deep": {
              "Deeper": true
            }
          }
        },
        "ResZ": "r"
      },
      "Borders": {
        "Left": 5,
        "BordZ": -1
      },
      "Sources": {
        "S": {
          "PixelX": 0,
          "SrcZ": 0.25
        }
      },
      "MonZ": "m"
    },
    "M1": {}
  },
  "TopZ": {},
  "TopA": []
}"#;
    let dto: LayoutDto = from_slice(layout.as_bytes()).unwrap();
    assert_eq!(to_string(&dto).unwrap(), layout);
    assert_eq!(
        dto.monitors.keys().collect::<Vec<_>>(),
        ["M2", "M1"],
        "monitors keep the file order"
    );

    let models = r#"{
  "B": {
    "Width": 1,
    "Borders": {
      "Top": 2,
      "BordZ": 3
    },
    "ModZ": "z"
  },
  "A": {}
}"#;
    let dto: Models = from_slice(models.as_bytes()).unwrap();
    assert_eq!(to_string(&dto).unwrap(), models);
}

#[test]
fn a_read_modify_write_through_the_store_keeps_unknown_members() {
    let work = tempfile::tempdir().unwrap();
    let dir = common::fixture_copy("incomplete-partial", work.path());
    let store = JsonLayoutStore::new(&dir);

    // What C# SaveEnabled does: read the document, change one option, write it back.
    let mut layout = store.read(LAYOUT_ID, &[]).unwrap().layout.unwrap();
    layout.options.get_or_insert_with(Default::default).enabled = Some(true);
    store.write_layout(LAYOUT_ID, &layout).unwrap();

    let reread = store.read(LAYOUT_ID, &[]).unwrap().layout.unwrap();
    assert_eq!(reread.options.unwrap().enabled, Some(true));
    assert_eq!(
        reread.monitors["TST1234_0"].extra.get("UnknownFutureField"),
        Some(&json!(42))
    );

    // Models the upsert does not name are rewritten from their stored document,
    // unknown members included.
    let models_path = dir.join("models.json");
    let with_future = fs::read_to_string(&models_path)
        .unwrap()
        .replace("\"Width\": 700,", "\"Width\": 700,\n    \"Future\": [1],");
    fs::write(&models_path, with_future).unwrap();
    store
        .write_models(&IndexMap::from([("NEW1".to_owned(), ModelDto::default())]))
        .unwrap();
    let models = store.read(LAYOUT_ID, &[]).unwrap().models;
    assert_eq!(
        models.keys().collect::<Vec<_>>(),
        ["TST1234", "OTHER99", "NEW1"]
    );
    assert_eq!(models["OTHER99"].extra.get("Future"), Some(&json!([1])));
}
