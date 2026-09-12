//! Linux discovery on recorded tool outputs and a fake sysfs: the C# rules of
//! `KScreenMonitorSource`, `XRandRMonitorSource` and `DrmEdidReader`, down to the
//! monitor ids the layout model builds from them.

use std::fs;
use std::path::Path;

use lbm_display::edid;
use lbm_display::linux::drm::{self, EdidMap};
use lbm_display::linux::{display_signature, kscreen, xrandr, KScreenError};
use lbm_layout::linux::add_monitor;
use lbm_layout::model::{Layout, LayoutOptions};

/// A 128-byte EDID: DEL, product A0B1, binary serial 0x01020304, model "U2720Q",
/// serial string "ABC123", 597 x 336 mm, DisplayPort.
fn dell_edid() -> Vec<u8> {
    let mut e = vec![0u8; 128];
    e[..8].copy_from_slice(&[0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x00]);
    e[8] = 0x10; // "DEL": 00100 00101 01100
    e[9] = 0xAC;
    e[10] = 0xB1; // product 0xA0B1, little-endian
    e[11] = 0xA0;
    e[12..16].copy_from_slice(&[0x04, 0x03, 0x02, 0x01]);
    e[16] = 12;
    e[17] = 30;
    e[18] = 1;
    e[19] = 4;
    e[20] = 0x80 | 0x05; // digital, DisplayPort
    e[21] = 60;
    e[22] = 34;
    e[66] = 0x55; // 597 = 0x255
    e[67] = 0x50; // 336 = 0x150
    e[68] = 0x21;
    let descriptor = |e: &mut Vec<u8>, at: usize, tag: u8, text: &[u8]| {
        e[at..at + 5].copy_from_slice(&[0, 0, 0, tag, 0]);
        e[at + 5..at + 18].fill(0x20);
        e[at + 5..at + 5 + text.len()].copy_from_slice(text);
        e[at + 5 + text.len()] = 0x0A;
    };
    descriptor(&mut e, 72, 0xFC, b"U2720Q");
    descriptor(&mut e, 90, 0xFF, b"ABC123");
    e
}

#[test]
fn the_edid_fields_the_id_is_made_of() {
    let e = edid::parse("k", &dell_edid());
    assert_eq!(e.manufacturer_code.as_deref(), Some("DEL"));
    assert_eq!(e.product_code.as_deref(), Some("A0B1"));
    assert_eq!(e.serial.as_deref(), Some("01020304"));
    assert_eq!(e.model.as_deref(), Some("U2720Q"));
    assert_eq!(e.serial_number.as_deref(), Some("ABC123"));
    assert_eq!((e.physical_width, e.physical_height), (597.0, 336.0));
    assert_eq!(e.video_interface.as_deref(), Some("DisplayPort"));
    assert_eq!(
        (e.week, e.year, e.version.as_deref()),
        (12, 2020, Some("1.4"))
    );
}

/// The shape of `kscreen-doctor --json` on Plasma 6, trimmed.
const KSCREEN: &str = r#"{
  "outputs": [
    {"connected": true, "currentModeId": "4", "enabled": true, "id": 1,
     "modes": [
       {"id": "1", "refreshRate": 59.99700164794922, "size": {"height": 2160, "width": 3840}},
       {"id": "4", "refreshRate": 143.97099304199219, "size": {"height": 2160, "width": 3840}}
     ],
     "name": "DP-2", "pos": {"x": 0, "y": 0}, "priority": 1, "rotation": 1, "scale": 1.25,
     "size": {"height": 2160, "width": 3840}, "sizeMM": {"height": 393, "width": 698}},
    {"connected": true, "currentModeId": 54, "enabled": true,
     "modes": [{"id": 54, "refreshRate": 60.5, "size": {"height": 1080, "width": 1920}}],
     "name": "DP-3", "pos": {"x": 3072, "y": -200}, "priority": 2, "rotation": 2, "scale": 1,
     "sizeMM": {"height": 300, "width": 530}},
    {"connected": true, "enabled": false, "name": "HDMI-A-1", "priority": -1, "rotation": 1,
     "scale": 0, "size": {"height": 1080, "width": 1920}},
    {"connected": false, "enabled": false, "name": "DP-1"}
  ]
}"#;

