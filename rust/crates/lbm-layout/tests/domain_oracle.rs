//! The domain oracle (`domain-oracle/` at the repository root) for the
//! scenarios that need no store: the Rust pipeline must produce what the C#
//! recorded. `zones.xml` must match byte for byte; `layout.json` and
//! `pixel-locations.json` must hold the same values, doubles bit for bit.
//!
//! Loading with nothing stored is the C# `LayoutPersistence.Load` reduced to
//! what it does then: mark everything saved and republish. The scenarios with a
//! store, `saved-store.json`, and the store key and file name recorded in
//! `layout.json` need the persistence layer, and are checked by lbm-store.

use std::path::{Path, PathBuf};

use lbm_layout::geo::Rect;
use lbm_layout::linux::{populate, LinuxEdid, LinuxMonitor};
use lbm_layout::model::{
    BorderSide, DisplaySize, Layout, LayoutOptions, Monitor, PhysicalSource, Ratio,
};
use lbm_layout::zoning::compute_zones;
use serde_json::{json, Value};

fn corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .expect("crate lives at rust/crates/lbm-layout")
        .join("domain-oracle/scenarios")
}

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

/// `OracleRun.Layout`, without the store key and file (see the module doc).
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

fn without(mut v: Value, keys: &[&str]) -> Value {
    if let Value::Object(map) = &mut v {
        for k in keys {
            map.remove(*k);
        }
    }
    v
}

fn parse(path: &Path) -> Value {
    serde_json::from_str(&read(path)).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

#[test]
fn scenarios_without_a_store_reproduce_the_recorded_outputs() {
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(corpus())
        .expect("domain-oracle/scenarios")
        .map(|e| e.unwrap().path())
        .filter(|p| p.is_dir())
        .collect();
    dirs.sort();

    let mut checked = 0;
    let mut failures = Vec::new();
    for dir in &dirs {
        let name = dir.file_name().unwrap().to_string_lossy().to_string();
        let input = parse(&dir.join("input.json"));
        if !matches!(input.get("store"), None | Some(Value::Null)) {
            continue;
        }
        checked += 1;
        let monitors: Vec<LinuxMonitor> = input["displays"]
            .as_array()
            .unwrap()
            .iter()
            .map(linux_monitor)
            .collect();
        let mut layout = Layout::new(LayoutOptions::default());
        populate(&mut layout, &monitors, |l| {
            // LayoutPersistence.Load with nothing stored.
            l.mark_saved();
            l.parse_physical_monitors();
        });

        let expected_zones = read(&dir.join("expected/zones.xml"));
        let zones = compute_zones(&layout).serialize() + "\n";
        if zones != expected_zones {
            failures.push(format!(
                "{name}/zones.xml differs:\n  rust: {zones}  c#:   {expected_zones}"
            ));
        }

        let mut out = Vec::new();
        diff(
            "layout",
            &without(
                parse(&dir.join("expected/layout.json")),
                &["StoreKey", "StoreFile"],
            ),
            &layout_json(&layout),
            &mut out,
        );
        diff(
            "pixel-locations",
            &parse(&dir.join("expected/pixel-locations.json")),
            &pixel_locations_json(&layout),
            &mut out,
        );
        failures.extend(out.into_iter().map(|d| format!("{name}: {d}")));
    }
    assert!(checked >= 9, "only {checked} store-less scenarios found");
    assert!(
        failures.is_empty(),
        "{} differences:\n{}",
        failures.len(),
        failures.join("\n")
    );
}
