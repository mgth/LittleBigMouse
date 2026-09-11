//! Port of `JsonLayoutStoreTests.cs`, one test per C# test, same names.
//!
//! Locks the on-disk JSON format of the store: the literal documents here are the files
//! written by the pre-refactor `LinuxLayoutPersistence`, so a DTO rename or a
//! serializer change that would orphan existing user configs fails these tests.

mod common;

use std::fs;

use indexmap::IndexMap;
use lbm_store::{
    BorderResistanceDto, BorderSectionDto, BorderSideDto, GlobalOptionsDto, JsonLayoutStore,
    LayoutDto, LayoutOptionsDto, LayoutStore, ModelDto, MonitorDto,
};
use tempfile::TempDir;

/// C# `_dir`: a fresh directory per test, deleted afterwards.
fn temp_dir() -> TempDir {
    tempfile::Builder::new()
        .prefix("lbm-json-store-tests")
        .tempdir()
        .unwrap()
}

fn enabled_layout() -> LayoutDto {
    LayoutDto {
        options: Some(LayoutOptionsDto {
            enabled: Some(true),
            ..LayoutOptionsDto::default()
        }),
        ..LayoutDto::default()
    }
}

#[test]
fn read_current_on_disk_format() {
    let dir = temp_dir();
    fs::create_dir_all(dir.path().join("layouts")).unwrap();
    fs::write(
        dir.path().join("options.json"),
        r#"{
  "DaemonPort": 25196,
  "Priority": "Normal",
  "HomeCinema": false,
  "ShowMonitorActionWarning": true,
  "BorderValues": "PerModel"
}"#,
    )
    .unwrap();
    fs::write(
        dir.path().join("models.json"),
        r#"{
  "TST1234": {
    "Width": 600,
    "Height": 340,
    "Borders": { "Left": 10, "Top": 11, "Right": 12, "Bottom": 13 },
    "PnpName": "Test monitor"
  }
}"#,
    )
    .unwrap();
    fs::write(
        dir.path().join("layouts").join("LAYOUT1.json"),
        r#"{
  "Options": { "Enabled": true, "Algorithm": "Strait", "MaxTravelDistance": 200 },
  "Monitors": {
    "MON1": {
      "XLocationInMm": 12.5,
      "YLocationInMm": -3,
      "PhysicalRatioX": 1,
      "BorderResistance": { "Left": 1, "Top": 2, "Right": 3, "Bottom": 4 },
      "Borders": { "Left": 5, "Top": 6, "Right": 7, "Bottom": 8 },
      "ActiveSource": "SRC1",
      "ExcludedFromLayout": false,
      "Sources": {
        "SRC1": { "PixelX": 0, "PixelY": 0, "PixelWidth": 1920, "PixelHeight": 1080, "Orientation": 0, "Primary": true }
      }
    }
  }
}"#,
    )
    .unwrap();

    let store = JsonLayoutStore::new(dir.path());
    let data = store.read("LAYOUT1", &["TST1234"]).unwrap();

    // "DaemonPort" is still in the document above on purpose: the TCP transport it
    // configured is gone, so it is now an unknown property, and a user's existing
    // options.json must keep reading cleanly rather than throwing.
    let options = data.global_options.as_ref().unwrap();
    assert_eq!(options.priority.as_deref(), Some("Normal"));
    assert_eq!(options.show_monitor_action_warning, Some(true));

    let model = &data.models["TST1234"];
    assert_eq!(model.width, Some(600.0));
    assert_eq!(model.borders.as_ref().unwrap().bottom, Some(13.0));
    assert_eq!(model.pnp_name.as_deref(), Some("Test monitor"));

    let layout = data.layout.as_ref().unwrap();
    let layout_options = layout.options.as_ref().unwrap();
    assert_eq!(layout_options.enabled, Some(true));
    assert_eq!(layout_options.max_travel_distance, Some(200.0));

    let monitor = &layout.monitors["MON1"];
    assert_eq!(monitor.x_location_in_mm, Some(12.5));

    // Pre-move/drag-split shape: one number per edge. It must still load, and the
    // single value governs both modes — that is what it always meant. Reading it as
    // the current object shape would fail, and the store turns failures into "absent",
    // so the whole layout would silently reset.
    let bottom = monitor
        .border_resistance
        .as_ref()
        .unwrap()
        .bottom
        .as_ref()
        .unwrap();
    assert_eq!(bottom.r#move, Some(4.0));
    assert_eq!(bottom.drag, Some(4.0));
    assert_eq!(bottom.sections, None);

    assert_eq!(monitor.borders.as_ref().unwrap().left, Some(5.0));
    assert_eq!(monitor.active_source.as_deref(), Some("SRC1"));
    let source = &monitor.sources.as_ref().unwrap()["SRC1"];
    assert_eq!(source.primary, Some(true));
    assert_eq!(source.pixel_width, Some(1920.0));
}

