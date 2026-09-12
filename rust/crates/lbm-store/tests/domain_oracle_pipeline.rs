//! The domain oracle (`domain-oracle/` at the repository root), end to end: every
//! scenario goes through the Rust pipeline — the Linux outputs mapped into an
//! `lbm-layout` model, the store loaded by the persistence engine, the monitors placed
//! and anchored, then a save — and must produce what the C# recorded (`OracleRun`):
//! `zones.xml` byte for byte; `layout.json`, `pixel-locations.json` and
//! `saved-store.json` value for value, doubles bit for bit.
//!
//! Like `OracleRun`, the store is a JSON store in a scratch directory seeded from
//! `input.json`, the excluded-processes file lives there too, and the process counts
//! as not elevated.

mod common;

use std::fs;
use std::path::{Path, PathBuf};

use lbm_layout::geo::{Point, Rect};
use lbm_layout::linux::{populate, LinuxEdid, LinuxMonitor};
use lbm_layout::model::{
    BorderSection, BorderSide, DisplaySize, Layout, LayoutOptions, Monitor, PhysicalSource, Ratio,
};
use lbm_layout::zoning::compute_zones;
use lbm_store::layout_store_key::key_for;
use lbm_store::{JsonLayoutStore, LayoutDocument, LayoutPersistence, PersistencePlatform};
use serde_json::{json, Map, Value};

fn read(path: &Path) -> String {
    std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()))
        .replace("\r\n", "\n")
}

//==================//
// Input            //
//==================//

fn string(v: &Value) -> Option<String> {
    v.as_str().map(str::to_owned)
}

fn linux_monitor(v: &Value) -> LinuxMonitor {
    let f = |k: &str| v[k].as_f64().unwrap_or_else(|| panic!("{k}"));
    let i = |k: &str| v[k].as_i64().unwrap_or_else(|| panic!("{k}")) as i32;
    let b = |k: &str| v[k].as_bool().unwrap_or_else(|| panic!("{k}"));
    let edid = match &v["Edid"] {
        Value::Null => None,
        e => Some(LinuxEdid {
            manufacturer_code: string(&e["ManufacturerCode"]),
            product_code: string(&e["ProductCode"]),
            serial: string(&e["Serial"]),
            serial_number: string(&e["SerialNumber"]),
            model: string(&e["Model"]),
            physical_width: e["PhysicalWidth"].as_f64().unwrap(),
            physical_height: e["PhysicalHeight"].as_f64().unwrap(),
            video_interface: string(&e["VideoInterface"]),
        }),
    };
    LinuxMonitor {
        connector_name: v["ConnectorName"].as_str().unwrap().to_owned(),
        logical_x: f("LogicalX"),
        logical_y: f("LogicalY"),
        logical_width: f("LogicalWidth"),
        logical_height: f("LogicalHeight"),
        pixel_width: i("PixelWidth"),
        pixel_height: i("PixelHeight"),
        scale: f("Scale"),
        width_mm: f("WidthMm"),
        height_mm: f("HeightMm"),
        primary: b("Primary"),
        enabled: b("Enabled"),
        orientation: i("Orientation"),
        frequency: i("Frequency"),
        edid,
    }
}

//==================//
// Output           //
//==================//

/// `OracleRun.Number`: a JSON number, or the string .NET spells a NaN or an
/// infinity with.
fn number(x: f64) -> Value {
    if x.is_nan() {
        json!("NaN")
    } else if x == f64::INFINITY {
        json!("Infinity")
    } else if x == f64::NEG_INFINITY {
        json!("-Infinity")
    } else {
        json!(x)
    }
}

fn rect(r: Rect) -> Value {
    json!({ "X": number(r.x()), "Y": number(r.y()), "Width": number(r.width()), "Height": number(r.height()) })
}

