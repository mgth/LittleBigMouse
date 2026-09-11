//! The Windows discovery on made-up Win32 answers: the rules of `MonitorDeviceHelper`
//! and `DeviceFactory` — the ids, the order, the duplicates, the connection that counts,
//! the Settings numbers, the specialized monitors, the EDID lookup — down to what the
//! layout model gets. The two tests `WindowsMonitorSafetyTests.cs` has on this code are
//! here too.

use lbm_display::edid;
use lbm_display::windows::ids::{
    disambiguate_source_ids, last_segment, physical_id, pnp_code_from_id, source_id,
};
use lbm_display::windows::registry::{
    decode_key_name_information, find_edid, multi_string, registry_path, single_string, EdidEntry,
    EdidRead, Hive, InvalidKeyName, RegistryValue,
};
use lbm_display::windows::tree::{
    display_mode, flag_specialized, number_monitors, select_connection, ConnectionChoice,
    DeviceCaps, DeviceEntry, DiscoveryError, DpiReading, Luid, RawAdapter, RawDevMode, RawDisplays,
    RawMonitor, RawMonitorInfo, RawRect, Target, ATTACHED_TO_DESKTOP, DM_DISPLAYFREQUENCY,
    DM_DISPLAYORIENTATION, DM_PELSHEIGHT, DM_PELSWIDTH, DM_POSITION,
};
use lbm_display::windows::{display_json, display_signature, dpi_awareness, DisplayTree};
use lbm_layout::model::{DpiAwareness, Layout, LayoutOptions};
use lbm_layout::windows::populate;

const MONITOR_CLASS: &str = "{4d36e96e-e325-11ce-bfc1-08002be10318}";

/// A monitor's device id as `EnumDisplayDevices` gives it.
fn device_id(pnp: &str, instance: &str) -> String {
    format!(r"MONITOR\{pnp}\{MONITOR_CLASS}\{instance}")
}

/// A 128-byte EDID: DEL, 597 x 336 mm, DisplayPort, week `week` of 2020, the serial
/// descriptor `serial` (none when empty), checksum byte `checksum`.
fn edid_bytes(serial: &str, week: u8, checksum: u8) -> Vec<u8> {
    let mut e = vec![0u8; 128];
    e[..8].copy_from_slice(&[0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x00]);
    e[8] = 0x10; // "DEL"
    e[9] = 0xAC;
    e[10] = 0xB1;
    e[11] = 0xA0;
    e[16] = week;
    e[17] = 30; // 2020
    e[18] = 1;
    e[19] = 4;
    e[20] = 0x80 | 0x05;
    e[66] = 0x55; // 597
    e[67] = 0x50; // 336
    e[68] = 0x21;
    let descriptor = |e: &mut Vec<u8>, at: usize, tag: u8, text: &[u8]| {
        e[at..at + 5].copy_from_slice(&[0, 0, 0, tag, 0]);
        e[at + 5..at + 18].fill(0x20);
        e[at + 5..at + 5 + text.len()].copy_from_slice(text);
        e[at + 5 + text.len()] = 0x0A;
    };
    descriptor(&mut e, 72, 0xFC, b"U2720Q");
    if !serial.is_empty() {
        descriptor(&mut e, 90, 0xFF, serial.as_bytes());
    }
    e[127] = checksum;
    e
}

fn edid_entry(id: &str, bytes: Vec<u8>) -> EdidEntry {
    EdidEntry {
        key_name: format!(
            r"\REGISTRY\MACHINE\SYSTEM\ControlSet001\Enum\DISPLAY\{id}\Device Parameters"
        ),
        id: id.to_owned(),
        edid: EdidRead::Bytes(bytes),
    }
}

fn entry(name: &str, string: &str, flags: u32, id: &str) -> DeviceEntry {
    DeviceEntry {
        device_name: name.to_owned(),
        device_string: string.to_owned(),
        state_flags: flags,
        device_id: id.to_owned(),
    }
}

