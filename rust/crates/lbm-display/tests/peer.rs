//! A remote machine's report of its screens, read the way this machine reads its own
//! (docs/multi-pc.md, "Model").

use base64::Engine as _;
use lbm_display::edid;
use lbm_display::peer::{merge_peer_as, panel_key, remote_monitor_as, IdForm};
use lbm_display::windows::tree::MonitorDevice;
use lbm_ipc::peer::PeerDisplay;
use lbm_layout::linux::{add_monitor, LinuxMonitor};
use lbm_layout::model::{Layout, LayoutOptions};
use lbm_layout::remote::remote_source_id;
use lbm_layout::windows::{add_or_update_monitor_device, WindowsMonitor};

const LAPTOP: &str = "a1b2c3d4e5f60718293a4b5c6d7e8f90a1b2c3d4e5f60718293a4b5c6d7e8f90";

/// A 128-byte EDID block: the manufacturer, product and binary serial of the basic
/// block, the image size of a first detailed timing, and the serial and name
/// descriptors. `interface` is the low nibble of byte 20 (5: DisplayPort, 2: HDMI) —
/// what differs between two inputs of one monitor, with the checksum.
fn edid_block(serial_string: &str, interface: u8) -> Vec<u8> {
    edid_block_of(
        "DEL",
        0x4065,
        [0x01, 0x02, 0x03, 0x04],
        serial_string,
        interface,
    )
}

/// [`edid_block`] of any make: a three-letter manufacturer, a product code, and the
/// binary serial's bytes as they sit in the block.
fn edid_block_of(
    manufacturer: &str,
    product: u16,
    binary_serial: [u8; 4],
    serial_string: &str,
    interface: u8,
) -> Vec<u8> {
    let mut b = vec![0u8; 128];
    b[..8].copy_from_slice(&[0, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0]);
    // Five-bit letters, A = 1.
    let letters = manufacturer
        .bytes()
        .fold(0u16, |acc, c| (acc << 5) | u16::from(c - b'A' + 1));
    b[8] = (letters >> 8) as u8;
    b[9] = letters as u8;
    b[10] = product as u8;
    b[11] = (product >> 8) as u8;
    b[12..16].copy_from_slice(&binary_serial);
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
    let monitor = remote_monitor_as(&peer_display("D", "HDMI-1", Some(&edid), 0), IdForm::Linux);
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
    merge_peer_as(
        &mut layout,
        LAPTOP,
        "laptop",
        &[
            peer_display("A", "HDMI-1", Some(&edid_block("ABC123", 2)), 0),
            peer_display("B", "DP-2", Some(&other), 1920),
        ],
        IdForm::Linux,
    );
    let dell = &layout.monitors()[0];
    assert_eq!(dell.sources.len(), 2);
    assert_eq!(dell.sources[1], remote_source_id(LAPTOP, "A"));
    assert_eq!(layout.monitors().len(), 2);
    assert_eq!(
        layout.monitors()[1].id,
        "DEL4065_OTHER",
        "a panel only the peer has is named by its key"
    );
    assert_eq!(
        layout.monitors()[1].sources,
        vec![remote_source_id(LAPTOP, "B")]
    );
    // The remote says it is primary there; here the primary is still this machine's.
    assert_eq!(layout.primary_monitor().unwrap().id, dell.id);
}

#[test]
fn the_layout_id_is_the_same_whichever_machine_shows_a_kvm_switched_panel() {
    // The maintainer's Samsung behind a hardware KVM: serial string H1AK500000, binary
    // serial 0, here on HDMI, there (Windows) under its device id.
    let samsung = edid_block_of("SAM", 0xE035, [0; 4], "H1AK500000", 2);
    let dell = edid_block("ABC123", 5);
    let output = |connector: &str, x: f64, edid: &[u8]| LinuxMonitor {
        connector_name: connector.to_owned(),
        logical_x: x,
        logical_y: 0.0,
        logical_width: 2560.0,
        logical_height: 1440.0,
        pixel_width: 2560,
        pixel_height: 1440,
        scale: 1.0,
        width_mm: 597.0,
        height_mm: 336.0,
        primary: x == 0.0,
        enabled: true,
        orientation: 0,
        frequency: 60,
        edid: Some(edid::parse(format!("card0-{connector}"), edid).to_linux()),
    };

    let mut here = Layout::new(LayoutOptions::default());
    add_monitor(&mut here, &output("DP-1", 0.0, &dell));
    add_monitor(&mut here, &output("HDMI-A-1", 2560.0, &samsung));

    let mut there = Layout::new(LayoutOptions::default());
    add_monitor(&mut there, &output("DP-1", 0.0, &dell));
    let device = r"MONITOR\SAME035\{4d36e96e-e325-11ce-bfc1-08002be10318}\0010";
    let shown_there = peer_display(device, r"\\.\DISPLAY1", Some(&samsung), 0);
    assert_eq!(
        panel_key(&shown_there).as_deref(),
        Some("SAME035_H1AK500000")
    );
    merge_peer_as(&mut there, LAPTOP, "windows", &[shown_there], IdForm::Linux);

    assert_eq!(here.compute_id(), "DEL4065_ABC123+SAME035_H1AK500000");
    assert_eq!(there.compute_id(), here.compute_id());
    assert_eq!(
        there.monitor("SAME035_H1AK500000").unwrap().sources,
        vec![remote_source_id(LAPTOP, device)]
    );
}

