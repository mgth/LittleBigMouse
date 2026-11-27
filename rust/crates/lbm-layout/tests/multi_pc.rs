//! A remote machine's screens in this machine's layout (docs/multi-pc.md, "Model"):
//! the merge, the shared panel's active source, the profile, local-only helpers, and
//! the zones the engine routes on across machines.

mod common;

use common::design_options;
use lbm_layout::geo::Rect;
use lbm_layout::linux::{add_monitor, LinuxEdid, LinuxMonitor};
use lbm_layout::model::{panel_key, Layout, HOST_GAP_PX};
use lbm_layout::remote::{merge_remote, remote_source_id, RemoteMonitor};
use lbm_layout::zoning::{compute_zones, ZonesLayout};

const LAPTOP: &str = "a1b2c3d4e5f60718293a4b5c6d7e8f90a1b2c3d4e5f60718293a4b5c6d7e8f90";
const DESKTOP: &str = "0f1e2d3c4b5a69788796a5b4c3d2e1f00f1e2d3c4b5a69788796a5b4c3d2e1f0";

fn edid(manufacturer: &str, product: &str, serial: &str, w: f64, h: f64) -> LinuxEdid {
    LinuxEdid {
        manufacturer_code: Some(manufacturer.to_owned()),
        product_code: Some(product.to_owned()),
        serial: Some("00000000".to_owned()),
        serial_number: Some(serial.to_owned()),
        model: Some(format!("{manufacturer} {product}")),
        physical_width: w,
        physical_height: h,
        video_interface: Some("DisplayPort".to_owned()),
    }
}

fn output(connector: &str, x: f64, width: f64, height: f64, edid: LinuxEdid) -> LinuxMonitor {
    LinuxMonitor {
        connector_name: connector.to_owned(),
        logical_x: x,
        logical_y: 0.0,
        logical_width: width,
        logical_height: height,
        pixel_width: width as i32,
        pixel_height: height as i32,
        scale: 1.0,
        width_mm: edid.physical_width,
        height_mm: edid.physical_height,
        primary: x == 0.0,
        enabled: true,
        orientation: 0,
        frequency: 60,
        edid: Some(edid),
    }
}

/// This machine: a Dell at the origin (the panel the laptop shares) and a Samsung to
/// its right.
fn local() -> Layout {
    let mut layout = Layout::new(design_options());
    add_monitor(
        &mut layout,
        &output(
            "DP-1",
            0.0,
            2560.0,
            1440.0,
            edid("DEL", "4065", "ABC123", 597.0, 336.0),
        ),
    );
    add_monitor(
        &mut layout,
        &output(
            "DP-2",
            2560.0,
            1920.0,
            1080.0,
            edid("SAM", "1234", "XYZ", 527.0, 296.0),
        ),
    );
    layout
}

fn remote(id: &str, connector: &str, key: Option<&str>, x: i32, w: i32, h: i32) -> RemoteMonitor {
    RemoteMonitor {
        id: id.to_owned(),
        connector: connector.to_owned(),
        name: connector.to_owned(),
        panel_key: key.map(str::to_owned),
        pnp_code: key.map(|k| k.split('_').next().unwrap().to_owned()),
        x,
        y: 0,
        width: w,
        height: h,
        scale: 1.0,
        attached: true,
        width_mm: 302.0,
        height_mm: 188.0,
        input_source: None,
        ..RemoteMonitor::default()
    }
}

/// The laptop: the Dell on its HDMI port, and its own panel, left of its primary as
/// it sees them (negative desktop coordinates), and primary on its side.
fn laptop() -> Vec<RemoteMonitor> {
    let mut panel = remote("LEN40A0_7", "eDP-1", Some("LEN40A0_7"), -1920, 1920, 1200);
    panel.input_source = None;
    let mut dell = remote(
        "DEL4065_ABC123",
        "HDMI-1",
        Some("DEL4065_ABC123"),
        0,
        1920,
        1080,
    );
    dell.input_source = Some(0x11);
    vec![panel, dell]
}

fn with_laptop() -> Layout {
    let mut layout = local();
    merge_remote(&mut layout, LAPTOP, "laptop", &laptop());
    layout
}

fn shared_id() -> &'static str {
    "DEL4065_ABC123"
}

/// The laptop's own panel: a monitor named by its panel key, shown through a source
/// named after the laptop.
const PANEL: &str = "LEN40A0_7";

fn panel_source() -> String {
    remote_source_id(LAPTOP, "LEN40A0_7")
}