fn mode(x: i32, width: u32, height: u32) -> RawDevMode {
    RawDevMode {
        fields: DM_POSITION
            | DM_PELSWIDTH
            | DM_PELSHEIGHT
            | DM_DISPLAYFREQUENCY
            | DM_DISPLAYORIENTATION,
        position_x: x,
        position_y: 0,
        display_orientation: 0,
        pels_width: width,
        pels_height: height,
        display_frequency: 60,
    }
}

const CAPS: DeviceCaps = DeviceCaps {
    horz_size: 598,
    vert_size: 336,
    horz_res: 2560,
    vert_res: 1440,
    log_pixels_x: 96,
    log_pixels_y: 96,
};

/// Adapter source `\\.\DISPLAY<n>` with the given monitors (device id, attached).
fn adapter(n: u32, current: Option<RawDevMode>, monitors: &[(&str, bool)]) -> RawAdapter {
    let name = format!(r"\\.\DISPLAY{n}");
    RawAdapter {
        device: entry(
            &name,
            "NVIDIA GeForce RTX 3080",
            ATTACHED_TO_DESKTOP,
            r"PCI\VEN_10DE&DEV_2206",
        ),
        capabilities: CAPS,
        current_mode: current,
        monitors: monitors
            .iter()
            .enumerate()
            .map(|(j, (id, attached))| RawMonitor {
                device: entry(
                    &format!(r"{name}\Monitor{j}"),
                    "Generic PnP Monitor",
                    if *attached { ATTACHED_TO_DESKTOP } else { 0 },
                    id,
                ),
                interface_path: interface_path(id),
            })
            .collect(),
    }
}

/// A made-up device interface path, one per device id.
fn interface_path(device_id: &str) -> String {
    format!(
        r"\\?\DISPLAY#{}#{}",
        pnp_code_from_id(device_id),
        last_segment(device_id)
    )
}

fn info(n: u32, x: i32, primary: bool, dpi: u32) -> RawMonitorInfo {
    let reading = DpiReading {
        x: dpi,
        y: dpi,
        succeeded: true,
    };
    RawMonitorInfo {
        device_name: format!(r"\\.\DISPLAY{n}"),
        flags: u32::from(primary),
        monitor: RawRect {
            left: x,
            top: 0,
            right: x + 2560,
            bottom: 1440,
        },
        work: RawRect {
            left: x,
            top: 0,
            right: x + 2560,
            bottom: 1400,
        },
        effective_dpi: reading,
        angular_dpi: reading,
        raw_dpi: DpiReading {
            x: 109,
            y: 109,
            succeeded: true,
        },
    }
}

fn target(high: i32, low: u32, id: u32, path: &str) -> Target {
    Target {
        adapter: Luid {
            low_part: low,
            high_part: high,
        },
        id,
        device_path: path.to_owned(),
        specialized: false,
    }
}

// ---- Ids ---------------------------------------------------------------------------

#[test]
fn the_source_id_is_the_pnp_code_serial_string_week_year_and_checksum() {
    let id = device_id("DEL4065", "0003");
    let e = edid::parse("k", &edid_bytes("ABC123", 12, 0x5A));
    // The PnP code is the device id's, not the EDID's (DELA0B1).
    assert_eq!(pnp_code_from_id(&id), "DEL4065");
    assert_eq!(physical_id(&id, Some(&e)), "DEL4065ABC123_0C_07E4");
    assert_eq!(source_id(&id, Some(&e)), "DEL4065ABC123_0C_07E4_5A");
}

#[test]
fn without_serial_string_the_id_has_none() {
    let id = device_id("DEL4065", "0003");
    let e = edid::parse("k", &edid_bytes("", 1, 0));
    assert_eq!(source_id(&id, Some(&e)), "DEL4065_01_07E4_00");
    // A block too short for the descriptors (C#'s null serial) and the checksum, or
    // for the year.
    let short = edid::parse("k", &edid_bytes("ABC123", 1, 7)[..68]);
    assert_eq!(short.serial_number, None);
    assert_eq!(source_id(&id, Some(&short)), "DEL4065_01_07E4_00");
    let shorter = edid::parse("k", &edid_bytes("ABC123", 1, 7)[..17]);
    assert_eq!(source_id(&id, Some(&shorter)), "DEL4065_01_0000_00");
}