/// A monitor as Windows detection hands it to the layout: its device id, and the ids
/// `MonitorDevice` makes of it and its EDID.
fn windows_monitor(device_id: &str, edid_bytes: &[u8]) -> WindowsMonitor {
    let parsed = edid::parse(device_id, edid_bytes);
    let device = MonitorDevice::new(device_id, "", Some(parsed.clone()));
    WindowsMonitor {
        id: device_id.to_owned(),
        pnp_code: device.pnp_code,
        source_id: device.source_id,
        interface_path: String::new(),
        monitor_number: String::new(),
        is_specialized: false,
        edid: Some(parsed.to_windows()),
        active_connection: None,
    }
}

const WINDOWS_SAMSUNG: &str = r"MONITOR\SAME035\{4d36e96e-e325-11ce-bfc1-08002be10318}\0010";
const WINDOWS_DELL: &str = r"MONITOR\DEL4065\{4d36e96e-e325-11ce-bfc1-08002be10318}\0003";

#[test]
fn a_windows_master_names_a_remote_panel_as_windows_names_it_here() {
    // The Samsung behind the KVM, read by the Linux remote through the same KVM port:
    // the same block Windows reads when the KVM points at it.
    let samsung = edid_block_of("SAM", 0xE035, [0; 4], "H1AK500000", 2);
    let dell = edid_block("ABC123", 5);

    let mut here = Layout::new(LayoutOptions::default());
    add_or_update_monitor_device(&mut here, &windows_monitor(WINDOWS_DELL, &dell));
    add_or_update_monitor_device(&mut here, &windows_monitor(WINDOWS_SAMSUNG, &samsung));

    let mut there = Layout::new(LayoutOptions::default());
    add_or_update_monitor_device(&mut there, &windows_monitor(WINDOWS_DELL, &dell));
    let shown_there = peer_display("SAME035_H1AK500000", "HDMI-A-1", Some(&samsung), 0);
    let remote = remote_monitor_as(&shown_there, IdForm::Windows);
    let local_samsung = &here.monitors()[1].id;
    assert_eq!(remote.local_id.as_ref(), Some(local_samsung));
    merge_peer_as(&mut there, LAPTOP, "linux", &[shown_there], IdForm::Windows);

    assert_eq!(
        there.compute_id(),
        here.compute_id(),
        "one profile both ways"
    );
    let shown = there.monitor(local_samsung).unwrap();
    assert_eq!(
        shown.sources,
        vec![remote_source_id(LAPTOP, "SAME035_H1AK500000")]
    );
    assert_eq!(shown.model, here.monitors()[1].model, "and one model");
}

#[test]
fn a_windows_master_without_a_serial_string_still_keeps_one_id() {
    // No serial string, so no panel key: the Windows id has the week, year and
    // checksum, which a KVM passes through unchanged.
    let samsung = edid_block_of("SAM", 0xE035, [0; 4], "", 2);
    let shown_there = peer_display("HDMI-A-1", "HDMI-A-1", Some(&samsung), 0);
    assert_eq!(panel_key(&shown_there), None);

    let mut here = Layout::new(LayoutOptions::default());
    add_or_update_monitor_device(&mut here, &windows_monitor(WINDOWS_SAMSUNG, &samsung));
    let mut there = Layout::new(LayoutOptions::default());
    merge_peer_as(&mut there, LAPTOP, "linux", &[shown_there], IdForm::Windows);
    assert_eq!(there.compute_id(), here.compute_id());
}

#[test]
fn a_windows_id_already_taken_here_falls_back_to_the_source_id() {
    // Two panels of one model, made the same week, with no serial string: one EDID,
    // one Windows id, no panel key. The one shown here keeps the id; the remote's,
    // which nothing tells apart from it, gets an id no local monitor can have.
    let samsung = edid_block_of("SAM", 0xE035, [0; 4], "", 2);
    let mut layout = Layout::new(LayoutOptions::default());
    add_or_update_monitor_device(&mut layout, &windows_monitor(WINDOWS_SAMSUNG, &samsung));
    let local = layout.monitors()[0].id.clone();
    let twin = peer_display("HDMI-A-1", "HDMI-A-1", Some(&samsung), 0);
    merge_peer_as(&mut layout, LAPTOP, "linux", &[twin], IdForm::Windows);
    assert_eq!(layout.monitors().len(), 2);
    assert_eq!(layout.monitors()[0].id, local);
    assert_eq!(
        layout.monitors()[1].id,
        remote_source_id(LAPTOP, "HDMI-A-1")
    );
}

