//! A layout holding a remote machine's screens, through the store (docs/multi-pc.md):
//! which machine a source is on, the input value that selects it, and which source a
//! shared panel shows.

mod common;

use common::design_options;
use lbm_layout::geo::Point;
use lbm_layout::linux::{add_monitor, LinuxEdid, LinuxMonitor};
use lbm_layout::model::Layout;
use lbm_layout::remote::{merge_remote, remote_source_id, RemoteMonitor};
use lbm_store::LayoutDocument;
use serde_json::Value;

const LAPTOP: &str = "a1b2c3d4e5f60718293a4b5c6d7e8f90a1b2c3d4e5f60718293a4b5c6d7e8f90";
const SHARED: &str = "DEL4065_ABC123";

fn dell() -> LinuxMonitor {
    LinuxMonitor {
        connector_name: "DP-1".to_owned(),
        logical_x: 0.0,
        logical_y: 0.0,
        logical_width: 2560.0,
        logical_height: 1440.0,
        pixel_width: 2560,
        pixel_height: 1440,
        scale: 1.0,
        width_mm: 597.0,
        height_mm: 336.0,
        primary: true,
        enabled: true,
        orientation: 0,
        frequency: 60,
        edid: Some(LinuxEdid {
            manufacturer_code: Some("DEL".to_owned()),
            product_code: Some("4065".to_owned()),
            serial: Some("00000000".to_owned()),
            serial_number: Some("ABC123".to_owned()),
            model: Some("DELL U2720Q".to_owned()),
            physical_width: 597.0,
            physical_height: 336.0,
            video_interface: Some("DisplayPort".to_owned()),
        }),
    }
}

fn laptop(dell_attached: bool) -> Vec<RemoteMonitor> {
    let display = |id: &str, key: Option<&str>, x: i32| RemoteMonitor {
        id: id.to_owned(),
        connector: id.to_owned(),
        panel_key: key.map(str::to_owned),
        x,
        width: 1920,
        height: 1080,
        scale: 1.0,
        attached: true,
        width_mm: 344.0,
        height_mm: 194.0,
        ..RemoteMonitor::default()
    };
    let mut dell = display("HDMI-1", Some(SHARED), 0);
    dell.attached = dell_attached;
    vec![display("eDP-1", None, -1920), dell]
}

/// What the agent builds at each display change: this machine's screens, then the
/// laptop's merged in.
fn rebuilt(dell_attached_on_laptop: bool) -> Layout {
    let mut layout = Layout::new(design_options());
    add_monitor(&mut layout, &dell());
    merge_remote(
        &mut layout,
        LAPTOP,
        "laptop",
        &laptop(dell_attached_on_laptop),
    );
    layout.id = layout.compute_id();
    layout
}

fn source_json<'a>(json: &'a Value, monitor: &str, source: &str) -> &'a Value {
    &json["Layout"]["Monitors"][monitor]["Sources"][source]
}

#[test]
fn the_document_says_which_machine_a_source_is_on_and_how_to_select_it() {
    let mut layout = rebuilt(true);
    let remote_dell = remote_source_id(LAPTOP, "HDMI-1");
    layout.set_active_source(SHARED, Some(remote_dell.clone()));
    layout.set_source_input(SHARED, Some(0x0F));
    layout.set_source_input(&remote_dell, Some(0x11));

    let json = serde_json::to_value(LayoutDocument::of(&layout)).unwrap();

    let local = source_json(&json, SHARED, SHARED);
    assert_eq!(local.get("Host"), None);
    assert_eq!(local["InputSource"], 15);
    let remote = source_json(&json, SHARED, &remote_dell);
    assert_eq!(remote["Host"], LAPTOP);
    assert_eq!(remote["InputSource"], 17);
    assert_eq!(
        json["Layout"]["Monitors"][SHARED]["ActiveSource"],
        remote_dell.as_str()
    );
    let panel = remote_source_id(LAPTOP, "eDP-1");
    assert_eq!(source_json(&json, &panel, &panel)["Host"], LAPTOP);
}

#[test]
fn a_rebuilt_layout_gets_back_the_choice_the_inputs_and_the_remote_placement() {
    let mut saved = rebuilt(true);
    let remote_dell = remote_source_id(LAPTOP, "HDMI-1");
    let panel = remote_source_id(LAPTOP, "eDP-1");
    saved.set_active_source(SHARED, Some(remote_dell.clone()));
    saved.set_source_input(SHARED, Some(0x0F));
    saved.set_location(&panel, Point::new(-400.0, 25.0));
    saved.set_placed(&panel, true);
    let text = serde_json::to_string(&LayoutDocument::of(&saved)).unwrap();

    let mut layout = rebuilt(true);
    let document: LayoutDocument = serde_json::from_str(&text).unwrap();
    document.apply(&mut layout);

    let shared = layout.monitor(SHARED).unwrap();
    assert_eq!(shared.active_source.as_deref(), Some(remote_dell.as_str()));
    assert_eq!(
        layout.source(SHARED).unwrap().source.input_source,
        Some(0x0F)
    );
    let restored = layout.monitor(&panel).unwrap();
    assert_eq!(restored.location(), Point::new(-400.0, 25.0));
    assert!(restored.placed);
}

#[test]
fn a_stored_choice_of_a_source_gone_dark_gives_way_to_the_one_still_showing() {
    let mut saved = rebuilt(true);
    let remote_dell = remote_source_id(LAPTOP, "HDMI-1");
    saved.set_active_source(SHARED, Some(remote_dell));
    let text = serde_json::to_string(&LayoutDocument::of(&saved)).unwrap();

    let mut layout = rebuilt(false);
    let document: LayoutDocument = serde_json::from_str(&text).unwrap();
    document.apply(&mut layout);

    assert_eq!(
        layout.monitor(SHARED).unwrap().active_source.as_deref(),
        Some(SHARED)
    );
}

#[test]
fn a_detached_source_is_kept_in_the_store_for_the_input_it_remembers() {
    let mut layout = rebuilt(false);
    let remote_dell = remote_source_id(LAPTOP, "HDMI-1");
    layout.set_source_input(&remote_dell, Some(0x11));
    let json = serde_json::to_value(LayoutDocument::of(&layout)).unwrap();
    assert_eq!(source_json(&json, SHARED, &remote_dell)["InputSource"], 17);

    let mut rebuilt = rebuilt(false);
    let document: LayoutDocument = serde_json::from_value(json).unwrap();
    document.apply(&mut rebuilt);
    assert_eq!(
        rebuilt.source(&remote_dell).unwrap().source.input_source,
        Some(0x11)
    );
}

#[test]
fn a_layout_without_remotes_stores_neither_host_nor_input() {
    let mut layout = Layout::new(design_options());
    add_monitor(&mut layout, &dell());
    let text = serde_json::to_string(&LayoutDocument::of(&layout)).unwrap();
    assert!(!text.contains("\"Host\""));
    assert!(!text.contains("InputSource"));
}
