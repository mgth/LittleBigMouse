//! C#'s device tree (`DisplayDevice` and its subclasses), built from what the Win32
//! calls returned: `DeviceFactory.BuildDisplayDeviceAndChildren`, then the rest of
//! `MonitorDeviceHelper.GetDisplayDevices` — the desktop state, the order, the
//! duplicate ids, the Settings numbers, the specialized monitors.

use std::collections::{HashMap, HashSet};
use std::fmt;

use lbm_layout::collation::invariant_compare;
use lbm_layout::geo::{Point, Size, Vector};
use lbm_layout::windows::{
    ordinal_ignore_case_compare, WindowsAdapter, WindowsConnection, WindowsDeviceCaps,
    WindowsDisplayMode, WindowsMonitor,
};

use super::ids::{disambiguate_source_ids, physical_id, pnp_code_from_id, source_id};
use super::registry::{find_edid, EdidEntry};
use crate::edid::Edid;

/// `DISPLAY_DEVICE_ATTACHED_TO_DESKTOP`, C#'s `DeviceState.AttachedToDesktop`.
pub const ATTACHED_TO_DESKTOP: u32 = 0x1;

/// `DM_POSITION`, `DM_DISPLAYORIENTATION`, `DM_PELSWIDTH`, `DM_PELSHEIGHT`,
/// `DM_DISPLAYFREQUENCY`: the `dmFields` bits `GetDisplayMode` tests.
pub const DM_POSITION: u32 = 0x20;
pub const DM_DISPLAYORIENTATION: u32 = 0x80;
pub const DM_PELSWIDTH: u32 = 0x8_0000;
pub const DM_PELSHEIGHT: u32 = 0x10_0000;
pub const DM_DISPLAYFREQUENCY: u32 = 0x40_0000;

/// What `EnumDisplayDevices` reports for one device (C#'s `WinGdi.DisplayDevice`, the
/// members read).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DeviceEntry {
    /// `\\.\DISPLAY1` for an adapter source, `\\.\DISPLAY1\Monitor0` for a monitor.
    pub device_name: String,
    /// The GPU's or the monitor's name.
    pub device_string: String,
    /// `DISPLAY_DEVICE_*`; only [`ATTACHED_TO_DESKTOP`] is read.
    pub state_flags: u32,
    /// `PCI\VEN_10DE&...` for an adapter,
    /// `MONITOR\DEL4065\{4d36e96e-e325-11ce-bfc1-08002be10318}\0003` for a monitor.
    pub device_id: String,
}

/// `GetDeviceCaps` on an adapter source: the sizes `DeviceCaps` keeps.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DeviceCaps {
    pub horz_size: i32,
    pub vert_size: i32,
    pub horz_res: i32,
    pub vert_res: i32,
    pub log_pixels_x: i32,
    pub log_pixels_y: i32,
}

/// The members of a `DEVMODE` that `GetDisplayMode` reads.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RawDevMode {
    pub fields: u32,
    pub position_x: i32,
    pub position_y: i32,
    pub display_orientation: u32,
    pub pels_width: u32,
    pub pels_height: u32,
    pub display_frequency: u32,
}

/// An adapter source as `EnumDisplayDevices(NULL, i)` lists it, with what C#'s
/// `BuildPhysicalAdapter` asks about it, and the monitors below it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RawAdapter {
    pub device: DeviceEntry,
    /// `GetDeviceCaps` on `CreateDC("DISPLAY", source)`: all 0 when there is no DC.
    pub capabilities: DeviceCaps,
    /// `EnumDisplaySettingsEx(source, ENUM_CURRENT_SETTINGS)`, `None` when it fails.
    pub current_mode: Option<RawDevMode>,
    /// `EnumDisplayDevices(source, j)`, in order.
    pub monitors: Vec<RawMonitor>,
}

/// A monitor `EnumDisplayDevices(source, j)` lists.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RawMonitor {
    pub device: DeviceEntry,
    /// The same call with `EDD_GET_DEVICE_INTERFACE_NAME`: the device interface path
    /// (`\\?\DISPLAY#DEL4065#...`), `""` when that call fails.
    pub interface_path: String,
}

