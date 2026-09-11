//! `lbm-agent`: the resident process of LittleBigMouse v6 — it watches the system,
//! loads the profiles and drives the hook (`docs/v6-architecture-plan.md` on the `v6`
//! branch, phase 3).
//!
//! For now it only has `--dump-displays` (phase 2): the display discovery of the
//! machine as JSON — the outputs, and the monitor ids and layout id the layout model
//! gives them. Its C# twin is `DisplayDump` in the C# test project; run both on the
//! same machine and the values must be equal.

use std::process::ExitCode;

use lbm_display::linux::{display_json, display_signature, drm, Backend};
use lbm_layout::linux::add_monitor;
use lbm_layout::model::{Layout, LayoutOptions};
use serde_json::json;

const USAGE: &str = "usage: lbm-agent --dump-displays";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        ["--dump-displays"] => dump_displays(),
        _ => {
            eprintln!("{USAGE}");
            ExitCode::from(2)
        }
    }
}

/// The display discovery as JSON, member for member what the C# `DisplayDump` writes.
fn dump_displays() -> ExitCode {
    if cfg!(windows) {
        eprintln!("--dump-displays: the Windows discovery is not ported yet");
        return ExitCode::FAILURE;
    }

    let backend = Backend::detect();
    let monitors = match backend.map(Backend::query).transpose() {
        Ok(monitors) => monitors.unwrap_or_default(),
        Err(error) => {
            eprintln!("--dump-displays: {error}");
            return ExitCode::FAILURE;
        }
    };

    let mut layout = Layout::new(LayoutOptions::default());
    for monitor in &monitors {
        add_monitor(&mut layout, monitor);
    }

    let dump = json!({
        "Backend": backend.map(Backend::name),
        "Displays": monitors.iter().map(display_json).collect::<Vec<_>>(),
        "LayoutId": layout.compute_id(),
        "Monitors": layout.monitors().iter().map(|m| json!({
            "Id": m.id,
            "PnpCode": m.model,
            "DeviceId": m.device_id,
        })).collect::<Vec<_>>(),
        "PlugSignature": drm::plug_signature(),
        "DisplaySignature": display_signature(&monitors),
    });
    println!(
        "{}",
        serde_json::to_string_pretty(&dump).expect("a JSON value prints")
    );
    ExitCode::SUCCESS
}
