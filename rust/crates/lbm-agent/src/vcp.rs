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