/// A rectangle as `RECT` has it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RawRect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl RawRect {
    pub fn width(&self) -> i64 {
        i64::from(self.right) - i64::from(self.left)
    }

    pub fn height(&self) -> i64 {
        i64::from(self.bottom) - i64::from(self.top)
    }
}

/// One `GetDpiForMonitor` answer: the values it wrote (0 where it wrote none) and
/// whether it succeeded.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DpiReading {
    pub x: u32,
    pub y: u32,
    pub succeeded: bool,
}

impl DpiReading {
    fn vector(self) -> Vector {
        Vector::new(f64::from(self.x), f64::from(self.y))
    }
}

/// One `HMONITOR` of `EnumDisplayMonitors` whose `GetMonitorInfo` succeeded.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RawMonitorInfo {
    /// `szDevice`: the adapter source it shows (`\\.\DISPLAY1`).
    pub device_name: String,
    /// `dwFlags`; C# takes exactly 1 for primary.
    pub flags: u32,
    /// `rcMonitor`, in virtual-screen pixels.
    pub monitor: RawRect,
    /// `rcWork`.
    pub work: RawRect,
    pub effective_dpi: DpiReading,
    pub angular_dpi: DpiReading,
    pub raw_dpi: DpiReading,
}

/// A locally unique id, as the CCD API names an adapter.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Luid {
    pub low_part: u32,
    pub high_part: i32,
}

/// A CCD target (a GPU connector) available in `QueryDisplayConfig(QDC_ALL_PATHS)`,
/// with what `DisplayConfigGetDeviceInfo` says about it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Target {
    pub adapter: Luid,
    pub id: u32,
    /// `DISPLAYCONFIG_TARGET_DEVICE_NAME.monitorDevicePath`: the device interface path
    /// of the monitor on it, `""` when the query fails.
    pub device_path: String,
    /// `DISPLAYCONFIG_GET_MONITOR_SPECIALIZATION.isSpecializationEnabled`; false when
    /// the query fails (before Windows 11).
    pub specialized: bool,
}

/// Everything the Win32 calls return, before any of C#'s rules: what `sys` reads on
/// Windows, and what tests make up anywhere.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RawDisplays {
    /// `EnumDisplayDevices(NULL, i)`, in order.
    pub adapters: Vec<RawAdapter>,
    /// `EnumDisplayMonitors`, in order.
    pub monitor_infos: Vec<RawMonitorInfo>,
    /// The available targets of `QueryDisplayConfig(QDC_ALL_PATHS)`; `None` when the
    /// query failed.
    pub targets: Option<Vec<Target>>,
    /// SetupAPI's monitor devices, in SetupAPI's order.
    pub edids: Vec<EdidEntry>,
}

/// Where C# throws out of the enumeration.
#[derive(Clone, Debug, PartialEq)]
pub enum DiscoveryError {
    /// A size or a rectangle with a negative side, which HLab.Geo's `Size` and `Rect`
    /// reject with `ArgumentException` (`BuildDeviceCaps`, `UpdateFromMonitorInfo`).
    NegativeSize {
        what: &'static str,
        device_name: String,
        width: i64,
        height: i64,
    },
}

impl fmt::Display for DiscoveryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DiscoveryError::NegativeSize {
                what,
                device_name,
                width,
                height,
            } => write!(
                f,
                "{device_name}: {what} of {width} x {height}, a negative size"
            ),
        }
    }
}

impl std::error::Error for DiscoveryError {}

/// An adapter source of the tree: C#'s `PhysicalAdapter`.
#[derive(Clone, Debug, PartialEq)]
pub struct Adapter {
    /// `DeviceID`: `PCI\VEN_10DE&DEV_2206&...`.
    pub id: String,
    pub state_flags: u32,
    /// C#'s `HMonitor != 0`: `EnumDisplayMonitors` shows this source.
    pub has_monitor: bool,
    /// What the mapping reads of it.
    pub adapter: WindowsAdapter,
    /// The monitors below it, in order.
    pub connections: Vec<Connection>,
}

/// A monitor below an adapter source: C#'s `MonitorDeviceConnection`.
#[derive(Clone, Debug, PartialEq)]
pub struct Connection {
    pub device: DeviceEntry,
    /// The monitor device it is a connection of, in [`DisplayTree::devices`].
    pub monitor: usize,
}