#[test]
fn without_edid_the_id_is_the_pnp_code_and_the_instance() {
    let id = device_id("DEL4065", "0003");
    assert_eq!(physical_id(&id, None), r"NOEDID_DEL4065_0003");
    assert_eq!(source_id(&id, None), r"NOEDID_DEL4065_0003");
    // A device id without segments is its own PnP code and instance.
    assert_eq!(pnp_code_from_id("ROOT"), "ROOT");
    assert_eq!(source_id("ROOT", None), "NOEDID_ROOT_ROOT");
    assert_eq!(source_id("", None), "NOEDID__");
}

#[test]
fn equal_neighbour_source_ids_get_their_instance() {
    let ids = [r"M\X\G\0001", r"M\X\G\0002", r"M\X\G\0003", r"M\Y\G\0004"];
    let mut source_ids: Vec<String> = ["A", "A", "A", "B"].map(String::from).to_vec();
    disambiguate_source_ids(&ids, &mut source_ids);
    assert_eq!(source_ids, ["A_0001", "A_0002", "A_0003", "B"]);

    let mut pairs: Vec<String> = ["A", "A", "B", "B"].map(String::from).to_vec();
    disambiguate_source_ids(&ids, &mut pairs);
    assert_eq!(pairs, ["A_0001", "A_0002", "B_0003", "B_0004"]);
}

/// C#'s scan only compares neighbours: the same id on both sides of another one stays.
#[test]
fn equal_source_ids_that_are_not_neighbours_stay_equal() {
    let ids = [r"M\X\G\0001", r"M\X\G\0002", r"M\X\G\0003"];
    let mut source_ids: Vec<String> = ["A", "B", "A"].map(String::from).to_vec();
    disambiguate_source_ids(&ids, &mut source_ids);
    assert_eq!(source_ids, ["A", "B", "A"]);
}

// ---- Registry ----------------------------------------------------------------------

/// C#: `WindowsMonitorSafetyTests.KeyNameInformationUsesByteLengthWithoutReadingPastBuffer`
#[test]
fn key_name_information_uses_byte_length_without_reading_past_buffer() {
    let path = r"\REGISTRY\MACHINE\SYSTEM\Monitor";
    let name: Vec<u8> = path.encode_utf16().flat_map(u16::to_le_bytes).collect();
    let mut data = (name.len() as u32).to_le_bytes().to_vec();
    data.extend_from_slice(&name);

    assert_eq!(decode_key_name_information(&data).as_deref(), Ok(path));

    data[..4].copy_from_slice(&(name.len() as u32 + 2).to_le_bytes());
    assert_eq!(decode_key_name_information(&data), Err(InvalidKeyName));
}

#[test]
fn an_odd_or_missing_key_name_length() {
    assert_eq!(decode_key_name_information(&[2, 0]).as_deref(), Ok(""));
    assert_eq!(
        decode_key_name_information(&[1, 0, 0, 0, 0x41]),
        Err(InvalidKeyName)
    );
    assert_eq!(
        decode_key_name_information(&[2, 0, 0, 0, 0x41, 0, 0x42, 0]).as_deref(),
        Ok("A")
    );
}

#[test]
fn registry_strings_as_dotnet_reads_them() {
    let units = |s: &str| s.encode_utf16().collect::<Vec<u16>>();
    assert_eq!(
        multi_string(&units("MONITOR\\DEL4065\0\0")),
        ["MONITOR\\DEL4065"]
    );
    assert_eq!(multi_string(&units("A\0\0B\0")), ["A", "", "B"]);
    assert_eq!(multi_string(&units("A")), ["A"]);
    assert_eq!(multi_string(&units("\0")), Vec::<String>::new());
    assert_eq!(multi_string(&units("\0\0")), [""]);
    assert_eq!(multi_string(&[]), Vec::<String>::new());
    // One terminating NUL goes, and only one.
    assert_eq!(single_string(&units("{guid}\\0003\0")), "{guid}\\0003");
    assert_eq!(single_string(&units("x\0\0")), "x\0");
    // What `s + value` makes of each type.
    assert_eq!(RegistryValue::Null.concat_text(), "");
    assert_eq!(RegistryValue::Dword(-3).concat_text(), "-3");
    assert_eq!(
        RegistryValue::Binary(vec![1]).concat_text(),
        "System.Byte[]"
    );
    assert_eq!(
        RegistryValue::MultiString(vec![]).concat_text(),
        "System.String[]"
    );
}

