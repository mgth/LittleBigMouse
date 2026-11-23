//! Serialized monitor I/O, independent from the agent event loop.
use lbm_ipc::vcp::{Monitor, Write};
use lbm_layout::model::Layout;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Target {
    pub connector: String,
    pub serial: Option<String>,
    /// The model's PnP code, when the layout read one from the monitor's EDID.
    pub model: Option<String>,
}

pub fn target(layout: &Layout, monitor: &str) -> Result<Target, String> {
    if layout.is_virtual() {
        return Err("A foreign layout cannot control physical monitors.".into());
    }
    if !layout.options.vcp_control {
        return Err("Enable VCP monitor control in Settings first.".into());
    }
    let m = layout
        .monitor(monitor)
        .ok_or("This monitor is no longer connected.")?;
    // Only a panel this machine is wired to can be reached over its DDC bus. A remote
    // machine's monitor names a connector of that machine, which may well be the name
    // of another monitor here.
    if !m
        .sources
        .iter()
        .any(|s| layout.source(s).is_some_and(|s| s.source.is_local()))
    {
        return Err("This monitor is on another machine.".into());
    }
    let connector = m
        .device_id
        .clone()
        .ok_or("This monitor has no hardware connector.")?;
    Ok(Target {
        // Without an EDID the layout names the model after the connector.
        model: Some(m.model.clone()).filter(|model| *model != connector),
        connector,
        // Without an EDID the layout knows no serial, so there is nothing to compare.
        serial: m
            .serial_number
            .clone()
            .filter(|s| !s.is_empty() && s != lbm_layout::linux::UNKNOWN_SERIAL),
    })
}

#[cfg(test)]
mod tests {
    use lbm_layout::linux::{add_monitor, LinuxMonitor};
    use lbm_layout::model::LayoutOptions;
    use lbm_layout::remote::{merge_remote, remote_source_id, RemoteMonitor};

    use super::*;

    #[test]
    fn a_monitor_of_another_machine_is_not_reached_over_this_ones_bus() {
        let mut layout = Layout::new(LayoutOptions::default());
        layout.options.vcp_control = true;
        add_monitor(&mut layout, &LinuxMonitor::fallback());
        // Named like an output of this machine, as a remote's connector may well be.
        let laptop = "a1b2c3d4e5f60718293a4b5c6d7e8f90";
        merge_remote(
            &mut layout,
            laptop,
            "laptop",
            &[RemoteMonitor {
                id: "FALLBACK".into(),
                connector: "FALLBACK".into(),
                width: 1920,
                height: 1080,
                scale: 1.0,
                attached: true,
                ..RemoteMonitor::default()
            }],
        );
        assert!(target(&layout, "FALLBACK").is_ok());
        let there = remote_source_id(laptop, "FALLBACK");
        assert_eq!(
            target(&layout, &there),
            Err("This monitor is on another machine.".to_owned())
        );
    }
}

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub use linux::Service;

#[cfg(not(target_os = "linux"))]
#[derive(Default)]
pub struct Service;
#[cfg(not(target_os = "linux"))]
impl Service {
    pub async fn execute(&mut self, _: Target, _: Option<Write>) -> Result<Monitor, String> {
        Err("VCP hardware control is currently available on Linux with ddcutil.".into())
    }
}
