//! This machine's screens as a remote reports them to its master (`PeerDisplay`).
//!
//! Built from the platform detection itself — the neutral descriptions the layout model
//! maps (`LinuxMonitor`, `WindowsMonitor`) — rather than from the layout: the master
//! maps them with its own model, and what it needs is what its detection would have
//! read here, the raw EDID included, not this machine's interpretation of it.

use base64::Engine as _;
use lbm_display::linux::drm::EdidMap;
use lbm_ipc::peer::PeerDisplay;
use lbm_layout::linux::LinuxMonitor;

fn base64(bytes: &[u8]) -> Option<String> {
    (!bytes.is_empty()).then(|| base64::engine::general_purpose::STANDARD.encode(bytes))
}

/// The Linux outputs, with the raw EDIDs read from sysfs by connector.
///
/// The id is the one the layout gives the monitor here (`lbm_layout::linux::add_monitor`:
/// PnP code and serial, else the connector; the connector appended to a duplicate), so
/// that what the master later asks of a display (`SelectInput`) names it the way this
/// machine does.
pub fn linux_displays(monitors: &[LinuxMonitor], edids: &EdidMap) -> Vec<PeerDisplay> {
    let mut displays: Vec<PeerDisplay> = Vec::with_capacity(monitors.len());
    for monitor in monitors {
        let connector = monitor.connector_name.as_str();
        let parsed = monitor.edid.as_ref();
        let mut id = match parsed {
            Some(edid) => format!(
                "{}_{}",
                edid.pnp_code().unwrap_or_else(|| connector.to_owned()),
                edid.monitor_serial().unwrap_or("")
            ),
            None => connector.to_owned(),
        };
        if displays.iter().any(|d| d.id == id) {
            id = format!("{id}@{connector}");
        }
        let name = parsed
            .and_then(|e| e.model.as_deref())
            .filter(|model| !model.is_empty())
            .unwrap_or(connector)
            .to_owned();
        displays.push(PeerDisplay {
            id,
            connector: connector.to_owned(),
            name,
            edid: edids.get(connector).and_then(|edid| base64(&edid.raw)),
            // The compositor's logical space: the one the cursor moves in under
            // Wayland, and the one this machine's layout and hook work in.
            x: monitor.logical_x.round() as i32,
            y: monitor.logical_y.round() as i32,
            width: monitor.logical_width.round() as i32,
            height: monitor.logical_height.round() as i32,
            scale: monitor.scale,
            effective_dpi: 96.0 * monitor.scale,
            orientation: monitor.orientation.rem_euclid(4) as u8,
            frequency: f64::from(monitor.frequency),
            primary: monitor.primary,
            attached: monitor.enabled,
            width_mm: monitor.width_mm,
            height_mm: monitor.height_mm,
            input_source: None,
        });
    }
    displays
}

/// The Win32 device tree's monitors that take part in a desktop connection, with their
/// raw EDIDs from the registry. A monitor kept off the desktop by Windows (a VR
/// headset, #364) or with no connection at all has no pixels to drive and is left out.
pub fn windows_displays(tree: &lbm_display::windows::tree::DisplayTree) -> Vec<PeerDisplay> {
    use lbm_layout::windows::{infer_orientation, physical_size_in_mm};

    tree.monitors()
        .zip(tree.layout_input())
        .filter(|(_, monitor)| !monitor.is_specialized)
        .filter_map(|(device, monitor)| {
            let connection = monitor.active_connection.as_ref()?;
            let adapter = &connection.adapter;
            let mode = adapter.current_mode.as_ref();
            let orientation = mode.map_or(0, |mode| infer_orientation(mode, monitor.edid.as_ref()));
            // Intrinsic, then turned the way the panel is: the contract's sizes are
            // oriented, as the Linux compositors report them.
            let (mut width_mm, mut height_mm) = physical_size_in_mm(&monitor);
            if orientation % 2 != 0 {
                std::mem::swap(&mut width_mm, &mut height_mm);
            }
            let name = monitor
                .edid
                .as_ref()
                .and_then(|e| e.model.clone())
                .filter(|model| !model.is_empty())
                .unwrap_or_else(|| connection.device_string.clone());
            Some(PeerDisplay {
                id: monitor.id.clone(),
                connector: adapter.device_name.clone(),
                name,
                edid: device.edid.as_ref().and_then(|edid| base64(&edid.raw)),
                x: mode.map_or(0, |m| m.position.x.round() as i32),
                y: mode.map_or(0, |m| m.position.y.round() as i32),
                width: mode.map_or(0, |m| m.pels.width().round() as i32),
                height: mode.map_or(0, |m| m.pels.height().round() as i32),
                scale: if adapter.effective_dpi.x > 0.0 {
                    adapter.effective_dpi.x / 96.0
                } else {
                    1.0
                },
                effective_dpi: adapter.effective_dpi.x,
                orientation: orientation.rem_euclid(4) as u8,
                frequency: mode.map_or(0.0, |m| f64::from(m.display_frequency)),
                primary: adapter.primary,
                attached: connection.attached_to_desktop,
                width_mm,
                height_mm,
                input_source: None,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use lbm_layout::linux::LinuxEdid;

    use super::*;

    fn output(connector: &str, x: f64, serial: Option<&str>) -> LinuxMonitor {
        LinuxMonitor {
            connector_name: connector.into(),
            logical_x: x,
            logical_y: 0.0,
            logical_width: 1706.6,
            logical_height: 960.0,
            pixel_width: 2560,
            pixel_height: 1440,
            scale: 1.5,
            width_mm: 597.0,
            height_mm: 336.0,
            primary: x == 0.0,
            enabled: true,
            orientation: 0,
            frequency: 60,
            edid: serial.map(|serial| LinuxEdid {
                manufacturer_code: Some("DEL".into()),
                product_code: Some("A0B1".into()),
                serial: Some("00000001".into()),
                serial_number: Some(serial.into()),
                model: Some("DELL U2720Q".into()),
                ..Default::default()
            }),
        }
    }

    #[test]
    fn a_linux_output_is_reported_in_logical_pixels_with_its_raw_edid() {
        let mut edids = EdidMap::default();
        let block: Vec<u8> = (0..128).collect();
        edids.insert("DP-1", lbm_display::edid::parse("card0-DP-1", &block));
        let displays = linux_displays(
            &[
                output("DP-1", 0.0, Some("ABC")),
                output("DP-2", 1707.0, Some("ABC")),
                output("HDMI-A-1", 3413.0, None),
            ],
            &edids,
        );
        let ids: Vec<_> = displays.iter().map(|d| d.id.as_str()).collect();
        assert_eq!(ids, ["DELA0B1_ABC", "DELA0B1_ABC@DP-2", "HDMI-A-1"]);

        let first = &displays[0];
        assert_eq!(first.connector, "DP-1");
        assert_eq!(first.name, "DELL U2720Q");
        assert_eq!(
            (first.x, first.y, first.width, first.height),
            (0, 0, 1707, 960)
        );
        assert_eq!(first.scale, 1.5);
        assert_eq!(first.effective_dpi, 144.0);
        assert!(first.primary && first.attached);
        assert_eq!((first.width_mm, first.height_mm), (597.0, 336.0));
        let edid = base64::engine::general_purpose::STANDARD
            .decode(first.edid.as_deref().unwrap())
            .unwrap();
        assert_eq!(edid, block);
        assert_eq!(displays[1].edid, None, "no EDID read for that connector");
        assert_eq!(displays[2].name, "HDMI-A-1");
        assert_eq!(first.input_source, None);
    }
}