#[test]
fn kernel_key_paths_open_from_their_hive() {
    let path =
        r"\REGISTRY\MACHINE\SYSTEM\ControlSet001\Enum\DISPLAY\DEL4065\5&1&0&UID1\Device Parameters";
    let (hive, keys) = registry_path(path, 1).unwrap();
    assert_eq!(hive, Hive::LocalMachine);
    assert_eq!(
        keys,
        [
            "SYSTEM",
            "ControlSet001",
            "Enum",
            "DISPLAY",
            "DEL4065",
            "5&1&0&UID1"
        ]
    );
    assert_eq!(
        registry_path(path, 0).unwrap().1.last(),
        Some(&"Device Parameters")
    );
    assert_eq!(
        registry_path(r"\REGISTRY\USER\S-1", 0).unwrap().0,
        Hive::CurrentUser
    );
    assert_eq!(
        registry_path(r"\REGISTRY\CONFIG", 0),
        Some((Hive::CurrentConfig, vec![]))
    );
    assert_eq!(
        registry_path(r"\REGISTRY\MACHINE", 1),
        Some((Hive::LocalMachine, vec![]))
    );
    assert_eq!(registry_path(r"\REGISTRY", 0), None);
}

#[test]
fn the_first_readable_entry_with_the_id_gives_the_edid() {
    let id = device_id("DEL4065", "0003");
    let other = device_id("DEL4065", "0004");
    let good = edid_bytes("GOOD", 1, 1);
    // A block C#'s parser throws on: a name descriptor running past 100 bytes.
    let mut throws = vec![0u8; 100];
    throws[90..95].copy_from_slice(&[0, 0, 0, 0xFC, 0]);
    throws[95..100].fill(b'A');
    assert!(edid::csharp_throws(&throws));

    let failed = EdidEntry {
        edid: EdidRead::Failed,
        ..edid_entry(&id, vec![])
    };
    let table = vec![
        edid_entry(&other, edid_bytes("OTHER", 1, 1)),
        failed,
        edid_entry(&id, throws),
        edid_entry(&id, good.clone()),
        edid_entry(&id, edid_bytes("LATER", 1, 1)),
    ];
    let found = find_edid(&table, &id).unwrap();
    assert_eq!(found.serial_number.as_deref(), Some("GOOD"));
    assert!(found.key.ends_with(r"\Device Parameters"));

    // A device with the id but no EDID value ends the search: no EDID.
    let missing = EdidEntry {
        edid: EdidRead::Missing,
        ..edid_entry(&id, vec![])
    };
    assert_eq!(find_edid(&[missing, edid_entry(&id, good)], &id), None);
    assert_eq!(find_edid(&[], &id), None);
}

#[test]
fn csharp_throws_where_a_descriptor_runs_past_the_end() {
    let block = |len: usize, at: usize, header: &[u8]| {
        let mut e = vec![0x20u8; len];
        e[at..at + header.len()].copy_from_slice(header);
        e
    };
    // Header complete, text past the end.
    assert!(edid::csharp_throws(&block(80, 72, &[0, 0, 0, 0xFF, 0])));
    // Text ended by a line feed before the end: fine.
    let mut ended = block(80, 72, &[0, 0, 0, 0xFF, 0]);
    ended[78] = 0x0A;
    assert!(!edid::csharp_throws(&ended));
    // A header cut short: C# reads its next byte past the end.
    assert!(edid::csharp_throws(&block(74, 72, &[0, 0])));
    // A header that differs before the end: skipped.
    assert!(!edid::csharp_throws(&block(74, 72, &[0, 1])));
    // Too short to reach the descriptors, or a whole block.
    assert!(!edid::csharp_throws(&[0; 68]));
    assert!(!edid::csharp_throws(&edid_bytes("X", 1, 1)));
}

// ---- The tree ------------------------------------------------------------------------

