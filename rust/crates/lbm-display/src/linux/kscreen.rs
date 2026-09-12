//! KDE output enumeration through `kscreen-doctor --json` — port of
//! `KScreenMonitorSource.cs`.
//!
//! Works on both Wayland and X11 Plasma sessions, and is the only source that gives
//! the compositor's own view: logical positions, per-output scale, priority.

use std::fmt;

use lbm_layout::geo::dotnet;
use lbm_layout::linux::LinuxMonitor;
use serde_json::Value;

use super::desktop::DesktopEnvironment;
use super::drm::EdidMap;

/// Why a `kscreen-doctor` document gave no outputs where C# would have thrown.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KScreenError {
    /// Not JSON (C#: `JsonDocument.Parse` throws).
    Json(String),
    /// `outputs` is not an array (C#: `EnumerateArray` throws).
    OutputsNotAnArray,
    /// Outputs, none of them enabled: C#'s primary fallback (`First(m => m.Enabled)`)
    /// throws.
    NoEnabledOutput,
}

impl fmt::Display for KScreenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            KScreenError::Json(e) => write!(f, "kscreen-doctor output is not JSON: {e}"),
            KScreenError::OutputsNotAnArray => write!(f, "kscreen-doctor outputs is not an array"),
            KScreenError::NoEnabledOutput => write!(f, "no enabled output to make the primary"),
        }
    }
}

impl std::error::Error for KScreenError {}

// Tolerant accessors: the kscreen-doctor schema moved between Plasma 5 and 6 (primary
// vs priority, number formats) — a missing or oddly typed field must never kill
// discovery.

fn get<'a>(e: &'a Value, name: &str) -> Option<&'a Value> {
    e.as_object()?.get(name)
}

/// C# `GetString`: a string, or a number's text; `None` otherwise. (C# keeps a
/// number's raw text; here it is re-printed, which only differs for exotic
/// spellings such as `1e2`.)
fn get_string(e: &Value, name: &str) -> Option<String> {
    match get(e, name)? {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

fn get_bool(e: &Value, name: &str) -> bool {
    matches!(get(e, name), Some(Value::Bool(true)))
}

fn get_double(e: &Value, name: &str, fallback: f64) -> f64 {
    get(e, name).and_then(Value::as_f64).unwrap_or(fallback)
}

/// C# `GetInt`: `(int)Math.Round(v.GetDouble())` — half to even, saturating.
fn get_int(e: &Value, name: &str, fallback: i32) -> i32 {
    get(e, name)
        .and_then(Value::as_f64)
        .map_or(fallback, |v| dotnet::round(v) as i32)
}

/// C# `CurrentMode`: the size and rounded refresh rate of the current mode, else the
/// output's size with no rate (disabled outputs may carry no current mode).
fn current_mode(output: &Value) -> (i32, i32, i32) {
    if let (Some(current), Some(Value::Array(modes))) =
        (get_string(output, "currentModeId"), get(output, "modes"))
    {
        for mode in modes {
            if get_string(mode, "id").as_deref() != Some(current.as_str()) {
                continue;
            }
            let frequency = dotnet::round(get_double(mode, "refreshRate", 0.0)) as i32;
            if let Some(size) = get(mode, "size") {
                return (
                    get_int(size, "width", 0),
                    get_int(size, "height", 0),
                    frequency,
                );
            }
        }
    }
    if let Some(size) = get(output, "size") {
        return (get_int(size, "width", 0), get_int(size, "height", 0), 0);
    }
    (0, 0, 0)
}

/// C# `KScreenMonitorSource.Query`, on a `kscreen-doctor --json` document: the
/// connected outputs, in the document's order, with their EDIDs from `edids`.
pub fn parse(json: &str, edids: &EdidMap) -> Result<Vec<LinuxMonitor>, KScreenError> {
    let doc: Value = serde_json::from_str(json).map_err(|e| KScreenError::Json(e.to_string()))?;
    let Some(outputs) = get(&doc, "outputs") else {
        return Ok(Vec::new());
    };
    let outputs = outputs.as_array().ok_or(KScreenError::OutputsNotAnArray)?;

    let mut monitors = Vec::new();
    for output in outputs {
        if !get_bool(output, "connected") {
            continue;
        }

        let connector = get_string(output, "name").unwrap_or_default();
        let enabled = get_bool(output, "enabled");
        let mut scale = get_double(output, "scale", 1.0);
        if scale <= 0.0 {
            scale = 1.0;
        }

        // KScreen rotation is a flag: 1 = none, 2 = 90°, 4 = 180°, 8 = 270°.
        let orientation = match get_int(output, "rotation", 1) {
            2 => 1,
            4 => 2,
            8 => 3,
            _ => 0,
        };

        let (mut mode_w, mut mode_h, frequency) = current_mode(output);
        if orientation % 2 != 0 {
            std::mem::swap(&mut mode_w, &mut mode_h);
        }

        let (x, y) = get(output, "pos").map_or((0.0, 0.0), |pos| {
            (get_double(pos, "x", 0.0), get_double(pos, "y", 0.0))
        });

        let (mut width_mm, mut height_mm) = get(output, "sizeMM").map_or((0.0, 0.0), |mm| {
            (get_double(mm, "width", 0.0), get_double(mm, "height", 0.0))
        });
        if orientation % 2 != 0 {
            std::mem::swap(&mut width_mm, &mut height_mm);
        }

        monitors.push(LinuxMonitor {
            edid: edids.get(&connector).map(|e| e.to_linux()),
            connector_name: connector,
            logical_x: x,
            logical_y: y,
            logical_width: dotnet::round(f64::from(mode_w) / scale),
            logical_height: dotnet::round(f64::from(mode_h) / scale),
            pixel_width: mode_w,
            pixel_height: mode_h,
            scale,
            width_mm,
            height_mm,
            // Plasma 6 replaced the primary flag by a priority order: 1 is the
            // primary, 0/-1 means none (disabled outputs report -1).
            primary: get_int(output, "priority", 0) == 1,
            enabled,
            orientation,
            frequency,
        });
    }

    // No priority-1 output (older Plasma, odd configs): fall back to the enabled output
    // at the logical origin, the model needs a primary to anchor on.
    if !monitors.is_empty() && !monitors.iter().any(|m| m.primary) {
        let primary = monitors
            .iter()
            .position(|m| m.enabled && m.logical_x == 0.0 && m.logical_y == 0.0)
            .or_else(|| monitors.iter().position(|m| m.enabled))
            .ok_or(KScreenError::NoEnabledOutput)?;
        monitors[primary].primary = true;
    }

    Ok(monitors)
}

/// C# `RunKScreenDoctor`: the document `kscreen-doctor --json` prints, when it exits
/// successfully with something that starts like a JSON object.
pub fn run_kscreen_doctor() -> Option<String> {
    // Bounded: kscreen-doctor neither prints nor exits when it cannot reach its session,
    // and the agent probes at startup, before it has logged anything (see `probe`).
    let stdout = super::probe::run("kscreen-doctor", &["--json"], super::probe::PATIENCE)?;
    stdout.trim_start().starts_with('{').then_some(stdout)
}

/// C# `KScreenMonitorSource.IsAvailable`: a KDE session where `kscreen-doctor` answers.
pub fn is_available(desktop: &DesktopEnvironment) -> bool {
    desktop.is_kde() && run_kscreen_doctor().is_some()
}