#[test]
fn read_border_resistance_with_sections() {
    let dir = temp_dir();
    fs::create_dir_all(dir.path().join("layouts")).unwrap();
    fs::write(
        dir.path().join("layouts").join("LAYOUT1.json"),
        r#"{
  "Monitors": {
    "MON1": {
      "BorderResistance": {
        "Right": {
          "Move": 1.5,
          "Drag": 20,
          "DragBlock": true,
          "Sections": [
            { "From": 0, "To": 100, "Move": 0, "MoveBlock": true, "Drag": 3 }
          ]
        }
      }
    }
  }
}"#,
    )
    .unwrap();

    let layout = JsonLayoutStore::new(dir.path())
        .read("LAYOUT1", &[])
        .unwrap()
        .layout
        .unwrap();
    let side = layout.monitors["MON1"]
        .border_resistance
        .as_ref()
        .unwrap()
        .right
        .as_ref()
        .unwrap();

    assert_eq!(side.r#move, Some(1.5));
    assert_eq!(side.drag, Some(20.0));
    assert_eq!(side.drag_block, Some(true));
    assert_eq!(side.move_block, None);

    let sections = side.sections.as_ref().unwrap();
    assert_eq!(sections.len(), 1);
    let section = &sections[0];
    assert_eq!(section.to, Some(100.0));
    assert_eq!(section.move_block, Some(true));
    assert_eq!(section.drag, Some(3.0));
}

#[test]
fn write_then_read_border_resistance_round_trips() {
    let dir = temp_dir();
    let store = JsonLayoutStore::new(dir.path());

    let mut layout = LayoutDto::default();
    layout.monitors.insert(
        "M1".into(),
        MonitorDto {
            border_resistance: Some(BorderResistanceDto {
                left: Some(BorderSideDto {
                    r#move: Some(2.0),
                    drag: Some(30.0),
                    move_block: Some(true),
                    sections: Some(vec![BorderSectionDto {
                        from: Some(5.0),
                        to: Some(15.0),
                        drag: Some(40.0),
                        drag_block: Some(true),
                        ..BorderSectionDto::default()
                    }]),
                    ..BorderSideDto::default()
                }),
                ..BorderResistanceDto::default()
            }),
            ..MonitorDto::default()
        },
    );
    store.write_layout("L1", &layout).unwrap();

    let read = store.read("L1", &[]).unwrap().layout.unwrap();
    let side = read.monitors["M1"]
        .border_resistance
        .as_ref()
        .unwrap()
        .left
        .as_ref()
        .unwrap();

    assert_eq!(side.r#move, Some(2.0));
    assert_eq!(side.drag, Some(30.0));
    assert_eq!(side.move_block, Some(true));

    let sections = side.sections.as_ref().unwrap();
    assert_eq!(sections.len(), 1);
    let section = &sections[0];
    assert_eq!(section.from, Some(5.0));
    assert_eq!(section.to, Some(15.0));
    assert_eq!(section.drag, Some(40.0));
    assert_eq!(section.drag_block, Some(true));
}

#[test]
fn write_then_read_round_trips() {
    let dir = temp_dir();
    let store = JsonLayoutStore::new(dir.path());

    store
        .write_global_options(&GlobalOptionsDto {
            pinned: Some(true),
            excluded_defaults_version: Some(1),
            ..GlobalOptionsDto::default()
        })
        .unwrap();
    let mut layout = enabled_layout();
    layout.monitors.insert(
        "M1".into(),
        MonitorDto {
            x_location_in_mm: Some(7.25),
            ..MonitorDto::default()
        },
    );
    store.write_layout("L1", &layout).unwrap();

    let data = store.read("L1", &[]).unwrap();

    let options = data.global_options.unwrap();
    assert_eq!(options.pinned, Some(true));
    assert_eq!(options.excluded_defaults_version, Some(1));
    let layout = data.layout.unwrap();
    assert_eq!(layout.options.unwrap().enabled, Some(true));
    assert_eq!(layout.monitors["M1"].x_location_in_mm, Some(7.25));
}

#[test]
fn write_then_read_nine_monitor_id_round_trips() {
    // #589: nine monitor ids join into a name longer than a file name may be (255
    // bytes). The C# WriteJson swallowed the IOException, so this silently stored
    // nothing.
    let id = common::layout_id(9);
    assert!(id.len() > 255);

    let dir = temp_dir();
    let store = JsonLayoutStore::new(dir.path());
    store.write_layout(&id, &enabled_layout()).unwrap();

    let layout = store.read(&id, &[]).unwrap().layout.unwrap();
    assert_eq!(layout.options.unwrap().enabled, Some(true));
}

#[test]
fn write_models_upserts_without_dropping_others() {
    let dir = temp_dir();
    let store = JsonLayoutStore::new(dir.path());

    let model = |width: f64| ModelDto {
        width: Some(width),
        ..ModelDto::default()
    };
    store
        .write_models(&IndexMap::from([("A".to_owned(), model(1.0))]))
        .unwrap();
    store
        .write_models(&IndexMap::from([("B".to_owned(), model(2.0))]))
        .unwrap();

    let data = store.read("x", &["A", "B"]).unwrap();
    assert_eq!(data.models["A"].width, Some(1.0));
    assert_eq!(data.models["B"].width, Some(2.0));
}

#[test]
fn read_corrupt_file_falls_back_to_defaults() {
    let dir = temp_dir();
    fs::write(dir.path().join("options.json"), "{ this is not json").unwrap();

    let store = JsonLayoutStore::new(dir.path());
    assert_eq!(store.read("x", &[]).unwrap().global_options, None);
}

#[test]
fn layout_id_is_sanitized_for_the_file_system() {
    let dir = temp_dir();
    let store = JsonLayoutStore::new(dir.path());
    store.write_layout("A/B", &enabled_layout()).unwrap();

    assert!(dir.path().join("layouts").join("A_B.json").is_file());
    let layout = store.read("A/B", &[]).unwrap().layout.unwrap();
    assert_eq!(layout.options.unwrap().enabled, Some(true));
}