#[test]
fn a_mode_reads_what_the_driver_filled() {
    let mut dm = mode(-1920, 2560, 1440);
    dm.display_orientation = 3;
    let m = display_mode(&dm);
    assert_eq!((m.position.x, m.position.y), (-1920.0, 0.0));
    assert_eq!((m.pels.width(), m.pels.height()), (2560.0, 1440.0));
    assert_eq!((m.display_orientation, m.display_frequency), (3, 60));

    let empty = display_mode(&RawDevMode { fields: 0, ..dm });
    assert_eq!((empty.position.x, empty.position.y), (0.0, 0.0));
    assert_eq!((empty.pels.width(), empty.pels.height()), (1.0, 1.0));
    assert_eq!((empty.display_orientation, empty.display_frequency), (0, 0));

    // Either size bit brings both.
    let width_only = display_mode(&RawDevMode {
        fields: DM_PELSWIDTH,
        ..dm
    });
    assert_eq!(
        (width_only.pels.width(), width_only.pels.height()),
        (2560.0, 1440.0)
    );
}

/// C#: `WindowsMonitorSafetyTests.ActiveConnectionWinsRegardlessOfEnumerationOrder`
#[test]
fn active_connection_wins_regardless_of_enumeration_order() {
    let stale = ConnectionChoice {
        attached_to_desktop: false,
        has_monitor: false,
        has_mode: false,
        adapter_name: r"\.\DISPLAY9",
    };
    let active = ConnectionChoice {
        attached_to_desktop: true,
        has_monitor: true,
        has_mode: true,
        adapter_name: r"\.\DISPLAY2",
    };

    assert_eq!(select_connection([stale, active]), Some(1));
    assert_eq!(select_connection([active, stale]), Some(0));
}

#[test]
fn the_connection_choice_falls_back_by_monitor_then_mode_then_name() {
    let choice = |attached, has_monitor, has_mode, name| ConnectionChoice {
        attached_to_desktop: attached,
        has_monitor,
        has_mode,
        adapter_name: name,
    };
    let d10 = choice(false, false, false, r"\\.\DISPLAY10");
    let d2 = choice(false, false, false, r"\\.\display2");
    assert_eq!(select_connection([d2, d10]), Some(1), "ordinal: 1 before 2");
    let with_mode = choice(false, false, true, r"\\.\DISPLAY9");
    assert_eq!(select_connection([d10, with_mode]), Some(1));
    let with_monitor = choice(false, true, false, r"\\.\DISPLAY9");
    assert_eq!(select_connection([with_mode, with_monitor]), Some(1));
    // A tie keeps the first.
    assert_eq!(select_connection([d10, d10]), Some(0));
    assert_eq!(select_connection(std::iter::empty()), None);
}

/// Two sources, a Dell on the primary and a Samsung without EDID on the second, both
/// accounted for by CCD.
fn dual() -> RawDisplays {
    let dell = device_id("DEL4065", "0001");
    let samsung = device_id("SAM0F9E", "0002");
    RawDisplays {
        adapters: vec![
            adapter(1, Some(mode(0, 2560, 1440)), &[(&dell, true)]),
            adapter(2, Some(mode(2560, 2560, 1440)), &[(&samsung, true)]),
        ],
        monitor_infos: vec![info(1, 0, true, 144), info(2, 2560, false, 96)],
        targets: Some(vec![
            target(0, 0x100, 1, &interface_path(&samsung)),
            target(0, 0x100, 0, &interface_path(&dell)),
        ]),
        edids: vec![edid_entry(&dell, edid_bytes("ABC123", 12, 0x5A))],
    }
}