fn size(s: &DisplaySize) -> Value {
    json!({
        "X": number(s.x), "Y": number(s.y), "Width": number(s.width), "Height": number(s.height),
        "LeftBorder": number(s.left_border), "TopBorder": number(s.top_border),
        "RightBorder": number(s.right_border), "BottomBorder": number(s.bottom_border),
    })
}

fn projection(s: &DisplaySize) -> Value {
    let mut v = size(s);
    v["Bounds"] = rect(s.bounds());
    v["OutsideBounds"] = rect(s.outside_bounds());
    v
}

fn ratio(r: Option<Ratio>) -> Value {
    match r {
        Some(r) => json!({ "X": number(r.x), "Y": number(r.y) }),
        None => Value::Null,
    }
}

fn sections(side: &BorderSide) -> Value {
    Value::Array(
        side.sections
            .iter()
            .map(|s| {
                json!({
                    "From": number(s.from()), "To": number(s.to()),
                    "Move": number(s.move_resistance()), "MoveBlock": s.move_block(),
                    "Drag": number(s.drag()), "DragBlock": s.drag_block(),
                })
            })
            .collect(),
    )
}

fn source(layout: &Layout, s: &PhysicalSource) -> Value {
    let d = &s.source;
    json!({
        "Id": d.id,
        "DeviceId": s.device_id,
        "DeviceName": d.device_name,
        "DisplayName": d.display_name,
        "SourceName": d.source_name,
        "InterfacePath": d.interface_path,
        "SourceNumber": d.source_number,
        "Primary": d.primary,
        "AttachedToDesktop": d.attached_to_desktop,
        "Orientation": d.orientation,
        "DisplayFrequency": d.display_frequency,
        "InPixel": rect(d.in_pixel.bounds()),
        "EffectiveDpi": ratio(Some(d.effective_dpi)),
        "DpiAwareAngularDpi": ratio(Some(d.dpi_aware_angular_dpi)),
        "RawDpi": ratio(Some(d.raw_dpi)),
        "InDip": rect(layout.in_dip(s).bounds()),
        "RealPitch": ratio(layout.real_pitch(s)),
        "Pitch": ratio(layout.pitch(s)),
        "RealDpi": ratio(layout.real_dpi(s)),
        "Dpi": ratio(layout.dpi(s)),
        "DipToPixelRatio": ratio(layout.dip_to_pixel_ratio(s)),
        "PixelToDipRatio": ratio(Some(layout.pixel_to_dip_ratio(s))),
        "PhysicalToPixelRatio": ratio(layout.physical_to_pixel_ratio(s)),
        "MmToDipRatio": ratio(layout.mm_to_dip_ratio(s)),
    })
}

fn monitor(layout: &Layout, m: &Monitor) -> Value {
    let model = layout.model(&m.model).unwrap();
    let borders = layout.monitor_borders(m);
    let mut sources: Vec<&PhysicalSource> = m
        .sources
        .iter()
        .filter_map(|id| layout.source(id))
        .collect();
    sources.sort_by(|a, b| a.source.id.cmp(&b.source.id));
    json!({
        "Id": m.id,
        "DeviceId": m.device_id,
        "SerialNumber": m.serial_number,
        "Model": {
            "PnpCode": model.pnp_code,
            "PnpDeviceName": model.pnp_device_name,
            "Logo": model.logo,
            "PhysicalSize": size(&model.physical_size.as_display_size()),
        },
        "Placed": m.placed,
        "ExcludedFromLayout": m.excluded_from_layout,
        "BordersCustomized": m.borders_customized(),
        "Borders": {
            "Left": number(borders.left), "Top": number(borders.top),
            "Right": number(borders.right), "Bottom": number(borders.bottom),
        },
        "EffectivePhysicalSize": size(&layout.effective_physical_size(m)),
        "PhysicalRotated": layout.physical_rotated(m).map_or(Value::Null, |s| size(&s)),
        "DepthRatio": ratio(Some(m.depth_ratio)),
        "DepthProjectionUnrotated": size(&layout.depth_projection_unrotated(m)),
        "DepthProjection": layout.depth_projection(m).map_or(Value::Null, |s| projection(&s)),
        "Diagonal": layout.diagonal(m).map_or(Value::Null, number),
        "ActiveSource": m.active_source,
        "BorderResistance": {
            "Left": sections(&m.border_resistance.left),
            "Top": sections(&m.border_resistance.top),
            "Right": sections(&m.border_resistance.right),
            "Bottom": sections(&m.border_resistance.bottom),
        },
        "Sources": sources.iter().map(|s| source(layout, s)).collect::<Vec<_>>(),
    })
}

