//! DDC/CI on Windows, through dxva2: the monitor of the layout found by its device id
//! (`winmap`), then its physical monitor read and written.
//!
//! Every call goes to a blocking thread under one deadline: a DDC/CI exchange takes
//! tens of milliseconds when it goes well, and a monitor that does not answer can hold
//! the call far longer. A call that never returns keeps its thread, never the agent.

use std::ptr::addr_of_mut;
use std::time::Duration;

use lbm_ipc::vcp::{self, Choice};
use windows::core::PCWSTR;
use windows::Win32::Devices::Display::{
    CapabilitiesRequestAndCapabilitiesReply, DestroyPhysicalMonitors, GetCapabilitiesStringLength,
    GetNumberOfPhysicalMonitorsFromHMONITOR, GetPhysicalMonitorsFromHMONITOR,
    GetVCPFeatureAndVCPFeatureReply, SetVCPFeature, PHYSICAL_MONITOR,
};
use windows::Win32::Foundation::{BOOL, HANDLE, LPARAM, RECT, TRUE};
use windows::Win32::Graphics::Gdi::{
    EnumDisplayDevicesW, EnumDisplayMonitors, GetMonitorInfoW, DISPLAY_DEVICEW, HDC, HMONITOR,
    MONITORINFO, MONITORINFOEXW,
};

use super::common::{feature, Reading, PRESET, RGB};
use super::winmap::{locate, AdapterMonitors};
use super::{DdcFuture, Monitor, Target, Write};

/// One deadline per request, as on Linux.
const PATIENCE: Duration = Duration::from_secs(18);
/// `DISPLAY_DEVICE_ACTIVE`, on a monitor device: it takes part in the desktop.
const ACTIVE: u32 = 1;

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}

fn text(buffer: &[u16]) -> String {
    let end = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
    String::from_utf16_lossy(&buffer[..end])
}

fn display_device(device: Option<&[u16]>, index: u32) -> Option<DISPLAY_DEVICEW> {
    let mut entry = DISPLAY_DEVICEW {
        cb: size_of::<DISPLAY_DEVICEW>() as u32,
        ..Default::default()
    };
    let name = device.map_or(PCWSTR::null(), |d| PCWSTR(d.as_ptr()));
    // SAFETY: `name` is null or a NUL-terminated buffer that outlives the call, and
    // `entry` is a DISPLAY_DEVICEW whose `cb` says so.
    unsafe { EnumDisplayDevicesW(name, index, &mut entry, 0) }
        .as_bool()
        .then_some(entry)
}

/// The adapter sources and the monitor devices below each.
fn adapters() -> Vec<AdapterMonitors> {
    let mut adapters = Vec::new();
    for i in 0.. {
        let Some(adapter) = display_device(None, i) else {
            break;
        };
        let name = text(&adapter.DeviceName);
        let wide_name = wide(&name);
        let mut monitors = Vec::new();
        for j in 0.. {
            let Some(monitor) = display_device(Some(&wide_name), j) else {
                break;
            };
            monitors.push((text(&monitor.DeviceID), monitor.StateFlags & ACTIVE != 0));
        }
        adapters.push(AdapterMonitors { name, monitors });
    }
    adapters
}

/// Every `HMONITOR`, with the name of the adapter source it shows.
fn hmonitors() -> Vec<(HMONITOR, String)> {
    let mut found: Vec<(HMONITOR, String)> = Vec::new();
    // SAFETY: the callback runs only during the call and gets back the pointer to
    // `found`, which outlives it.
    unsafe {
        let _ = EnumDisplayMonitors(
            HDC::default(),
            None,
            Some(collect),
            LPARAM(addr_of_mut!(found) as isize),
        );
    }
    found
}

unsafe extern "system" fn collect(
    hmonitor: HMONITOR,
    _hdc: HDC,
    _rect: *mut RECT,
    data: LPARAM,
) -> BOOL {
    // SAFETY: `data` is the `Vec` `hmonitors` passed, alive and not otherwise borrowed
    // during the enumeration.
    let found = unsafe { &mut *(data.0 as *mut Vec<(HMONITOR, String)>) };
    let mut info = MONITORINFOEXW::default();
    info.monitorInfo.cbSize = size_of::<MONITORINFOEXW>() as u32;
    // SAFETY: `info` is a MONITORINFOEXW whose cbSize says so.
    if unsafe { GetMonitorInfoW(hmonitor, addr_of_mut!(info).cast::<MONITORINFO>()) }.as_bool() {
        found.push((hmonitor, text(&info.szDevice)));
    }
    TRUE
}