impl Connection {
    pub fn attached_to_desktop(&self) -> bool {
        self.device.state_flags & ATTACHED_TO_DESKTOP != 0
    }
}

/// Where a connection is: its adapter and its rank below it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ConnectionRef {
    pub adapter: usize,
    pub index: usize,
}

/// A monitor: C#'s `MonitorDevice`.
#[derive(Clone, Debug, PartialEq)]
pub struct MonitorDevice {
    /// The monitor's device id.
    pub id: String,
    pub pnp_code: String,
    pub physical_id: String,
    pub source_id: String,
    pub interface_path: String,
    pub edid: Option<Edid>,
    pub monitor_number: String,
    pub is_specialized: bool,
    /// Its connections, in the order C# adds them.
    pub connections: Vec<ConnectionRef>,
}

impl MonitorDevice {
    /// The monitor `DeviceFactory.BuildMonitorDevice` creates for a device id it has
    /// not seen: its EDID, PnP code, physical and source ids.
    pub fn new(device_id: &str, interface_path: &str, edid: Option<Edid>) -> Self {
        Self {
            id: device_id.to_owned(),
            pnp_code: pnp_code_from_id(device_id).to_owned(),
            physical_id: physical_id(device_id, edid.as_ref()),
            source_id: source_id(device_id, edid.as_ref()),
            interface_path: interface_path.to_owned(),
            edid,
            monitor_number: String::new(),
            is_specialized: false,
            connections: Vec::new(),
        }
    }
}

/// C#'s `GetDisplayMode(DevMode)`, the members kept: a field the driver did not fill
/// reads as 0, the size as 1 x 1 — and as `dmPelsWidth` x `dmPelsHeight` when either
/// bit is set.
pub fn display_mode(dm: &RawDevMode) -> WindowsDisplayMode {
    let has = |bit: u32| dm.fields & bit != 0;
    WindowsDisplayMode {
        display_orientation: if has(DM_DISPLAYORIENTATION) {
            dm.display_orientation as i32
        } else {
            0
        },
        position: if has(DM_POSITION) {
            Point::new(f64::from(dm.position_x), f64::from(dm.position_y))
        } else {
            Point::new(0.0, 0.0)
        },
        pels: if has(DM_PELSWIDTH | DM_PELSHEIGHT) {
            Size::new(f64::from(dm.pels_width), f64::from(dm.pels_height))
        } else {
            Size::new(1.0, 1.0)
        },
        display_frequency: if has(DM_DISPLAYFREQUENCY) {
            dm.display_frequency as i32
        } else {
            0
        },
    }
}

/// One connection as `MonitorDevice.SelectConnection` weighs it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ConnectionChoice<'a> {
    /// The monitor device is attached to the desktop.
    pub attached_to_desktop: bool,
    /// Its adapter has an `HMONITOR`.
    pub has_monitor: bool,
    /// Its adapter has a current mode.
    pub has_mode: bool,
    /// Its adapter's device name.
    pub adapter_name: &'a str,
}

/// C# `MonitorDevice.SelectConnection`: the connection attached to the desktop, then
/// the one on a source with an `HMONITOR`, then with a mode, then by source name
/// (ordinal, case ignored), the first on a tie. `EnumDisplayDevices` lists a detached
/// monitor below every inactive source, so the enumeration order is no identity.
/// Returns the chosen index.
pub fn select_connection<'a>(
    connections: impl IntoIterator<Item = ConnectionChoice<'a>>,
) -> Option<usize> {
    let rank = |a: &ConnectionChoice, b: &ConnectionChoice| {
        b.attached_to_desktop
            .cmp(&a.attached_to_desktop)
            .then(b.has_monitor.cmp(&a.has_monitor))
            .then(b.has_mode.cmp(&a.has_mode))
            .then_with(|| ordinal_ignore_case_compare(a.adapter_name, b.adapter_name))
    };
    connections
        .into_iter()
        .enumerate()
        // `min_by` keeps the first of equals, as the stable OrderBy's first does.
        .min_by(|(_, a), (_, b)| rank(a, b))
        .map(|(i, _)| i)
}