fn zone_summary(zones: &ZonesLayout) -> Vec<(String, Option<String>, Rect)> {
    zones
        .main_zones
        .iter()
        .map(|&i| {
            let z = &zones.zones[i];
            (
                z.device_id.clone().unwrap(),
                z.host.clone(),
                z.pixels_bounds,
            )
        })
        .collect()
}

#[test]
fn the_panel_key_is_make_model_and_serial_string_else_the_binary_serial() {
    assert_eq!(
        panel_key(Some("DEL"), Some("4065"), Some("ABC123"), Some("0000ABCD")).as_deref(),
        Some("DEL4065_ABC123")
    );
    assert_eq!(
        panel_key(Some("DEL"), Some("4065"), Some(""), Some("0000ABCD")).as_deref(),
        Some("DEL4065_0000ABCD")
    );
    // No serial to tell two panels of one model apart: no identity at all.
    assert_eq!(
        panel_key(Some("DEL"), Some("4065"), Some(""), Some("00000000")),
        None
    );
    assert_eq!(panel_key(None, None, None, None), None);
}

#[test]
fn detection_gives_a_local_monitor_its_panel_key() {
    let layout = local();
    assert_eq!(
        layout.monitor(shared_id()).unwrap().panel_key.as_deref(),
        Some("DEL4065_ABC123")
    );
}

#[test]
fn a_shared_panel_gains_a_source_and_a_remote_only_panel_gets_a_monitor() {
    let layout = with_laptop();

    let shared = layout.monitor(shared_id()).unwrap();
    let remote_dell = remote_source_id(LAPTOP, "DEL4065_ABC123");
    assert_eq!(remote_dell, "DEL4065_ABC123@a1b2c3d4");
    assert_eq!(
        shared.sources,
        vec![shared_id().to_owned(), remote_dell.clone()]
    );
    assert_eq!(
        layout.source(&remote_dell).unwrap().source.host.as_deref(),
        Some(LAPTOP)
    );
    assert_eq!(
        layout.source(&remote_dell).unwrap().source.input_source,
        Some(0x11)
    );

    let panel = layout.monitor(PANEL).unwrap();
    assert_eq!(panel.sources, vec![panel_source()]);
    assert_eq!(
        panel.active_source.as_deref(),
        Some(panel_source().as_str())
    );
    assert_eq!(
        layout.source(&panel_source()).unwrap().monitor,
        PANEL,
        "the source knows its monitor by the monitor's id, not its own"
    );
    assert_eq!(layout.monitors().len(), 3);
}

#[test]
fn a_shared_panel_attached_on_both_machines_shows_this_one_until_told_otherwise() {
    let mut layout = with_laptop();
    assert_eq!(
        layout
            .monitor(shared_id())
            .unwrap()
            .active_source
            .as_deref(),
        Some(shared_id())
    );

    let remote_dell = remote_source_id(LAPTOP, "DEL4065_ABC123");
    layout.set_active_source(shared_id(), Some(remote_dell.clone()));
    layout.resolve_active_sources();
    assert_eq!(
        layout
            .monitor(shared_id())
            .unwrap()
            .active_source
            .as_deref(),
        Some(remote_dell.as_str()),
        "attached on both: the choice stands"
    );
}

#[test]
fn a_shared_panel_attached_on_one_machine_only_shows_that_one() {
    let mut layout = local();
    let mut displays = laptop();
    displays[1].attached = true;
    // The Dell has gone dark on this machine: only the laptop shows it.
    let id = shared_id().to_owned();
    let mut detached = layout.source(&id).unwrap().clone();
    detached.source.attached_to_desktop = false;
    layout.add_or_update_source(detached);
    merge_remote(&mut layout, LAPTOP, "laptop", &displays);

    assert_eq!(
        layout
            .monitor(shared_id())
            .unwrap()
            .active_source
            .as_deref(),
        Some(remote_source_id(LAPTOP, "DEL4065_ABC123").as_str())
    );
}