#[test]
fn kscreen_outputs_map_as_in_csharp() {
    let mut edids = EdidMap::default();
    edids.insert("dp-2", edid::parse("sys", &dell_edid()));
    let monitors = kscreen::parse(KSCREEN, &edids).unwrap();
    assert_eq!(monitors.len(), 3, "the disconnected output is left out");

    let [dp2, dp3, hdmi] = &monitors[..] else {
        unreachable!()
    };
    // The current mode by id, its rate rounded; logical size is the mode over the scale.
    assert_eq!(dp2.connector_name, "DP-2");
    assert_eq!(
        (dp2.pixel_width, dp2.pixel_height, dp2.frequency),
        (3840, 2160, 144)
    );
    assert_eq!((dp2.logical_width, dp2.logical_height), (3072.0, 1728.0));
    assert_eq!((dp2.width_mm, dp2.height_mm), (698.0, 393.0));
    assert!(dp2.primary && dp2.enabled);
    assert_eq!(dp2.orientation, 0);
    assert_eq!(
        dp2.edid.as_ref().and_then(|e| e.serial_number.as_deref()),
        Some("ABC123"),
        "EDIDs are matched case-insensitively"
    );

    // Rotation flag 2 (90°): one quarter turn, mode and millimetres swapped. A
    // numeric mode id still matches; 60.5 Hz rounds half to even.
    assert_eq!(dp3.orientation, 1);
    assert_eq!(
        (dp3.pixel_width, dp3.pixel_height, dp3.frequency),
        (1080, 1920, 60)
    );
    assert_eq!((dp3.width_mm, dp3.height_mm), (300.0, 530.0));
    assert_eq!((dp3.logical_x, dp3.logical_y), (3072.0, -200.0));
    assert!(!dp3.primary);
    assert!(dp3.edid.is_none());

    // No current mode: the output's size, no rate; a zero scale counts as 1.
    assert!(!hdmi.enabled);
    assert_eq!(
        (hdmi.pixel_width, hdmi.pixel_height, hdmi.frequency),
        (1920, 1080, 0)
    );
    assert_eq!(hdmi.scale, 1.0);

    assert_eq!(
        display_signature(&monitors),
        "DP-2[0,0 3840x2160@1.25]*d698x393|DP-3[3072,-200 1080x1920@1]d300x530|HDMI-A-1[0,0 1920x1080@1]-d0x0"
    );
}