fn options(layout: &Layout) -> Value {
    let o = &layout.options;
    json!({
        "Enabled": o.enabled,
        "AllowOverlaps": o.allow_overlaps,
        "AllowDiscontinuity": o.allow_discontinuity,
        "Algorithm": o.algorithm,
        "MinimalEdgeOverlap": number(o.minimal_edge_overlap),
        "MaxTravelDistance": number(o.max_travel_distance),
        "MinimalMaxTravelDistance": number(layout.minimal_max_travel_distance()),
        "FreelookCheckInterval": number(o.freelook_check_interval),
        "FreelookEnabled": o.freelook_enabled,
        "LoopX": o.loop_x,
        "LoopY": o.loop_y,
        "AdjustPointer": o.adjust_pointer,
        "AdjustSpeed": o.adjust_speed,
        "IsUnaryRatio": layout.is_unary_ratio(),
        "BorderValues": o.border_values,
        "RescueShortcut": o.rescue_shortcut,
        "Priority": o.priority,
        "PriorityUnhooked": o.priority_unhooked,
        "AutoUpdate": o.auto_update,
        "HomeCinema": o.home_cinema,
        "Pinned": o.pinned,
        "StartMinimized": o.start_minimized,
        "StartElevated": o.start_elevated,
        "HideTrayIcon": o.hide_tray_icon,
        "DebugTools": o.debug_tools,
        "ExperimentalFeatures": o.experimental_features,
        "VcpControl": o.vcp_control,
        "ShowMonitorActionWarning": o.show_monitor_action_warning,
    })
}

/// `OracleRun.Layout`, without the store key and file (added by `run`).
fn layout_json(layout: &Layout) -> Value {
    let mut monitors: Vec<&Monitor> = layout.monitors().iter().collect();
    monitors.sort_by(|a, b| a.id.cmp(&b.id));
    let (dpi_x, dpi_y) = layout.max_effective_dpi();
    json!({
        "Id": layout.id,
        "Saved": layout.saved(),
        "PrimaryMonitor": layout.primary_monitor().map(|m| m.id.clone()),
        "PrimarySource": layout.primary_source().map(|s| s.source.id.clone()),
        "PhysicalBounds": rect(layout.physical_bounds()),
        "MaxEffectiveDpiX": number(dpi_x),
        "MaxEffectiveDpiY": number(dpi_y),
        "Options": options(layout),
        "Monitors": monitors.iter().map(|m| monitor(layout, m)).collect::<Vec<_>>(),
    })
}

/// `OracleRun.PixelLocations`: without and with the Wayland scale adjustment.
fn pixel_locations_json(layout: &Layout) -> Value {
    Value::Array(
        [false, true]
            .into_iter()
            .map(|adjust| {
                let mut placements = layout.compute_pixel_locations_from_physical(adjust);
                placements.sort_by(|a, b| a.0.cmp(&b.0));
                json!({
                    "AdjustScale": adjust,
                    "Placements": placements.iter().map(|(source, p)| json!({
                        "Source": source,
                        "X": number(p.pixel_bounds.x()),
                        "Y": number(p.pixel_bounds.y()),
                        "Width": number(p.pixel_bounds.width()),
                        "Height": number(p.pixel_bounds.height()),
                        "Scale": p.scale.map_or(Value::Null, number),
                    })).collect::<Vec<_>>(),
                })
            })
            .collect(),
    )
}