#[test]
fn a_dual_monitor_machine() {
    let tree = DisplayTree::assemble(dual()).unwrap();
    let monitors: Vec<_> = tree.monitors().collect();
    // Sorted by physical id: DEL4065ABC123... before NOEDID_...
    assert_eq!(monitors.len(), 2);
    assert_eq!(monitors[0].source_id, "DEL4065ABC123_0C_07E4_5A");
    assert_eq!(monitors[0].monitor_number, "1");
    assert_eq!(monitors[1].source_id, "NOEDID_SAM0F9E_0002");
    assert_eq!(monitors[1].monitor_number, "2");
    assert!(monitors.iter().all(|m| !m.is_specialized));

    let (adapter, connection) = tree.active_connection(monitors[0]).unwrap();
    assert_eq!(adapter.adapter.device_name, r"\\.\DISPLAY1");
    assert!(adapter.has_monitor && adapter.adapter.primary);
    assert_eq!(adapter.adapter.effective_dpi.x, 144.0);
    assert_eq!(adapter.adapter.raw_dpi.y, 109.0);
    assert_eq!(connection.device.device_name, r"\\.\DISPLAY1\Monitor0");

    let input = tree.layout_input();
    assert_eq!(input[0].pnp_code, "DEL4065");
    let edid = input[0].edid.as_ref().unwrap();
    assert_eq!(edid.serial_number.as_deref(), Some("ABC123"));
    assert_eq!(edid.video_interface.as_deref(), Some("DisplayPort"));
    let caps = input[0]
        .active_connection
        .as_ref()
        .unwrap()
        .adapter
        .capabilities;
    assert_eq!((caps.size.width(), caps.log_pixels.height()), (598.0, 96.0));
    assert!(input[1].edid.is_none());

    let mut layout = Layout::new(LayoutOptions::default());
    populate(&mut layout, DpiAwareness::PerMonitorAware, &input, |_| {
        Ok::<(), ()>(())
    })
    .unwrap();
    assert_eq!(layout.id, "DEL4065ABC123_0C_07E4_5A+NOEDID_SAM0F9E_0002");

    let json = display_json(&tree, monitors[0]);
    assert_eq!(json["SourceId"], "DEL4065ABC123_0C_07E4_5A");
    assert_eq!(json["Edid"]["Week"], 12);
    assert_eq!(
        json["ActiveConnection"]["Adapter"]["CurrentMode"]["Width"],
        2560.0
    );
    assert!(display_json(&tree, monitors[1])["Edid"].is_null());
}

/// A detached monitor is listed below every inactive source: one monitor, one
/// connection per source, the first source by name counts.
#[test]
fn a_detached_monitor_below_every_inactive_source_is_one_monitor() {
    let dell = device_id("DEL4065", "0001");
    let lg = device_id("GSM5B7F", "0005");
    let mut raw = dual();
    let mut late = adapter(4, None, &[(&lg, false)]);
    late.monitors[0].interface_path = interface_path(&lg);
    let mut early = adapter(3, None, &[(&lg, false)]);
    // The first listing has no interface path: the later one fills it.
    early.monitors[0].interface_path = String::new();
    raw.adapters.push(early);
    raw.adapters.push(late);
    let tree = DisplayTree::assemble(raw).unwrap();

    let monitors: Vec<_> = tree.monitors().collect();
    assert_eq!(monitors.len(), 3);
    let lg_monitor = monitors.iter().find(|m| m.id == lg).unwrap();
    assert_eq!(lg_monitor.connections.len(), 2);
    assert_eq!(lg_monitor.interface_path, interface_path(&lg));
    let (adapter, _) = tree.active_connection(lg_monitor).unwrap();
    assert_eq!(adapter.adapter.device_name, r"\\.\DISPLAY3");
    assert!(!adapter.has_monitor);
    // No target: numbered after the ones CCD accounts for.
    assert_eq!(lg_monitor.monitor_number, "3");
    let input = tree.layout_input();
    let lg_input = input.iter().find(|m| m.id == lg).unwrap();
    let connection = lg_input.active_connection.as_ref().unwrap();
    assert!(!connection.attached_to_desktop);
    assert!(connection.adapter.current_mode.is_none());
    assert_eq!(connection.adapter.effective_dpi.x, 0.0);
    assert!(input.iter().any(|m| m.id == dell));
}