#[test]
fn zones_follow_the_shared_panels_active_source() {
    let mut layout = with_laptop();
    layout.set_locations_from_system_configuration(false);
    let remote_dell = remote_source_id(LAPTOP, "DEL4065_ABC123");
    let panel = remote_source_id(LAPTOP, "LEN40A0_7");

    // The laptop's desktop runs -1920..1920; this one ends at 4480.
    let x = 4480 + HOST_GAP_PX + 1920;
    let host = Some(LAPTOP.to_owned());

    let shown_here = zone_summary(&compute_zones(&layout));
    assert_eq!(
        shown_here,
        vec![
            (
                shared_id().to_owned(),
                None,
                Rect::new(0.0, 0.0, 2560.0, 1440.0)
            ),
            (
                "SAM1234_XYZ".to_owned(),
                None,
                Rect::new(2560.0, 0.0, 1920.0, 1080.0)
            ),
            (
                panel.clone(),
                host.clone(),
                Rect::new(f64::from(x - 1920), 0.0, 1920.0, 1200.0)
            ),
        ]
    );

    layout.set_active_source(shared_id(), Some(remote_dell.clone()));
    let shown_there = zone_summary(&compute_zones(&layout));
    assert_eq!(
        shown_there,
        vec![
            (
                remote_dell,
                host.clone(),
                Rect::new(f64::from(x), 0.0, 1920.0, 1080.0)
            ),
            (
                "SAM1234_XYZ".to_owned(),
                None,
                Rect::new(2560.0, 0.0, 1920.0, 1080.0)
            ),
            (
                panel,
                host,
                Rect::new(f64::from(x - 1920), 0.0, 1920.0, 1200.0)
            ),
        ],
        "the Dell's zone is now the laptop's, in the laptop's pixels"
    );
}

#[test]
fn a_local_zone_links_to_a_remote_zone_like_to_any_other() {
    let mut layout = with_laptop();
    layout.set_locations_from_system_configuration(false);
    let zones = compute_zones(&layout);
    let samsung = zones
        .zones
        .iter()
        .find(|z| z.device_id.as_deref() == Some("SAM1234_XYZ"))
        .unwrap();
    let targets: Vec<Option<String>> = samsung
        .right_links
        .iter()
        .filter_map(|l| l.target)
        .map(|t| zones.zones[t].host.clone())
        .collect();
    assert_eq!(
        targets,
        vec![Some(LAPTOP.to_owned())],
        "the laptop's panel is placed right of the Samsung, and reached through it"
    );
}

#[test]
fn the_profile_key_changes_with_a_remote_that_brings_a_panel_and_back_without_it() {
    let alone = local();
    let id_alone = alone.compute_id();
    let together = with_laptop();
    let id_together = together.compute_id();
    assert_eq!(id_alone, "DEL4065_ABC123+SAM1234_XYZ");
    assert_eq!(
        id_together, "DEL4065_ABC123+LEN40A0_7+SAM1234_XYZ",
        "the remote-only panel is a monitor of the set; the shared one already was"
    );

    // The laptop leaves: the layout is rebuilt without it.
    assert_eq!(local().compute_id(), id_alone);
}

#[test]
fn a_remote_that_only_shares_panels_keeps_the_profile() {
    let mut layout = local();
    let dell = laptop().remove(1);
    merge_remote(&mut layout, LAPTOP, "laptop", &[dell]);
    assert_eq!(layout.compute_id(), local().compute_id());
}

#[test]
fn merging_a_machine_again_updates_its_sources_in_place() {
    let mut layout = with_laptop();
    let mut again = laptop();
    again[0].width = 2560;
    merge_remote(&mut layout, LAPTOP, "laptop", &again);
    let panel = panel_source();
    assert_eq!(layout.monitors().len(), 3);
    assert_eq!(layout.monitor(PANEL).unwrap().sources, vec![panel.clone()]);
    assert_eq!(layout.source(&panel).unwrap().source.in_pixel.width, 2560.0);
    assert_eq!(layout.monitor(shared_id()).unwrap().sources.len(), 2);
}

#[test]
fn a_remotes_primary_and_pixels_stay_out_of_the_local_desktop() {
    let mut displays = laptop();
    for d in &mut displays {
        d.x += 10_000;
    }
    let mut layout = local();
    merge_remote(&mut layout, LAPTOP, "laptop", &displays);
    assert_eq!(layout.primary_monitor().unwrap().id, shared_id());
    assert_eq!(
        layout.desktop_pixel_bounds(),
        Some(Rect::new(0.0, 0.0, 4480.0, 1440.0))
    );
    let placements = layout.compute_pixel_locations_from_physical(false);
    let mut placed: Vec<&str> = placements.iter().map(|(id, _)| id.as_str()).collect();
    placed.sort_unstable();
    assert_eq!(placed, vec![shared_id(), "SAM1234_XYZ"]);
}