//==================//
// Comparison       //
//==================//

/// Same values: objects with the same members, arrays in the same order,
/// numbers equal as doubles (so 20 and 20.0 agree, and a double is compared
/// bit for bit once parsed).
fn diff(path: &str, expected: &Value, actual: &Value, out: &mut Vec<String>) {
    match (expected, actual) {
        (Value::Object(e), Value::Object(a)) => {
            for k in e.keys().chain(a.keys().filter(|k| !e.contains_key(*k))) {
                let p = format!("{path}.{k}");
                match (e.get(k), a.get(k)) {
                    (Some(ev), Some(av)) => diff(&p, ev, av, out),
                    (Some(_), None) => out.push(format!("{p}: missing")),
                    (None, Some(_)) => out.push(format!("{p}: unexpected")),
                    (None, None) => {}
                }
            }
        }
        (Value::Array(e), Value::Array(a)) => {
            if e.len() != a.len() {
                out.push(format!("{path}: {} items, expected {}", a.len(), e.len()));
            }
            for (i, (ev, av)) in e.iter().zip(a).enumerate() {
                diff(&format!("{path}[{i}]"), ev, av, out);
            }
        }
        (Value::Number(e), Value::Number(a)) => {
            if e.as_f64() != a.as_f64() {
                out.push(format!("{path}: {a}, expected {e}"));
            }
        }
        (e, a) => {
            if e != a {
                out.push(format!("{path}: {a}, expected {e}"));
            }
        }
    }
}

fn parse(path: &Path) -> Value {
    serde_json::from_str(&read(path)).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

//==================//
// Store directory  //
//==================//

/// `OracleRun.OraclePersistence`: the base hooks, with the process not elevated.
struct OraclePlatform;

impl PersistencePlatform for OraclePlatform {
    fn is_elevated(&self) -> bool {
        false
    }
}

/// `OracleRun.SeedStore`: every file of `input.json`'s `store`, a JSON string standing
/// for raw file text.
fn seed_store(input: &Value, config: &Path) {
    let Some(files) = input.get("store").and_then(Value::as_object) else {
        return;
    };
    for (relative, content) in files {
        assert!(
            !relative.starts_with('/') && !relative.split('/').any(|p| p == ".."),
            "store path must stay inside the store: {relative}"
        );
        let path = config.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let text = match content {
            Value::String(raw) => raw.clone(),
            document => serde_json::to_string_pretty(document).unwrap(),
        };
        fs::write(path, text).unwrap();
    }
}

fn relative_store_path(config: &Path, path: &Path) -> String {
    let relative = path.strip_prefix(config).unwrap();
    relative
        .components()
        .map(|c| c.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

fn files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            files(&path, out);
        } else {
            out.push(path);
        }
    }
}

/// `OracleRun.SavedStore`: every file of the store directory, ordinal order, parsed
/// when it is JSON and as raw text otherwise.
fn saved_store(config: &Path) -> Value {
    let mut paths = Vec::new();
    files(config, &mut paths);
    let mut entries: Vec<(String, Value)> = paths
        .iter()
        .map(|path| {
            let text = fs::read_to_string(path).unwrap();
            let content = serde_json::from_str(&text).unwrap_or(Value::String(text));
            (relative_store_path(config, path), content)
        })
        .collect();
    entries.sort_by(|a, b| a.0.cmp(&b.0));
    Value::Object(entries.into_iter().collect::<Map<_, _>>())
}

//==================//
// Scenarios        //
//==================//

