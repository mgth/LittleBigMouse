//! The generic X11 fallback through `xrandr --query` — port of
//! `XRandRMonitorSource.cs`.
//!
//! For sessions without KScreen. On native X11 the output names are real connectors
//! (sysfs EDID matching works) and the geometry is in pixels: logical is pixels, and
//! the scale is 1. Under a Wayland compositor other than KWin this sees whatever
//! XWayland exposes, which is still enough to edit a layout.

use std::process::{Command, Stdio};
use std::sync::OnceLock;

use lbm_layout::linux::LinuxMonitor;
use regex::Regex;

use super::drm::EdidMap;

/// C#'s pattern, as it is: `DP-4 connected primary 3840x2160+0+0 left (normal left
/// inverted right x axis y axis) 597mm x 336mm`. Anchored at the start of the line
/// only; a connected output without a geometry (disabled) does not match.
fn line() -> &'static Regex {
    static LINE: OnceLock<Regex> = OnceLock::new();
    LINE.get_or_init(|| {
        Regex::new(
            r"^(?<name>\S+) connected(?<primary> primary)? (?<w>\d+)x(?<h>\d+)\+(?<x>-?\d+)\+(?<y>-?\d+)(?<rot> normal| left| inverted| right)?(?: \([^)]*\))?(?: (?<wmm>\d+)mm x (?<hmm>\d+)mm)?",
        )
        .expect("a valid pattern")
    })
}

/// C# `XRandRMonitorSource.Query`, on `xrandr --query` output: the outputs with a
/// geometry, in xrandr's order, with their EDIDs from `edids`. A number too large for
/// an `int` skips its line (C#'s `int.Parse` would throw).
pub fn parse(stdout: &str, edids: &EdidMap) -> Vec<LinuxMonitor> {
    let mut monitors = Vec::new();
    for text in stdout.split('\n') {
        let Some(m) = line().captures(text) else {
            continue;
        };
        let int = |name: &str| m.name(name).map(|g| g.as_str().parse::<i32>());
        let (Some(Ok(width)), Some(Ok(height)), Some(Ok(x)), Some(Ok(y))) =
            (int("w"), int("h"), int("x"), int("y"))
        else {
            continue;
        };
        let (width_mm, height_mm) = match (int("wmm"), int("hmm")) {
            (Some(Ok(w)), Some(Ok(h))) => (w, h),
            (None, None) => (0, 0),
            _ => continue,
        };

        let connector = &m["name"];
        let orientation = match m.name("rot").map(|r| r.as_str().trim()) {
            Some("left") => 1,
            Some("inverted") => 2,
            Some("right") => 3,
            _ => 0,
        };

        monitors.push(LinuxMonitor {
            connector_name: connector.to_owned(),
            logical_x: f64::from(x),
            logical_y: f64::from(y),
            logical_width: f64::from(width),
            logical_height: f64::from(height),
            pixel_width: width,
            pixel_height: height,
            scale: 1.0,
            width_mm: f64::from(width_mm),
            height_mm: f64::from(height_mm),
            primary: m.name("primary").is_some(),
            enabled: true,
            orientation,
            frequency: 0,
            edid: edids.get(connector).map(|e| e.to_linux()),
        });
    }

    // xrandr marks no output as primary under some compositors: pick the one at (0,0),
    // the model needs a primary to anchor and place monitors.
    if !monitors.is_empty() && !monitors.iter().any(|m| m.primary) {
        let primary = monitors
            .iter()
            .position(|m| m.logical_x == 0.0 && m.logical_y == 0.0)
            .unwrap_or(0);
        monitors[primary].primary = true;
    }
    monitors
}

/// C# `RunXRandR`: `xrandr --query`'s output, when it exits successfully.
pub fn run_xrandr() -> Option<String> {
    let output = Command::new("xrandr")
        .arg("--query")
        .stdin(Stdio::null())
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).into_owned())
}

/// C# `XRandRMonitorSource.IsAvailable`: an X display where `xrandr` answers.
pub fn is_available() -> bool {
    std::env::var_os("DISPLAY").is_some_and(|d| !d.is_empty()) && run_xrandr().is_some()
}