/// C# looks a monitor up among the sources already in the tree, not the one it is
/// building: an id listed twice below one source makes two monitor devices, and only
/// the first is enumerated.
#[test]
fn an_id_listed_twice_below_one_source_makes_two_devices() {
    let dell = device_id("DEL4065", "0001");
    let raw = RawDisplays {
        adapters: vec![
            adapter(
                1,
                Some(mode(0, 2560, 1440)),
                &[(&dell, false), (&dell, true)],
            ),
            adapter(2, None, &[(&dell, false)]),
        ],
        monitor_infos: vec![info(1, 0, true, 96)],
        targets: None,
        edids: vec![],
    };
    let tree = DisplayTree::assemble(raw).unwrap();
    assert_eq!(tree.devices.len(), 2);
    let monitors: Vec<_> = tree.monitors().collect();
    assert_eq!(monitors.len(), 1);
    // The first device has the first connection below DISPLAY1 and the one below
    // DISPLAY2; the attached second connection belongs to the other device.
    let first = monitors[0];
    assert_eq!(first.connections.len(), 2);
    let (adapter, connection) = tree.active_connection(first).unwrap();
    assert_eq!(adapter.adapter.device_name, r"\\.\DISPLAY1");
    assert!(!connection.attached_to_desktop());
    assert_eq!(connection.device.device_name, r"\\.\DISPLAY1\Monitor0");
}

#[test]
fn identical_monitors_get_their_instance_appended() {
    let a = device_id("DEL4065", "0001");
    let b = device_id("DEL4065", "0002");
    let raw = RawDisplays {
        adapters: vec![
            adapter(1, Some(mode(0, 2560, 1440)), &[(&a, true)]),
            adapter(2, Some(mode(2560, 2560, 1440)), &[(&b, true)]),
        ],
        monitor_infos: vec![info(1, 0, true, 96), info(2, 2560, false, 96)],
        targets: None,
        edids: vec![
            edid_entry(&a, edid_bytes("", 5, 0x11)),
            edid_entry(&b, edid_bytes("", 5, 0x11)),
        ],
    };
    let tree = DisplayTree::assemble(raw).unwrap();
    let ids: Vec<&str> = tree.monitors().map(|m| m.source_id.as_str()).collect();
    assert_eq!(ids, ["DEL4065_05_07E4_11_0001", "DEL4065_05_07E4_11_0002"]);
    let numbers: Vec<&str> = tree.monitors().map(|m| m.monitor_number.as_str()).collect();
    assert_eq!(numbers, ["1", "2"]);
}

/// The enumeration order is C#'s `OrderBy(PhysicalId)` with the (invariant) culture's
/// comparer, where case only breaks ties: an ordinal sort would put `B` before `a`.
#[test]
fn monitors_are_ordered_by_physical_id_with_the_culture_comparer() {
    let lower = device_id("DEL", "a");
    let upper = device_id("DEL", "B");
    let raw = RawDisplays {
        adapters: vec![adapter(1, None, &[(&upper, true), (&lower, true)])],
        ..Default::default()
    };
    let tree = DisplayTree::assemble(raw).unwrap();
    let ids: Vec<&str> = tree.monitors().map(|m| m.physical_id.as_str()).collect();
    assert_eq!(ids, ["NOEDID_DEL_a", "NOEDID_DEL_B"]);
}

#[test]
fn settings_numbers_follow_the_targets_by_adapter_luid_then_id() {
    let paths = ["igpu-0", "dgpu-1", "virtual", "dgpu-0"];
    let targets = [
        // The iGPU comes first in the path array and has the lowest target id...
        target(0, 0x9000, 0, "igpu-0"),
        target(0, 0x100, 7, "dgpu-1"),
        target(0, 0x100, 3, "dgpu-0"),
        // ...a connected target with no monitor of ours still takes a number...
        target(0, 0x100, 5, "nobody"),
        target(0, 0x100, 3, "dgpu-0"),
    ];
    // ...and the dGPU (lower LUID) is numbered first.
    assert_eq!(
        number_monitors(&paths, Some(&targets)),
        ["4", "3", "5", "1"]
    );
    // A high part weighs before the low part.
    let by_high = [target(1, 0, 0, "a"), target(0, 0xFFFF_FFFF, 0, "b")];
    assert_eq!(number_monitors(&["a", "b"], Some(&by_high)), ["2", "1"]);
    // No CCD: enumeration order.
    assert_eq!(number_monitors(&paths, None), ["1", "2", "3", "4"]);
    // A target without a path takes its number and matches nothing.
    assert_eq!(number_monitors(&[""], Some(&[target(0, 0, 0, "")])), ["2"]);
}