/// One scenario's world: the store seeded in a scratch directory, the engine over it, and
/// the layout built from the inputs — what `OracleRun` sets up before it looks at anything.
fn build(
    input: &Value,
    work: &Path,
) -> (
    PathBuf,
    LayoutPersistence<JsonLayoutStore, OraclePlatform>,
    Layout,
) {
    let config = work.join("config");
    let data = work.join("data");
    fs::create_dir_all(&config).unwrap();
    fs::create_dir_all(&data).unwrap();

    seed_store(input, &config);
    let excluded_file = data.join("Excluded.txt");
    if let Some(lines) = input.get("excluded").and_then(Value::as_array) {
        // File.WriteAllLines: every line ended by the platform's new line.
        let text: String = lines
            .iter()
            .map(|l| format!("{}\n", l.as_str().unwrap()))
            .collect();
        fs::write(&excluded_file, text).unwrap();
    }

    let store = JsonLayoutStore::new(&config);
    let mut persistence =
        LayoutPersistence::with_excluded_list_file(store, OraclePlatform, move || {
            excluded_file.clone()
        });

    let monitors: Vec<LinuxMonitor> = input["displays"]
        .as_array()
        .unwrap()
        .iter()
        .map(linux_monitor)
        .collect();
    let mut layout = Layout::new(LayoutOptions::default());
    populate(&mut layout, &monitors, |l| persistence.load(l)).unwrap();
    (config, persistence, layout)
}

/// Everything the agent's copy of a layout could hold differently from what the frontend
/// edited: a document that carries less than a save writes leaves one of these behind.
fn perturb(layout: &mut Layout) {
    layout.edit_options(|o| {
        o.enabled = !o.enabled;
        o.loop_x = !o.loop_x;
        o.loop_y = !o.loop_y;
        o.allow_overlaps = !o.allow_overlaps;
        o.allow_discontinuity = !o.allow_discontinuity;
        o.adjust_pointer = !o.adjust_pointer;
        o.adjust_speed = !o.adjust_speed;
        o.algorithm = "Perturbed".to_owned();
        o.minimal_edge_overlap += 7.0;
        o.max_travel_distance += 11.0;
        o.freelook_enabled = !o.freelook_enabled;
        o.freelook_check_interval += 13.0;
        o.priority = "Perturbed".to_owned();
        o.priority_unhooked = "Perturbed".to_owned();
        o.home_cinema = !o.home_cinema;
        o.pinned = !o.pinned;
        o.auto_update = !o.auto_update;
        o.start_minimized = !o.start_minimized;
        o.start_elevated = !o.start_elevated;
        o.debug_tools = !o.debug_tools;
        o.experimental_features = !o.experimental_features;
        o.vcp_control = !o.vcp_control;
        o.show_monitor_action_warning = !o.show_monitor_action_warning;
        o.border_values = "Perturbed".to_owned();
        o.rescue_shortcut = "Ctrl+Alt+Perturbed".to_owned();
        o.hide_tray_icon = !o.hide_tray_icon;
    });

    let monitors: Vec<(String, String)> = layout
        .monitors()
        .iter()
        .map(|m| (m.id.clone(), m.model.clone()))
        .collect();
    for (id, model) in monitors {
        let placed = layout
            .depth_projection(layout.monitor(&id).unwrap())
            .unwrap();
        layout.set_location(&id, Point::new(placed.x + 37.0, placed.y - 11.0));
        layout.set_depth_ratio(&id, Ratio::new(1.75, 2.25));
        layout.edit_border_resistance(&id, |resistance| {
            resistance.left.sections.clear();
            resistance.right.sections.clear();
            resistance.top.sections.clear();
            resistance.bottom.sections.clear();
            resistance
                .left
                .sections
                .push(BorderSection::new(1.0, 2.0, 3.0, true, 4.0, true));
        });
        layout.edit_model(&model, |size, name| {
            // A size only where there is one: a stored non-positive size deliberately
            // never overrides the live one (#419), so a document carrying 0 cannot undo
            // a perturbation to 19 — that is the rule, not a hole in the document.
            if size.width() > 0.0 && size.height() > 0.0 {
                size.set_width(size.width() + 19.0);
                size.set_height(size.height() + 23.0);
            }
            size.set_left_border(size.borders().left + 3.0);
            *name = Some("Perturbed".to_owned());
        });
    }
}