#[test]
fn a_truncated_edid_gives_no_windows_id_and_the_monitor_its_fallback() {
    // Well-formed base64 of the first 20 bytes: the manufacturer is there, the
    // product, week, year and checksum are not.
    let whole = edid_block_of("SAM", 0xE035, [0; 4], "H1AK500000", 2);
    let shown_there = peer_display("HDMI-A-1", "HDMI-A-1", Some(&whole[..20]), 0);
    let remote = remote_monitor_as(&shown_there, IdForm::Windows);
    assert_eq!(remote.local_id, None);

    let mut layout = Layout::new(LayoutOptions::default());
    merge_peer_as(
        &mut layout,
        LAPTOP,
        "linux",
        &[shown_there],
        IdForm::Windows,
    );
    assert_eq!(
        layout.monitors()[0].id,
        remote_source_id(LAPTOP, "HDMI-A-1")
    );

    // The whole block gives it.
    let shown_whole = peer_display("HDMI-A-1", "HDMI-A-1", Some(&whole), 0);
    assert!(remote_monitor_as(&shown_whole, IdForm::Windows)
        .local_id
        .is_some());
}

/// The EDIDs of one Samsung Odyssey G80SD, read through its two inputs: here on
/// DisplayPort (sysfs), on a Windows machine on HDMI (the registry). Samsung gives
/// each input its own product code, and the HDMI block a first detailed timing of
/// 1872 x 1053 mm for a 70 x 39 cm panel.
const G80SD_DISPLAYPORT: &[u8] = include_bytes!("data/g80sd-displayport.edid");
const G80SD_HDMI: &[u8] = include_bytes!("data/g80sd-hdmi.edid");

#[test]
fn the_g80sd_has_two_panel_keys_and_one_panel_identity() {
    let dp = edid::parse("dp", G80SD_DISPLAYPORT);
    let hdmi = edid::parse("hdmi", G80SD_HDMI);
    assert_eq!(dp.panel_key().as_deref(), Some("SAME035_H1AK500000"));
    assert_eq!(hdmi.panel_key().as_deref(), Some("SAME031_H1AK500000"));
    assert_eq!(
        dp.panel_identity().as_deref(),
        Some("SAM/Odyssey G80SD_H1AK500000")
    );
    assert_eq!(dp.panel_identity(), hdmi.panel_identity());
}

#[test]
fn a_detailed_timing_that_contradicts_the_basic_block_gives_way_to_it() {
    let dp = edid::parse("dp", G80SD_DISPLAYPORT);
    let hdmi = edid::parse("hdmi", G80SD_HDMI);
    // C#'s members are left as C# reads them…
    assert_eq!(
        (hdmi.physical_width, hdmi.physical_height),
        (1872.0, 1053.0)
    );
    // …the size the mappings take is not.
    assert_eq!(hdmi.image_size_mm(), (700.0, 390.0));
    assert_eq!(hdmi.to_linux().physical_width, 700.0);
    assert_eq!(hdmi.to_windows().physical_height, 390.0);
    // A timing that agrees with the centimetres keeps its millimetres.
    assert_eq!(dp.image_size_mm(), (697.0, 392.0));

    let monitor = remote_monitor_as(
        &peer_display("SAME031", "HDMI-1", Some(G80SD_HDMI), 0),
        IdForm::Linux,
    );
    assert_eq!(monitor.edid_size_mm, Some((700.0, 390.0)));
}

#[test]
fn a_timing_without_a_size_or_a_basic_block_without_one_decide_nothing_wrong() {
    let mut block = G80SD_HDMI[..128].to_vec();
    // No size in the timing: the centimetres.
    block[66] = 0;
    block[67] = 0;
    block[68] = 0;
    assert_eq!(edid::parse("x", &block).image_size_mm(), (700.0, 390.0));
    // No size in the basic block: the timing, whatever it says.
    let mut block = G80SD_HDMI[..128].to_vec();
    block[21] = 0;
    block[22] = 0;
    assert_eq!(edid::parse("x", &block).image_size_mm(), (1872.0, 1053.0));
}

#[test]
fn merging_the_g80sd_seen_on_hdmi_there_makes_a_second_source_of_it_here() {
    let mut layout = local_dell(G80SD_DISPLAYPORT);
    merge_peer_as(
        &mut layout,
        LAPTOP,
        "laptop",
        &[peer_display("SAME031", "HDMI-1", Some(G80SD_HDMI), 0)],
        IdForm::Linux,
    );
    assert_eq!(layout.monitors().len(), 1);
    assert_eq!(
        layout.monitors()[0].sources,
        vec![
            "SAME035_H1AK500000".to_owned(),
            remote_source_id(LAPTOP, "SAME031")
        ]
    );
}