#[test]
fn a_remote_monitor_never_placed_goes_right_of_the_others_top_aligned() {
    let mut layout = with_laptop();
    layout.set_locations_from_system_configuration(false);
    layout.anchor_on_primary();

    let samsung = layout.monitor("SAM1234_XYZ").unwrap();
    let samsung = layout.depth_projection(samsung).unwrap().outside_bounds();
    let panel = layout.monitor(PANEL).unwrap();
    let projection = layout.depth_projection(panel).unwrap();
    let outside = projection.outside_bounds();
    assert!((outside.left() - samsung.right()).abs() < 1e-9);
    assert!((outside.top() - samsung.top()).abs() < 1e-9);
    // Its physical size: the remote's report, the panel having no EDID size here.
    assert_eq!((projection.width, projection.height), (302.0, 188.0));
}

#[test]
fn compaction_moves_remote_monitors_with_the_rest() {
    let mut layout = with_laptop();
    layout.set_locations_from_system_configuration(false);
    let before = layout.monitor(PANEL).unwrap().location();
    layout.set_location(PANEL, before.offset(500.0, 0.0));
    layout.force_compact();
    let after = layout.monitor(PANEL).unwrap().location();
    assert!(
        (after.x - before.x).abs() < 1e-6,
        "the gap closed: {after:?} against {before:?}"
    );
}

#[test]
fn desktops_never_overlap_in_the_engines_pixel_space() {
    let mut layout = with_laptop();
    let mut tower = laptop();
    for d in &mut tower {
        d.panel_key = None;
        d.x -= 5000;
    }
    merge_remote(&mut layout, DESKTOP, "tower", &tower);
    layout.set_locations_from_system_configuration(false);

    let offsets = layout.host_pixel_offsets();
    assert_eq!(
        offsets.iter().map(|o| o.host.as_str()).collect::<Vec<_>>(),
        vec![DESKTOP, LAPTOP],
        "in machine-id order"
    );
    let zones = compute_zones(&layout);
    let rects: Vec<Rect> = zone_summary(&zones).into_iter().map(|z| z.2).collect();
    // This machine's two, the laptop's own panel, and both of the tower's (whose Dell
    // has no serial, so is not the shared one).
    assert_eq!(rects.len(), 5);
    for (i, a) in rects.iter().enumerate() {
        for b in &rects[i + 1..] {
            assert!(
                a.right() <= b.left()
                    || b.right() <= a.left()
                    || a.bottom() <= b.top()
                    || b.bottom() <= a.top(),
                "{a:?} and {b:?} overlap"
            );
        }
    }
}

#[test]
fn a_layout_without_remotes_compiles_to_the_same_document() {
    let mut alone = local();
    alone.set_locations_from_system_configuration(false);
    let mut merged_nothing = local();
    merge_remote(&mut merged_nothing, LAPTOP, "laptop", &[]);
    merged_nothing.set_locations_from_system_configuration(false);

    let xml = compute_zones(&alone).serialize();
    assert_eq!(compute_zones(&merged_nothing).serialize(), xml);
    assert!(!xml.contains("Host"));
}

#[test]
fn only_remote_zones_carry_their_host_in_the_document() {
    let mut layout = with_laptop();
    layout.set_locations_from_system_configuration(false);
    let xml = compute_zones(&layout).serialize();
    let x = 4480 + HOST_GAP_PX + 1920;
    assert_eq!(xml.matches(" Host=").count(), 1);
    assert!(xml.contains(&format!(
        "DeviceId=\"LEN40A0_7@a1b2c3d4\" Host=\"{LAPTOP}\" HostX=\"{x}\" HostY=\"0\">"
    )));
}

#[test]
fn the_hook_reads_back_which_machine_a_point_is_on_and_where_on_it() {
    let mut layout = with_laptop();
    layout.set_locations_from_system_configuration(false);
    let xml = compute_zones(&layout).serialize();
    let mut zones = lbm_zones::ZonesLayout::from_xml(&xml).unwrap();
    // As the evdev router does while its agent is connected: another machine's zones
    // are out of play until then.
    zones.set_remote_enterable(true);
    let x = 4480 + HOST_GAP_PX + 1920;

    let here = zones.host_point(lbm_geom::Point::new(100, 100)).unwrap();
    assert_eq!(here.host, None);
    assert_eq!(here.pixel, lbm_geom::Point::new(100, 100));

    let there = zones
        .host_point(lbm_geom::Point::new(x - 1900, 50))
        .unwrap();
    assert_eq!(there.host, Some(LAPTOP));
    assert_eq!(there.pixel, lbm_geom::Point::new(-1900, 50));

    assert!(zones
        .host_point(lbm_geom::Point::new(4480 + 10, 0))
        .is_none());
}

