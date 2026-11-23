//! A remote machine's report of its screens, read the way this machine reads its own
//! (docs/multi-pc.md, "Model").

use base64::Engine as _;
use lbm_display::edid;
use lbm_display::peer::{merge_peer, panel_key, remote_monitor};
use lbm_ipc::peer::PeerDisplay;
use lbm_layout::linux::{add_monitor, LinuxMonitor};
use lbm_layout::model::{Layout, LayoutOptions};
use lbm_layout::remote::remote_source_id;

const LAPTOP: &str = "a1b2c3d4e5f60718293a4b5c6d7e8f90a1b2c3d4e5f60718293a4b5c6d7e8f90";

/// A 128-byte EDID block: the manufacturer, product and binary serial of the basic
/// block, the image size of a first detailed timing, and the serial and name
/// descriptors. `interface` is the low nibble of byte 20 (5: DisplayPort, 2: HDMI) —
/// what differs between two inputs of one monitor, with the checksum.
fn edid_block(serial_string: &str, interface: u8) -> Vec<u8> {
    let mut b = vec![0u8; 128];
    b[..8].copy_from_slice(&[0, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0]);
    // "DEL": D=4, E=5, L=12 in five-bit letters.
    let letters = (4u16 << 10) | (5 << 5) | 12;
    b[8] = (letters >> 8) as u8;
    b[9] = letters as u8;
    b[10] = 0x65;
    b[11] = 0x40;
    b[12..16].copy_from_slice(&[0x01, 0x02, 0x03, 0x04]);
    b[16] = 12;
    b[17] = 30;
    b[18] = 1;
    b[19] = 4;
    b[20] = 0xA0 | interface;
    b[21] = 60;
    b[22] = 34;
    // First detailed timing: 597 x 336 mm.
    b[54] = 0x01;
    b[66] = (597 & 0xFF) as u8;
    b[67] = (336 & 0xFF) as u8;
    b[68] = (((597 >> 8) << 4) | (336 >> 8)) as u8;
    let descriptor = |b: &mut Vec<u8>, at: usize, tag: u8, text: &str| {
        b[at..at + 5].copy_from_slice(&[0, 0, 0, tag, 0]);
        let mut bytes = text.as_bytes().to_vec();
        bytes.push(0x0A);
        bytes.resize(13, 0x20);
        b[at + 5..at + 18].copy_from_slice(&bytes);
    };
    descriptor(&mut b, 72, 0xFF, serial_string);
    descriptor(&mut b, 90, 0xFC, "DELL U2720Q");
    let sum: u32 = b[..127].iter().map(|&x| u32::from(x)).sum();
    b[127] = ((256 - sum % 256) % 256) as u8;
    b
}

fn peer_display(id: &str, connector: &str, edid: Option<&[u8]>, x: i32) -> PeerDisplay {
    PeerDisplay {
        id: id.to_owned(),
        connector: connector.to_owned(),
        name: "DELL U2720Q".to_owned(),
        edid: edid.map(|e| base64::engine::general_purpose::STANDARD.encode(e)),
        x,
        y: 0,
        width: 1920,
        height: 1080,
        scale: 1.0,
        effective_dpi: 96.0,
        orientation: 0,
        frequency: 59.94,
        primary: true,
        attached: true,
        width_mm: 597.0,
        height_mm: 336.0,
        input_source: Some(0x11),
    }
}

fn local_dell(edid: &[u8]) -> Layout {
    let mut layout = Layout::new(LayoutOptions::default());
    add_monitor(
        &mut layout,
        &LinuxMonitor {
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
            edid: Some(edid::parse("card0-DP-1", edid).to_linux()),
        },
    );
    layout
}

#[test]
fn a_remotes_edid_gives_the_key_this_machines_detection_gives_the_same_panel() {
    // The same panel seen through its DisplayPort input here and its HDMI one there:
    // the interface and the checksum differ, the key does not.
    let here = edid_block("ABC123", 5);
    let there = edid_block("ABC123", 2);
    assert_ne!(here[127], there[127]);

    let display = peer_display("DEL4065ABC123_12_07BC_9F", "HDMI-1", Some(&there), 0);
    assert_eq!(panel_key(&display).as_deref(), Some("DEL4065_ABC123"));
    assert_eq!(
        local_dell(&here).monitors()[0].panel_key.as_deref(),
        Some("DEL4065_ABC123")
    );
}

#[test]
fn a_display_without_an_edid_has_no_key() {
    assert_eq!(panel_key(&peer_display("X", "HDMI-1", None, 0)), None);
    let mut garbled = peer_display("X", "HDMI-1", None, 0);
    garbled.edid = Some("not base64!".to_owned());
    assert_eq!(panel_key(&garbled), None);
}

#[test]
fn the_remote_monitor_carries_the_edids_model_and_size() {
    let edid = edid_block("ABC123", 2);
    let monitor = remote_monitor(&peer_display("D", "HDMI-1", Some(&edid), 0));
    assert_eq!(monitor.pnp_code.as_deref(), Some("DEL4065"));
    assert_eq!(monitor.model_name.as_deref(), Some("DELL U2720Q"));
    assert_eq!(monitor.edid_size_mm, Some((597.0, 336.0)));
    assert_eq!(monitor.frequency, 60);
    assert_eq!(monitor.input_source, Some(0x11));
}

#[test]
fn merging_a_peer_makes_its_view_of_our_panel_a_second_source() {
    let mut layout = local_dell(&edid_block("ABC123", 5));
    let other = edid_block("OTHER", 5);
    merge_peer(
        &mut layout,
        LAPTOP,
        "laptop",
        &[
            peer_display("A", "HDMI-1", Some(&edid_block("ABC123", 2)), 0),
            peer_display("B", "DP-2", Some(&other), 1920),
        ],
    );
    let dell = &layout.monitors()[0];
    assert_eq!(dell.sources.len(), 2);
    assert_eq!(dell.sources[1], remote_source_id(LAPTOP, "A"));
    assert_eq!(layout.monitors().len(), 2);
    assert_eq!(layout.monitors()[1].id, remote_source_id(LAPTOP, "B"));
    // The remote says it is primary there; here the primary is still this machine's.
    assert_eq!(layout.primary_monitor().unwrap().id, dell.id);
}