/// Runs `work` on the physical monitor of the layout's monitor `device_id`, and lets go
/// of the handles whatever `work` did.
fn with_monitor<T>(
    device_id: &str,
    work: impl FnOnce(HANDLE) -> Result<T, String>,
) -> Result<T, String> {
    let adapters = adapters();
    let hmonitors = hmonitors();
    let names: Vec<String> = hmonitors.iter().map(|(_, name)| name.clone()).collect();
    let located = locate(&adapters, &names, device_id)?;
    let hmonitor = hmonitors[located.hmonitor].0;
    let mut count = 0u32;
    // SAFETY: a valid out pointer.
    unsafe { GetNumberOfPhysicalMonitorsFromHMONITOR(hmonitor, &mut count) }
        .map_err(|e| format!("Windows offers no DDC/CI handle for this monitor: {e}"))?;
    if count as usize != located.expected {
        return Err(
            "Windows groups this display's monitors in a way that does not say which is which."
                .into(),
        );
    }
    let mut physical = vec![PHYSICAL_MONITOR::default(); count as usize];
    // SAFETY: `physical` holds as many entries as Windows said there are.
    unsafe { GetPhysicalMonitorsFromHMONITOR(hmonitor, &mut physical) }
        .map_err(|e| format!("Windows offers no DDC/CI handle for this monitor: {e}"))?;
    let result = work(physical[located.physical].hPhysicalMonitor);
    // SAFETY: the handles Windows just gave, released once.
    let _ = unsafe { DestroyPhysicalMonitors(&physical) };
    result
}

/// One code's current value and maximum, as dxva2 reads them.
fn get(handle: HANDLE, code: u8) -> Result<Reading, String> {
    let (mut current, mut maximum) = (0u32, 0u32);
    // SAFETY: valid out pointers; the handle is alive for the call.
    let ok = unsafe {
        GetVCPFeatureAndVCPFeatureReply(handle, code, None, &mut current, Some(&mut maximum))
    };
    if ok == 0 {
        return Err(format!(
            "No DDC/CI answer for {}. Enable DDC/CI in the monitor menu and check the cable or dock.",
            vcp::name(code)
        ));
    }
    Ok(if vcp::continuous(code) {
        Reading {
            value: current.min(u32::from(u16::MAX)) as u16,
            maximum: maximum.min(u32::from(u16::MAX)) as u16,
            plain: true,
        }
    } else {
        // A non-continuous state is the low byte: the high one is a separate field, as
        // on Linux (ddcutil's SL).
        Reading {
            value: (current & 0xff) as u16,
            maximum: 255,
            plain: true,
        }
    })
}

fn set(handle: HANDLE, code: u8, value: u16) -> Result<(), String> {
    // SAFETY: the handle is alive for the call.
    if unsafe { SetVCPFeature(handle, code, u32::from(value)) } == 0 {
        Err(format!(
            "The monitor rejected the {} change.",
            vcp::name(code)
        ))
    } else {
        Ok(())
    }
}

fn capabilities_string(handle: HANDLE) -> Result<String, String> {
    let mut length = 0u32;
    // SAFETY: a valid out pointer.
    if unsafe { GetCapabilitiesStringLength(handle, &mut length) } == 0 || length == 0 {
        return Err("The monitor did not say what it supports.".into());
    }
    let mut buffer = vec![0u8; length as usize];
    // SAFETY: the buffer is as long as Windows said the string is.
    if unsafe { CapabilitiesRequestAndCapabilitiesReply(handle, &mut buffer) } == 0 {
        return Err("The monitor did not say what it supports.".into());
    }
    let end = buffer.iter().position(|&b| b == 0).unwrap_or(buffer.len());
    Ok(String::from_utf8_lossy(&buffer[..end]).into_owned())
}

/// `work` on a blocking thread, under the deadline.
async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    match tokio::time::timeout(PATIENCE, tokio::task::spawn_blocking(work)).await {
        Err(_) => Err(
            "The monitor did not respond in time. Check DDC/CI in its on-screen menu, then refresh."
                .into(),
        ),
        Ok(Err(error)) => Err(format!("The DDC/CI request failed: {error}")),
        Ok(Ok(result)) => result,
    }
}

/// The property panel's controls of a monitor, as on Linux: a probe hands out a token,
/// a write is accepted only against the latest one.
#[derive(Default)]
pub struct Service {
    next_token: u64,
    cached: Option<(Target, Monitor)>,
}

impl Service {
    pub async fn execute(
        &mut self,
        target: Target,
        write: Option<Write>,
    ) -> Result<Monitor, String> {
        match write {
            Some(write) => self.write(target, write).await,
            None => self.probe(target).await,
        }
    }

    /// See the Linux service: the input switching wrote behind the panel's back.
    pub fn forget(&mut self) {
        self.cached = None;
    }