/// The maintainer's desk: a Dell, a Philips, and a Samsung behind a hardware KVM,
/// which this machine stops detecting when the KVM points at the Windows remote.
fn desk(with_samsung: bool) -> Layout {
    let mut layout = Layout::new(design_options());
    add_monitor(
        &mut layout,
        &output(
            "DP-1",
            0.0,
            2560.0,
            1440.0,
            edid("DEL", "A0BE", "7XK1R83", 597.0, 336.0),
        ),
    );
    add_monitor(
        &mut layout,
        &output(
            "DP-2",
            2560.0,
            1920.0,
            1080.0,
            edid("PHL", "0927", "UK02113005123", 527.0, 296.0),
        ),
    );
    if with_samsung {
        add_monitor(
            &mut layout,
            &output(
                "HDMI-A-1",
                4480.0,
                3840.0,
                2160.0,
                edid("SAM", "E035", "H1AK500000", 698.0, 393.0),
            ),
        );
    }
    layout
}

/// The Samsung as the Windows remote reports it: its device id there, and the panel
/// key of its EDID (serial string `H1AK500000`, binary serial 0).
fn windows_samsung() -> RemoteMonitor {
    remote(
        r"MONITOR\SAME035\{4d36e96e-e325-11ce-bfc1-08002be10318}\0010",
        r"\\.\DISPLAY1",
        Some("SAME035_H1AK500000"),
        0,
        3840,
        2160,
    )
}

#[test]
fn the_layout_id_is_the_same_whichever_machine_shows_a_kvm_switched_panel() {
    let here = desk(true);
    assert_eq!(
        here.monitor("SAME035_H1AK500000")
            .unwrap()
            .panel_key
            .as_deref(),
        Some("SAME035_H1AK500000"),
        "on Linux the local id is the panel key"
    );

    let mut there = desk(false);
    merge_remote(&mut there, LAPTOP, "windows", &[windows_samsung()]);

    assert_eq!(
        there.compute_id(),
        "DELA0BE_7XK1R83+PHL0927_UK02113005123+SAME035_H1AK500000"
    );
    assert_eq!(there.compute_id(), here.compute_id());
    let samsung = there.monitor("SAME035_H1AK500000").unwrap();
    let source = remote_source_id(LAPTOP, &windows_samsung().id);
    assert_eq!(samsung.sources, vec![source.clone()]);
    assert_eq!(samsung.active_source.as_deref(), Some(source.as_str()));
    assert_eq!(
        samsung.model, "SAME035",
        "the same model as here, so its stored size applies too"
    );
}

#[test]
fn a_remote_display_without_a_panel_key_keeps_its_source_id_as_monitor_id() {
    let mut layout = local();
    merge_remote(
        &mut layout,
        LAPTOP,
        "laptop",
        &[remote("eDP-1", "eDP-1", None, 0, 1920, 1080)],
    );
    let id = remote_source_id(LAPTOP, "eDP-1");
    assert_eq!(layout.monitor(&id).unwrap().sources, vec![id.clone()]);
}

#[test]
fn identical_twins_of_a_remote_get_the_panel_key_once_and_their_source_id_after() {
    // Two panels reporting the same serial, on the remote only.
    let twins = vec![
        remote(
            "TWIN-A",
            "DP-1",
            Some("ACR0620_FAKE24000001"),
            0,
            1920,
            1080,
        ),
        remote(
            "TWIN-B",
            "DP-2",
            Some("ACR0620_FAKE24000001"),
            1920,
            1920,
            1080,
        ),
    ];
    let mut layout = local();
    merge_remote(&mut layout, LAPTOP, "laptop", &twins);

    let first = layout.monitor("ACR0620_FAKE24000001").unwrap();
    assert_eq!(first.sources, vec![remote_source_id(LAPTOP, "TWIN-A")]);
    let second = remote_source_id(LAPTOP, "TWIN-B");
    assert_eq!(
        layout.monitor(&second).unwrap().sources,
        vec![second.clone()]
    );
    assert_eq!(layout.monitors().len(), 4);

    // Merged again: each keeps its monitor, neither becomes a source of the other.
    merge_remote(&mut layout, LAPTOP, "laptop", &twins);
    assert_eq!(layout.monitors().len(), 4);
    assert_eq!(
        layout.monitor("ACR0620_FAKE24000001").unwrap().sources,
        vec![remote_source_id(LAPTOP, "TWIN-A")]
    );
    assert_eq!(
        layout.monitor(&second).unwrap().sources,
        vec![second.clone()]
    );
}