/// C#'s device tree after `GetDisplayDevices`.
#[derive(Clone, Debug, PartialEq)]
pub struct DisplayTree {
    /// The adapter sources, in `EnumDisplayDevices` order.
    pub adapters: Vec<Adapter>,
    /// Every monitor device built, in creation order. C# builds a second one for a
    /// device id listed twice below one source; only the first is ever enumerated.
    pub devices: Vec<MonitorDevice>,
    /// `root.AllMonitorDevices()`: the first device of each id in tree order, sorted
    /// by physical id; indices into `devices`.
    order: Vec<usize>,
}

impl DisplayTree {
    /// `GetDisplayDevices`, from the Win32 answers.
    ///
    /// 1. The tree (`BuildDisplayDeviceAndChildren`): each source, each monitor below
    ///    it. A monitor id already seen below an *earlier* source is that monitor again
    ///    (a detached monitor is listed below every inactive source), which takes the
    ///    interface path if it had none; C# does not look among the monitors of the
    ///    source it is building, which is not in the tree yet. A new monitor gets its
    ///    EDID ([`find_edid`]) and its ids.
    /// 2. The desktop state (`EnumDisplayMonitors`): every source an `HMONITOR` shows
    ///    gets it, its primary flag and its three DPIs.
    /// 3. The monitors in `AllMonitorDevices` order: the first of each id in tree
    ///    order, sorted by physical id with the invariant culture's comparer (C# uses
    ///    the current culture), stable.
    /// 4. The duplicate source ids ([`disambiguate_source_ids`]), the Settings numbers
    ///    ([`number_monitors`]), the specialized monitors ([`flag_specialized`]).
    ///
    /// Fails where HLab.Geo throws in C#: a negative device-caps size, an inverted
    /// monitor or work area.
    pub fn assemble(raw: RawDisplays) -> Result<DisplayTree, DiscoveryError> {
        let RawDisplays {
            adapters: raw_adapters,
            monitor_infos,
            targets,
            edids,
        } = raw;

        let mut adapters: Vec<Adapter> = Vec::with_capacity(raw_adapters.len());
        let mut devices: Vec<MonitorDevice> = Vec::new();
        // The monitors of the sources already in the tree, by id: the first device of
        // each id in tree order, as `AllMonitorDevices` groups them.
        let mut committed: HashMap<String, usize> = HashMap::new();

        for raw_adapter in raw_adapters {
            let name = raw_adapter.device.device_name;
            let caps = raw_adapter.capabilities;
            let capabilities = WindowsDeviceCaps {
                size: size("HORZSIZE x VERTSIZE", &name, caps.horz_size, caps.vert_size)?,
                resolution: size("HORZRES x VERTRES", &name, caps.horz_res, caps.vert_res)?,
                log_pixels: size(
                    "LOGPIXELSX x LOGPIXELSY",
                    &name,
                    caps.log_pixels_x,
                    caps.log_pixels_y,
                )?,
            };
            let a = adapters.len();
            let mut adapter = Adapter {
                id: raw_adapter.device.device_id,
                state_flags: raw_adapter.device.state_flags,
                has_monitor: false,
                adapter: WindowsAdapter {
                    device_name: name,
                    device_string: raw_adapter.device.device_string,
                    primary: false,
                    effective_dpi: Vector::default(),
                    angular_dpi: Vector::default(),
                    raw_dpi: Vector::default(),
                    current_mode: raw_adapter.current_mode.as_ref().map(display_mode),
                    capabilities,
                },
                connections: Vec::new(),
            };

            for raw_monitor in raw_adapter.monitors {
                let id = raw_monitor.device.device_id.as_str();
                let monitor = match committed.get(id) {
                    Some(&known) => {
                        if devices[known].interface_path.is_empty() {
                            devices[known].interface_path = raw_monitor.interface_path.clone();
                        }
                        known
                    }
                    None => {
                        let edid = find_edid(&edids, id);
                        devices.push(MonitorDevice::new(id, &raw_monitor.interface_path, edid));
                        devices.len() - 1
                    }
                };
                devices[monitor].connections.push(ConnectionRef {
                    adapter: a,
                    index: adapter.connections.len(),
                });
                adapter.connections.push(Connection {
                    device: raw_monitor.device,
                    monitor,
                });
            }

            for connection in &adapter.connections {
                committed
                    .entry(devices[connection.monitor].id.clone())
                    .or_insert(connection.monitor);
            }
            adapters.push(adapter);
        }

        for info in &monitor_infos {
            for adapter in adapters
                .iter_mut()
                .filter(|a| a.adapter.device_name == info.device_name)
            {
                adapter.has_monitor = true;
                adapter.adapter.primary = info.flags == 1;
                rect("monitor area", &info.device_name, info.monitor)?;
                rect("work area", &info.device_name, info.work)?;
                adapter.adapter.effective_dpi = info.effective_dpi.vector();
                adapter.adapter.angular_dpi = info.angular_dpi.vector();
                adapter.adapter.raw_dpi = info.raw_dpi.vector();
            }
        }

        let mut seen = HashSet::new();
        let mut order: Vec<usize> = adapters
            .iter()
            .flat_map(|a| a.connections.iter().map(|c| c.monitor))
            .filter(|&d| seen.insert(devices[d].id.clone()))
            .collect();
        order.sort_by(|&a, &b| invariant_compare(&devices[a].physical_id, &devices[b].physical_id));

        let ids: Vec<String> = order.iter().map(|&d| devices[d].id.clone()).collect();
        let ids: Vec<&str> = ids.iter().map(String::as_str).collect();
        let mut source_ids: Vec<String> = order
            .iter()
            .map(|&d| devices[d].source_id.clone())
            .collect();
        disambiguate_source_ids(&ids, &mut source_ids);
        for (&d, id) in order.iter().zip(source_ids) {
            devices[d].source_id = id;
        }

        // Both read the interface paths only, which neither changes.
        let paths: Vec<&str> = order
            .iter()
            .map(|&d| devices[d].interface_path.as_str())
            .collect();
        let numbers = number_monitors(&paths, targets.as_deref());
        let on_desktop: Vec<bool> = order
            .iter()
            .map(|&d| {
                devices[d]
                    .connections
                    .iter()
                    .any(|c| adapters[c.adapter].has_monitor)
            })
            .collect();
        let specialized = flag_specialized(&paths, &on_desktop, targets.as_deref());
        for ((&d, number), specialized) in order.iter().zip(numbers).zip(specialized) {
            devices[d].monitor_number = number;
            devices[d].is_specialized = specialized;
        }

        Ok(DisplayTree {
            adapters,
            devices,
            order,
        })
    }