/// Every difference between one scenario's recorded outputs and the Rust ones.
fn run(dir: &Path) -> Vec<String> {
    let input = parse(&dir.join("input.json"));
    let work = tempfile::tempdir().unwrap();
    let (config, persistence, mut layout) = build(&input, work.path());

    let mut out = Vec::new();

    let expected_zones = read(&dir.join("expected/zones.xml"));
    let zones = compute_zones(&layout).serialize() + "\n";
    if zones != expected_zones {
        out.push(format!(
            "zones.xml differs:\n  rust: {zones}  c#:   {expected_zones}"
        ));
    }

    let mut actual_layout = layout_json(&layout);
    actual_layout["StoreKey"] = json!(key_for(&layout.id));
    actual_layout["StoreFile"] = json!(relative_store_path(
        &config,
        &persistence.store().layout_path(&layout.id)
    ));
    diff(
        "layout",
        &parse(&dir.join("expected/layout.json")),
        &actual_layout,
        &mut out,
    );
    diff(
        "pixel-locations",
        &parse(&dir.join("expected/pixel-locations.json")),
        &pixel_locations_json(&layout),
        &mut out,
    );

    // Last, as in OracleRun: a save only flips saved flags on the model.
    assert!(persistence.save(&mut layout).unwrap());
    let expected_store = parse(&dir.join("expected/saved-store.json"));
    diff(
        "saved-store",
        &expected_store,
        &saved_store(&config),
        &mut out,
    );

    // The frontends' document (v6, phase 4): the UI sends what it would have saved and
    // the agent writes it. Applied to the agent's own copy of this layout, a save must
    // then write exactly what the C# save wrote — everything the store keeps travels.
    let document: LayoutDocument =
        serde_json::from_str(&read(&dir.join("expected/agent-document.json")))
            .expect("the C# agent document");
    let agent_work = tempfile::tempdir().unwrap();
    let (agent_config, agent_persistence, mut agent_layout) = build(&input, agent_work.path());
    // Moved away from what the store holds first: whatever the document fails to carry
    // stays perturbed and the save below says so.
    perturb(&mut agent_layout);
    document.apply(&mut agent_layout);
    // What the agent would send back: nothing the document carries may have been lost on
    // the way in. The excluded list is left out of the corpus (the defaults are the
    // platform's, see OracleRun); lbm-store's own document test pins that it travels.
    let rebuilt = LayoutDocument {
        excluded: None,
        ..LayoutDocument::of(&agent_layout)
    };
    diff(
        "agent document",
        &parse(&dir.join("expected/agent-document.json")),
        &serde_json::to_value(rebuilt).unwrap(),
        &mut out,
    );
    assert!(agent_persistence.save(&mut agent_layout).unwrap());
    diff(
        "saved-store after the agent document",
        &expected_store,
        &saved_store(&agent_config),
        &mut out,
    );
    out
}

#[test]
fn every_scenario_reproduces_the_recorded_outputs() {
    let mut dirs: Vec<PathBuf> = fs::read_dir(common::oracle_scenarios())
        .expect("domain-oracle/scenarios")
        .map(|e| e.unwrap().path())
        .filter(|p| p.is_dir())
        .collect();
    dirs.sort();
    assert!(dirs.len() >= 21, "only {} scenarios found", dirs.len());

    let mut failures = Vec::new();
    for dir in &dirs {
        let name = dir.file_name().unwrap().to_string_lossy().to_string();
        failures.extend(run(dir).into_iter().map(|d| format!("{name}: {d}")));
    }
    assert!(
        failures.is_empty(),
        "{} differences:\n{}",
        failures.len(),
        failures.join("\n")
    );
}