#[test]
fn a_remote_twin_of_a_local_panel_never_takes_the_local_monitors_id() {
    // The Dell's key is taken here, and the laptop has two panels reporting it.
    let mut layout = local();
    let twins = vec![
        remote("A", "HDMI-1", Some(shared_id()), 0, 1920, 1080),
        remote("B", "HDMI-2", Some(shared_id()), 1920, 1920, 1080),
    ];
    merge_remote(&mut layout, LAPTOP, "laptop", &twins);

    assert_eq!(
        layout.monitor(shared_id()).unwrap().sources,
        vec![shared_id().to_owned(), remote_source_id(LAPTOP, "A")],
        "the first one is the shared panel"
    );
    let b = remote_source_id(LAPTOP, "B");
    assert_eq!(layout.monitor(&b).unwrap().sources, vec![b.clone()]);
    assert_eq!(layout.monitors().len(), 3);
}

#[test]
fn a_panel_two_remotes_report_is_one_monitor_named_by_its_key() {
    let mut layout = local();
    merge_remote(&mut layout, DESKTOP, "tower", &[windows_samsung()]);
    merge_remote(&mut layout, LAPTOP, "laptop", &[windows_samsung()]);
    let samsung = layout.monitor("SAME035_H1AK500000").unwrap();
    assert_eq!(
        samsung.sources,
        vec![
            remote_source_id(DESKTOP, &windows_samsung().id),
            remote_source_id(LAPTOP, &windows_samsung().id)
        ]
    );
    assert_eq!(layout.monitors().len(), 3);
}

/// A Samsung Odyssey G80SD gives each input its own product code (E035 on DisplayPort,
/// E031 on HDMI) under one name and one serial string: this machine on its DisplayPort
/// input, the laptop on its HDMI input, see one panel.
fn g80sd_here() -> Layout {
    let mut layout = Layout::new(design_options());
    let mut edid = edid("SAM", "E035", "H1AK500000", 697.0, 392.0);
    edid.model = Some("Odyssey G80SD".to_owned());
    add_monitor(&mut layout, &output("DP-1", 0.0, 3840.0, 2160.0, edid));
    layout
}

fn g80sd_there(name: &str) -> RemoteMonitor {
    let mut display = remote(
        "SAME031_H1AK500000",
        "HDMI-1",
        Some("SAME031_H1AK500000"),
        0,
        3840,
        2160,
    );
    display.model_name = Some(name.to_owned());
    display.panel_identity =
        lbm_layout::model::panel_identity(Some("SAM"), Some(name), Some("H1AK500000"));
    display
}

#[test]
fn a_panel_seen_through_two_inputs_with_two_product_codes_is_one_monitor() {
    let mut layout = g80sd_here();
    merge_remote(
        &mut layout,
        LAPTOP,
        "laptop",
        &[g80sd_there("Odyssey G80SD")],
    );

    assert_eq!(layout.monitors().len(), 1);
    let monitor = &layout.monitors()[0];
    assert_eq!(monitor.panel_key.as_deref(), Some("SAME035_H1AK500000"));
    assert_eq!(
        monitor.sources,
        [
            "SAME035_H1AK500000".to_owned(),
            remote_source_id(LAPTOP, "SAME031_H1AK500000")
        ]
    );
}

#[test]
fn another_model_with_the_same_serial_string_is_another_monitor() {
    let mut layout = g80sd_here();
    merge_remote(
        &mut layout,
        LAPTOP,
        "laptop",
        &[g80sd_there("Odyssey G70D")],
    );

    assert_eq!(layout.monitors().len(), 2);
}

#[test]
fn the_panel_identity_needs_a_name_and_a_serial_string() {
    assert_eq!(
        lbm_layout::model::panel_identity(Some("SAM"), Some("Odyssey G80SD"), Some("H1AK500000"))
            .as_deref(),
        Some("SAM/Odyssey G80SD_H1AK500000")
    );
    assert_eq!(
        lbm_layout::model::panel_identity(Some("SAM"), Some(""), Some("H1AK500000")),
        None
    );
    assert_eq!(
        lbm_layout::model::panel_identity(Some("SAM"), Some("Odyssey G80SD"), Some(" ")),
        None
    );
    assert_eq!(
        lbm_layout::model::panel_identity(None, Some("x"), Some("y")),
        None
    );
}