    /// `root.AllMonitorDevices()`.
    pub fn monitors(&self) -> impl Iterator<Item = &MonitorDevice> {
        self.order.iter().map(|&d| &self.devices[d])
    }

    /// `MonitorDevice.ActiveConnection` ([`select_connection`]): the connection and its
    /// adapter.
    pub fn active_connection(&self, monitor: &MonitorDevice) -> Option<(&Adapter, &Connection)> {
        let connections: Vec<(&Adapter, &Connection)> = monitor
            .connections
            .iter()
            .map(|c| {
                let adapter = &self.adapters[c.adapter];
                (adapter, &adapter.connections[c.index])
            })
            .collect();
        let chosen =
            select_connection(
                connections
                    .iter()
                    .map(|(adapter, connection)| ConnectionChoice {
                        attached_to_desktop: connection.attached_to_desktop(),
                        has_monitor: adapter.has_monitor,
                        has_mode: adapter.adapter.current_mode.is_some(),
                        adapter_name: &adapter.adapter.device_name,
                    }),
            )?;
        Some(connections[chosen])
    }

    /// The monitors in `AllMonitorDevices` order as the layout model takes them.
    pub fn layout_input(&self) -> Vec<WindowsMonitor> {
        self.monitors()
            .map(|m| WindowsMonitor {
                id: m.id.clone(),
                pnp_code: m.pnp_code.clone(),
                source_id: m.source_id.clone(),
                interface_path: m.interface_path.clone(),
                monitor_number: m.monitor_number.clone(),
                is_specialized: m.is_specialized,
                edid: m.edid.as_ref().map(Edid::to_windows),
                active_connection: self.active_connection(m).map(|(adapter, connection)| {
                    WindowsConnection {
                        device_name: connection.device.device_name.clone(),
                        device_string: connection.device.device_string.clone(),
                        attached_to_desktop: connection.attached_to_desktop(),
                        adapter: adapter.adapter.clone(),
                    }
                }),
            })
            .collect()
    }
}

