//! Serialized monitor I/O, independent from the agent event loop.
//!
//! Two users of DDC/CI: the property panel ([`Service`]: a probe of every allowed
//! control, then writes against it), and the input switching of the multi-PC mode
//! ([`Ddc`]: one code read or written on its own, `crate::input_switch`). Linux goes
//! through the `ddcutil` command, Windows through dxva2; both run outside the agent's
//! loop and under a deadline, since a monitor that does not answer can hold a request
//! for seconds.
use std::future::Future;
use std::pin::Pin;

use lbm_ipc::vcp::{Monitor, Write};
use lbm_layout::model::Layout;

mod common;
mod mccs;
mod winmap;

/// VCP 0x60, the monitor's input source.
pub const INPUT: u8 = 0x60;

/// What a [`Ddc`] request comes back with, later.
pub type DdcFuture<T> = Pin<Box<dyn Future<Output = Result<T, String>> + Send>>;

/// One VCP code read or written on one monitor, on its own. Behind a trait so that the
/// input switching is exercised against a recording double: a real 0x60 write changes
/// what the user's screens show.
pub trait Ddc: Send + Sync {
    fn read(&self, target: Target, code: u8) -> DdcFuture<u16>;
    fn write(&self, target: Target, code: u8, value: u16) -> DdcFuture<()>;
}

/// A request a [`FakeDdc`] was given.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DdcCall {
    Read {
        connector: String,
        code: u8,
    },
    Write {
        connector: String,
        code: u8,
        value: u16,
    },
}

/// A monitor bus that touches no monitor: what each connector's codes hold, which
/// connectors do not answer, and every request, for the tests.
#[derive(Debug, Default)]
pub struct FakeDdc {
    state: std::sync::Mutex<FakeState>,
}

#[derive(Debug, Default)]
struct FakeState {
    values: std::collections::HashMap<(String, u8), u16>,
    silent: std::collections::HashSet<String>,
    calls: Vec<DdcCall>,
}

impl FakeDdc {
    /// `connector` answers `value` for `code`.
    pub fn set(&self, connector: &str, code: u8, value: u16) {
        let mut state = self.state.lock().unwrap();
        state.values.insert((connector.to_owned(), code), value);
    }

    /// `connector` answers nothing, read or write.
    pub fn silence(&self, connector: &str) {
        self.state
            .lock()
            .unwrap()
            .silent
            .insert(connector.to_owned());
    }

    pub fn calls(&self) -> Vec<DdcCall> {
        self.state.lock().unwrap().calls.clone()
    }

    pub fn writes(&self) -> Vec<DdcCall> {
        self.calls()
            .into_iter()
            .filter(|c| matches!(c, DdcCall::Write { .. }))
            .collect()
    }
}

impl Ddc for FakeDdc {
    fn read(&self, target: Target, code: u8) -> DdcFuture<u16> {
        let mut state = self.state.lock().unwrap();
        state.calls.push(DdcCall::Read {
            connector: target.connector.clone(),
            code,
        });
        let result = if state.silent.contains(&target.connector) {
            Err(format!("no DDC/CI answer from {}", target.connector))
        } else {
            state
                .values
                .get(&(target.connector.clone(), code))
                .copied()
                .ok_or_else(|| format!("{} does not support {code:#04x}", target.connector))
        };
        Box::pin(async move { result })
    }

    fn write(&self, target: Target, code: u8, value: u16) -> DdcFuture<()> {
        let mut state = self.state.lock().unwrap();
        state.calls.push(DdcCall::Write {
            connector: target.connector.clone(),
            code,
            value,
        });
        let result = if state.silent.contains(&target.connector) {
            Err(format!("no DDC/CI answer from {}", target.connector))
        } else {
            state.values.insert((target.connector, code), value);
            Ok(())
        };
        Box::pin(async move { result })
    }
}

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
pub use linux::{Service, SystemDdc};

#[cfg(windows)]
mod dxva2;
#[cfg(windows)]
pub use dxva2::{Service, SystemDdc};

#[cfg(not(any(target_os = "linux", windows)))]
const UNSUPPORTED: &str = "VCP hardware control is available on Linux with ddcutil and on Windows.";

#[cfg(not(any(target_os = "linux", windows)))]
#[derive(Default)]
pub struct Service;
#[cfg(not(any(target_os = "linux", windows)))]
impl Service {
    pub async fn execute(&mut self, _: Target, _: Option<Write>) -> Result<Monitor, String> {
        Err(UNSUPPORTED.into())
    }
    pub fn forget(&mut self) {}
}

#[cfg(not(any(target_os = "linux", windows)))]
#[derive(Default)]
pub struct SystemDdc(());
#[cfg(not(any(target_os = "linux", windows)))]
impl Ddc for SystemDdc {
    fn read(&self, _: Target, _: u8) -> DdcFuture<u16> {
        Box::pin(async { Err(UNSUPPORTED.into()) })
    }
    fn write(&self, _: Target, _: u8, _: u16) -> DdcFuture<()> {
        Box::pin(async { Err(UNSUPPORTED.into()) })
    }
}