    async fn probe(&mut self, target: Target) -> Result<Monitor, String> {
        self.cached = None;
        let device = target.connector.clone();
        let (caps, readings) = blocking(move || {
            with_monitor(&device, |handle| {
                let caps = capabilities_string(handle)
                    .map(|caps| super::mccs::capabilities(&caps))
                    .unwrap_or_default();
                let codes: Vec<u8> = if caps.is_empty() {
                    vec![0x10, 0x12, 0x16, 0x18, 0x1a]
                } else {
                    vcp::CODES
                        .iter()
                        .copied()
                        .filter(|c| caps.contains_key(c))
                        .collect()
                };
                let readings: Vec<(u8, Option<Reading>)> =
                    codes.iter().map(|&c| (c, get(handle, c).ok())).collect();
                Ok((caps, readings))
            })
        })
        .await?;
        let fallback = caps.is_empty();
        if readings.is_empty() {
            return Err("This monitor advertises none of the supported VCP controls.".into());
        }
        let answered = readings.iter().filter(|(_, r)| r.is_some()).count();
        if answered == 0 {
            return Err(format!(
                "No DDC/CI response from {}. Enable DDC/CI in the monitor menu and check the cable or dock.",
                target.connector
            ));
        }
        let mut features = vec![];
        for (code, reading) in &readings {
            let Some(reading) = reading else {
                continue;
            };
            let choices: Vec<Choice> = caps.get(code).cloned().unwrap_or_default();
            if !vcp::continuous(*code) && choices.is_empty() {
                continue;
            }
            features.extend(feature(*code, *reading, choices));
        }
        if features.is_empty() {
            return Err("The monitor returned no usable controls.".into());
        }
        self.next_token += 1;
        let state = Monitor {
            token: self.next_token,
            connection: format!("{} · DDC/CI · dxva2", target.connector),
            features,
            note: if fallback {
                Some("Capabilities unavailable; showing controls that answered the probe.".into())
            } else if answered < readings.len() {
                Some(
                    "Some advertised controls did not answer; only readable controls are shown."
                        .into(),
                )
            } else {
                None
            },
        };
        self.cached = Some((target, state.clone()));
        Ok(state)
    }

    async fn write(&mut self, target: Target, write: Write) -> Result<Monitor, String> {
        let Some((old_target, mut state)) = self.cached.clone() else {
            return Err("Refresh the monitor controls before changing a value.".into());
        };
        if old_target != target || state.token != write.token {
            return Err("The monitor changed; refresh its controls before editing.".into());
        }
        state
            .features
            .iter()
            .find(|f| f.code == write.code)
            .filter(|f| f.accepts(write.value))
            .ok_or("This value is not supported by the monitor.")?;
        // A failed or cancelled write can still have reached the monitor.
        self.cached = None;
        // A colour preset can change gain, brightness and contrast as well, and changing
        // a gain can make the monitor leave its preset. An input or power change may
        // leave nothing to read back.
        let has_preset = state.features.iter().any(|f| f.code == PRESET);
        let codes: Vec<u8> = if matches!(write.code, 0x60 | 0xd6) {
            Vec::new()
        } else if write.code == PRESET {
            state.features.iter().map(|f| f.code).collect()
        } else if has_preset && RGB.contains(&write.code) {
            vec![write.code, PRESET]
        } else {
            vec![write.code]
        };
        let device = target.connector.clone();
        let (code, value) = (write.code, write.value);
        let reread = codes.clone();
        let readings = blocking(move || {
            with_monitor(&device, |handle| {
                set(handle, code, value)?;
                Ok(reread
                    .iter()
                    .map(|&c| (c, get(handle, c).ok()))
                    .collect::<Vec<_>>())
            })
        })
        .await?;
        if codes.is_empty() {
            if let Some(f) = state.features.iter_mut().find(|f| f.code == write.code) {
                f.value = write.value;
            }
            state.note = Some("Command sent. The monitor may stop answering after changing input or power; refresh when it is available again.".into());
        } else {
            let actual = readings
                .iter()
                .find(|(c, _)| *c == write.code)
                .and_then(|(_, r)| *r)
                .ok_or("The command was sent, but its value could not be read back. Refresh the monitor controls.")?;
            // What could not be read back is unknown now, and showing its old value would
            // make selecting that value again do nothing.
            let before = state.features.len();
            state.features = std::mem::take(&mut state.features)
                .into_iter()
                .filter_map(|f| {
                    if !codes.contains(&f.code) {
                        return Some(f);
                    }
                    let reading = readings.iter().find(|(c, _)| *c == f.code)?.1?;
                    feature(f.code, reading, f.choices)
                })
                .collect();
            let lost = state.features.len() < before;
            state.note = (actual.value != write.value).then(|| {
                format!(
                    "The monitor returned {} for {} (requested {}).",
                    actual.value,
                    vcp::name(write.code),
                    write.value
                )
            });
            if lost && state.note.is_none() {
                state.note =
                    Some("Some controls could not be read back. Refresh to show them.".into());
            }
        }
        self.cached = Some((target, state.clone()));
        Ok(state)
    }
}

/// One code read or written on its own: what the input switching needs. It holds
/// nothing: every request finds its monitor afresh, since the displays may have
/// changed since the last one.
#[derive(Default)]
pub struct SystemDdc(());

impl super::Ddc for SystemDdc {
    fn read(&self, target: Target, code: u8) -> DdcFuture<u16> {
        Box::pin(blocking(move || {
            with_monitor(&target.connector, |handle| {
                get(handle, code).map(|r| r.value)
            })
        }))
    }

    fn write(&self, target: Target, code: u8, value: u16) -> DdcFuture<()> {
        Box::pin(blocking(move || {
            with_monitor(&target.connector, |handle| set(handle, code, value))
        }))
    }
}