#[test]
fn without_a_priority_one_output_the_primary_falls_back_as_in_csharp() {
    let doc = |a: &str, b: &str| {
        format!(
            r#"{{"outputs": [
            {{"connected": true, "name": "A", "pos": {{"x": 0, "y": 0}}, {a}}},
            {{"connected": true, "name": "B", "pos": {{"x": 1920, "y": 0}}, {b}}}]}}"#
        )
    };
    let primary = |json: &str| -> Vec<bool> {
        kscreen::parse(json, &EdidMap::default())
            .unwrap()
            .iter()
            .map(|m| m.primary)
            .collect()
    };
    // The enabled output at the origin...
    assert_eq!(
        primary(&doc(r#""enabled": true"#, r#""enabled": true"#)),
        [true, false]
    );
    // ...else the first enabled one.
    assert_eq!(
        primary(&doc(r#""enabled": false"#, r#""enabled": true"#)),
        [false, true]
    );
    // None enabled: C#'s `First(m => m.Enabled)` throws.
    assert_eq!(
        kscreen::parse(
            &doc(r#""enabled": false"#, r#""enabled": false"#),
            &EdidMap::default()
        ),
        Err(KScreenError::NoEnabledOutput)
    );
    assert!(kscreen::parse("{}", &EdidMap::default())
        .unwrap()
        .is_empty());
    assert!(kscreen::parse("{", &EdidMap::default()).is_err());
}

const XRANDR: &str = "\
Screen 0: minimum 8 x 8, current 4920 x 2160, maximum 32767 x 32767
DP-4 connected 3840x2160+0+0 (normal left inverted right x axis y axis) 597mm x 336mm
   3840x2160     60.00*+
DP-5 connected primary 1080x1920+3840+0 left (normal left inverted right x axis y axis) 336mm x 597mm
HDMI-0 disconnected (normal left inverted right x axis y axis)
DP-6 connected (normal left inverted right x axis y axis)
VIRTUAL1 connected 800x600+-800+0
";

#[test]
fn xrandr_lines_map_as_in_csharp() {
    let mut edids = EdidMap::default();
    edids.insert("DP-4", edid::parse("sys", &dell_edid()));
    let monitors = xrandr::parse(XRANDR, &edids);
    let names: Vec<&str> = monitors.iter().map(|m| m.connector_name.as_str()).collect();
    assert_eq!(
        names,
        ["DP-4", "DP-5", "VIRTUAL1"],
        "outputs without a geometry are left out"
    );

    let dp4 = &monitors[0];
    assert_eq!(
        (dp4.logical_x, dp4.logical_y, dp4.logical_width),
        (0.0, 0.0, 3840.0)
    );
    assert_eq!((dp4.width_mm, dp4.height_mm), (597.0, 336.0));
    assert!(!dp4.primary && dp4.enabled && dp4.edid.is_some());
    assert_eq!((dp4.scale, dp4.frequency), (1.0, 0));

    let dp5 = &monitors[1];
    assert!(dp5.primary);
    assert_eq!(dp5.orientation, 1);
    assert_eq!((dp5.pixel_width, dp5.pixel_height), (1080, 1920));

    let virtual1 = &monitors[2];
    assert_eq!(virtual1.logical_x, -800.0);
    assert_eq!((virtual1.width_mm, virtual1.height_mm), (0.0, 0.0));

    // No primary: the output at the origin, else the first.
    let no_primary = XRANDR.replace(" primary", "");
    let primaries: Vec<bool> = xrandr::parse(&no_primary, &edids)
        .iter()
        .map(|m| m.primary)
        .collect();
    assert_eq!(primaries, [true, false, false]);
}

fn connector(root: &Path, name: &str, status: &str, edid: &[u8]) {
    let dir = root.join(name);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("status"), format!("{status}\n")).unwrap();
    fs::write(dir.join("edid"), edid).unwrap();
}

#[test]
fn sysfs_edids_follow_the_dual_gpu_rule() {
    let sys = tempfile::tempdir().unwrap();
    let root = sys.path();
    // The same connector name on two cards: only the live one counts.
    connector(root, "card0-DP-1", "connected", &dell_edid());
    connector(root, "card1-DP-1", "disconnected", &[]);
    // A partial block is no EDID.
    connector(root, "card0-HDMI-A-1", "connected", &dell_edid()[..100]);
    // Not connectors.
    fs::create_dir_all(root.join("card1")).unwrap();
    fs::create_dir_all(root.join("renderD128")).unwrap();
    fs::write(root.join("version"), "drm 1.1.0").unwrap();

    let edids = drm::read_all_in(root);
    assert_eq!(edids.len(), 1);
    let dp1 = edids.get("DP-1").expect("the connected card's EDID");
    assert_eq!(dp1.serial_number.as_deref(), Some("ABC123"));
    assert!(dp1.key.ends_with("card0-DP-1"));

    assert_eq!(
        drm::plug_signature_in(root),
        "card0-DP-1:connected:128|card0-HDMI-A-1:connected:100|card1-DP-1:disconnected:0"
    );
    assert_eq!(drm::plug_signature_in(&root.join("missing")), "");
}

#[test]
fn discovered_outputs_give_the_edid_monitor_ids() {
    let mut edids = EdidMap::default();
    edids.insert("DP-2", edid::parse("sys", &dell_edid()));
    let monitors = kscreen::parse(KSCREEN, &edids).unwrap();

    let mut layout = Layout::new(LayoutOptions::default());
    for monitor in &monitors {
        add_monitor(&mut layout, monitor);
    }
    let ids: Vec<&str> = layout.monitors().iter().map(|m| m.id.as_str()).collect();
    // {Mfg}{Product}_{serial string}; without an EDID, the connector.
    assert_eq!(ids, ["DELA0B1_ABC123", "DP-3", "HDMI-A-1"]);
    assert_eq!(layout.compute_id(), "DELA0B1_ABC123+DP-3_1+HDMI-A-1");
}