/// `new HLab.Geo.Size(width, height)`, which throws on a negative side.
fn size(
    what: &'static str,
    device_name: &str,
    width: i32,
    height: i32,
) -> Result<Size, DiscoveryError> {
    if width < 0 || height < 0 {
        return Err(DiscoveryError::NegativeSize {
            what,
            device_name: device_name.to_owned(),
            width: i64::from(width),
            height: i64::from(height),
        });
    }
    Ok(Size::new(f64::from(width), f64::from(height)))
}

/// `WinDef.Rect.ToRect()`, which builds an HLab.Geo `Rect` and throws on a negative
/// side. C# keeps the rectangle only for the wallpaper and the Info view.
fn rect(what: &'static str, device_name: &str, r: RawRect) -> Result<(), DiscoveryError> {
    if r.width() < 0 || r.height() < 0 {
        return Err(DiscoveryError::NegativeSize {
            what,
            device_name: device_name.to_owned(),
            width: r.width(),
            height: r.height(),
        });
    }
    Ok(())
}

/// C# `UpdateMonitorNumbers`, over the monitors' interface paths in enumeration order:
/// the number Settings > System > Display shows, which is the rank of the monitor's CCD
/// target among the available ones — adapters by LUID (high part, then low part),
/// targets by id. A target with no monitor still takes its number; the monitors CCD
/// does not account for (no query, a virtual or remote display) are numbered after,
/// in order.
pub fn number_monitors(interface_paths: &[&str], targets: Option<&[Target]>) -> Vec<String> {
    let mut numbers = vec![String::new(); interface_paths.len()];
    let mut remaining: Vec<usize> = (0..interface_paths.len()).collect();
    let mut number = 1;

    if let Some(targets) = targets {
        // A set of (adapter, id), as C#'s HashSet.
        let mut unique: Vec<&Target> = Vec::new();
        for target in targets {
            if !unique
                .iter()
                .any(|t| t.adapter == target.adapter && t.id == target.id)
            {
                unique.push(target);
            }
        }
        let mut adapters: Vec<Luid> = Vec::new();
        for target in &unique {
            if !adapters.contains(&target.adapter) {
                adapters.push(target.adapter);
            }
        }
        adapters.sort_by(|a, b| {
            a.high_part
                .cmp(&b.high_part)
                .then(a.low_part.cmp(&b.low_part))
        });

        for adapter in adapters {
            let mut on_adapter: Vec<&Target> = unique
                .iter()
                .copied()
                .filter(|t| t.adapter == adapter)
                .collect();
            on_adapter.sort_by_key(|t| t.id);
            for target in on_adapter {
                // Settings numbers every connected target.
                let n = number;
                number += 1;
                if target.device_path.is_empty() {
                    continue;
                }
                let Some(position) = remaining
                    .iter()
                    .position(|&m| interface_paths[m] == target.device_path)
                else {
                    continue;
                };
                numbers[remaining.remove(position)] = n.to_string();
            }
        }
    }

    for m in remaining {
        numbers[m] = number.to_string();
        number += 1;
    }
    numbers
}

/// C# `UpdateSpecializedMonitors`, over the monitors' interface paths in enumeration
/// order and whether a source of theirs is on the desktop: a monitor whose target has
/// specialization enabled (a VR headset, #364) is specialized — unless the desktop
/// shows it, since some systems report specialization on regular monitors (#506).
pub fn flag_specialized(
    interface_paths: &[&str],
    on_desktop: &[bool],
    targets: Option<&[Target]>,
) -> Vec<bool> {
    let mut specialized = vec![false; interface_paths.len()];
    let Some(targets) = targets else {
        return specialized;
    };
    for target in targets.iter().filter(|t| t.specialized) {
        if target.device_path.is_empty() {
            continue;
        }
        for (m, path) in interface_paths.iter().enumerate() {
            if *path != target.device_path || on_desktop.get(m).copied().unwrap_or(false) {
                continue;
            }
            specialized[m] = true;
        }
    }
    specialized
}