#[test]
fn a_specialized_target_off_the_desktop_is_a_specialized_monitor() {
    let mut headset = target(0, 1, 0, "hmd");
    headset.specialized = true;
    let mut desktop = target(0, 1, 1, "panel");
    desktop.specialized = true;
    let targets = [headset, desktop, target(0, 1, 2, "plain")];
    // #364: the headset has no HMONITOR; #506: a monitor the desktop shows is not
    // specialized, whatever the query says.
    assert_eq!(
        flag_specialized(
            &["hmd", "panel", "plain"],
            &[false, true, false],
            Some(&targets)
        ),
        [true, false, false]
    );
    assert_eq!(flag_specialized(&["hmd"], &[false], None), [false]);
}

#[test]
fn a_specialized_headset_stays_out_of_the_layout() {
    let dell = device_id("DEL4065", "0001");
    let hmd = device_id("HVR0001", "0009");
    let mut raw = dual();
    raw.adapters.push(adapter(3, None, &[(&hmd, false)]));
    let mut headset = target(0, 0x100, 9, &interface_path(&hmd));
    headset.specialized = true;
    raw.targets.as_mut().unwrap().push(headset);
    let tree = DisplayTree::assemble(raw).unwrap();
    let input = tree.layout_input();
    assert!(input.iter().find(|m| m.id == hmd).unwrap().is_specialized);

    let mut layout = Layout::new(LayoutOptions::default());
    populate(&mut layout, DpiAwareness::PerMonitorAware, &input, |_| {
        Ok::<(), ()>(())
    })
    .unwrap();
    assert_eq!(layout.monitors().len(), 2);
    assert!(layout
        .monitors()
        .iter()
        .all(|m| m.device_id.as_deref() != Some(hmd.as_str())));
    assert!(layout
        .monitors()
        .iter()
        .any(|m| m.device_id.as_deref() == Some(dell.as_str())));
}

#[test]
fn negative_sizes_fail_as_hlab_geo_throws() {
    let mut raw = dual();
    raw.adapters[1].capabilities.horz_size = -1;
    assert!(matches!(
        DisplayTree::assemble(raw),
        Err(DiscoveryError::NegativeSize { width: -1, .. })
    ));

    let mut raw = dual();
    raw.monitor_infos[0].work.right = -5;
    let error = DisplayTree::assemble(raw).unwrap_err();
    assert!(error.to_string().contains("work area"), "{error}");

    // A monitor info for a source that is not in the tree is not read.
    let mut raw = dual();
    let mut stray = info(7, 0, false, 96);
    stray.monitor.right = -5;
    raw.monitor_infos.push(stray);
    assert!(DisplayTree::assemble(raw).is_ok());
}

#[test]
fn no_display_at_all() {
    let tree = DisplayTree::assemble(RawDisplays::default()).unwrap();
    assert_eq!(tree.monitors().count(), 0);
    assert!(tree.layout_input().is_empty());
}

// ---- Signature, awareness -----------------------------------------------------------------

#[test]
fn the_display_signature_lists_sources_sorted() {
    let mut failed = info(1, -2560, false, 144);
    failed.effective_dpi.succeeded = false;
    let signature = display_signature(&[info(2, 0, true, 96), failed]);
    assert_eq!(
        signature,
        r"\\.\DISPLAY1[-2560,0 2560x1440]d0|\\.\DISPLAY2[0,0 2560x1440]*d96"
    );
    assert_eq!(display_signature(&[]), "");
}

#[test]
fn dpi_awareness_values() {
    assert_eq!(dpi_awareness(-1), DpiAwareness::Invalid);
    assert_eq!(dpi_awareness(0), DpiAwareness::Unaware);
    assert_eq!(dpi_awareness(1), DpiAwareness::SystemAware);
    assert_eq!(dpi_awareness(2), DpiAwareness::PerMonitorAware);
    assert_eq!(dpi_awareness(3), DpiAwareness::Invalid);
}
