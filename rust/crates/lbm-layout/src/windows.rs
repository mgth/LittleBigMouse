//! How the Windows platform layer's description of a monitor becomes monitors,
//! models and sources: C#'s `WindowsLayoutBuilder`, `WindowsSourceMapper` and
//! `WindowsPhysicalSize` (`LittleBigMouse.Platform.Windows`), `PnpName.Cleanup`,
//! and the domain half of `WindowsLayoutFactory.Create`. Pure: the Win32
//! enumeration (`HLab.Sys.Windows.Monitors`, ported in `lbm_display::windows`)
//! is the caller's business, and hands over a [`WindowsMonitor`] per monitor.
//!
//! Not ported: the desktop wallpaper `WindowsSourceMapper.UpdateFrom` copies at
//! the end (`WindowsWallpaperMapping.UpdateWallpaperFrom`, read through COM
//! `IDesktopWallpaper`). As on Linux, sources carry no wallpaper for now; nothing
//! of it reaches an id, a size or a DPI.
//!
//! C#'s culture-sensitive casing (`ToLower()` on the names Windows reports) is
//! done with the simple one-to-one case mapping, which is what every culture
//! but Turkish and Azeri does to those ASCII names.

use std::cmp::Ordering;

use crate::geo::{Point, Rect, Size, Vector};
use crate::model::{
    DisplaySize, DisplaySource, DpiAwareness, Layout, Monitor, MonitorModel, PhysicalSource, Ratio,
};

/// One monitor as the Windows enumeration leaves it: C#'s `MonitorDevice`,
/// with the members the mapping reads.
#[derive(Clone, Debug, PartialEq)]
pub struct WindowsMonitor {
    /// The monitor's device id, as `EnumDisplayDevices` reports it:
    /// `MONITOR\DEL4065\{4d36e96e-e325-11ce-bfc1-08002be10318}\0003`. It becomes
    /// the source's device id.
    pub id: String,
    /// The id's second segment (`DEL4065`): the key monitors share a model by.
    pub pnp_code: String,
    /// `{pnp}{serial string}_{week:X2}_{year:X4}_{checksum:X2}`, or
    /// `NOEDID_{pnp}_{instance}` without an EDID, with duplicates told apart:
    /// the monitor's id and its source's.
    pub source_id: String,
    /// The device interface path (`\\?\DISPLAY#DEL4065#...`), `""` when Windows
    /// did not give one.
    pub interface_path: String,
    /// The number Settings > System > Display shows.
    pub monitor_number: String,
    /// Bound to a specialized target (a VR headset) Windows keeps off the
    /// desktop: left out of the layout (#364).
    pub is_specialized: bool,
    pub edid: Option<WindowsEdid>,
    /// C#'s `ActiveConnection`: the connection taking part in the desktop, the
    /// likeliest one otherwise (`MonitorDevice.SelectConnection`); `None` for a
    /// monitor with no connection.
    pub active_connection: Option<WindowsConnection>,
}

/// The EDID members the mapping reads. `None` strings are C# nulls: a block too
/// short to hold them.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WindowsEdid {
    pub manufacturer_code: Option<String>,
    /// The 0xFC descriptor string, `""` when absent.
    pub model: Option<String>,
    /// The 0xFF descriptor string, `""` when absent.
    pub serial_number: Option<String>,
    /// `None` for an analog input.
    pub video_interface: Option<String>,
    /// First detailed timing's image size, in mm. Never rotated.
    pub physical_width: f64,
    pub physical_height: f64,
}

/// A monitor below an adapter source: C#'s `MonitorDeviceConnection`.
#[derive(Clone, Debug, PartialEq)]
pub struct WindowsConnection {
    /// `\\.\DISPLAY1\Monitor0`.
    pub device_name: String,
    /// The monitor's name as Windows has it: `Generic PnP Monitor`,
    /// `Dell U2720Q (DisplayPort)`.
    pub device_string: String,
    /// `DISPLAY_DEVICE_ATTACHED_TO_DESKTOP` on the monitor device.
    pub attached_to_desktop: bool,
    /// The source it hangs from: C#'s `Parent`.
    pub adapter: WindowsAdapter,
}

/// An adapter source (`\\.\DISPLAY1`): C#'s `PhysicalAdapter`, with what
/// `GetMonitorInfo` and `GetDpiForMonitor` add when the source has an
/// `HMONITOR` — for one that has none, not primary and every DPI (0, 0).
#[derive(Clone, Debug, PartialEq)]
pub struct WindowsAdapter {
    /// `\\.\DISPLAY1`.
    pub device_name: String,
    /// The GPU's name: `NVIDIA GeForce RTX 3080`.
    pub device_string: String,
    /// `MONITORINFOF_PRIMARY`.
    pub primary: bool,
    pub effective_dpi: Vector,
    pub angular_dpi: Vector,
    pub raw_dpi: Vector,
    /// `EnumDisplaySettingsEx(ENUM_CURRENT_SETTINGS)`; `None` for a source that
    /// has no mode (detached).
    pub current_mode: Option<WindowsDisplayMode>,
    /// `GetDeviceCaps` on the source.
    pub capabilities: WindowsDeviceCaps,
}

/// C#'s `DisplayMode` (`MonitorDeviceHelper.GetDisplayMode`), the members read.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WindowsDisplayMode {
    /// `dmPosition`: the source's top left on the desktop, in pixels.
    pub position: Point,
    /// `dmPelsWidth` x `dmPelsHeight`: they follow the orientation.
    pub pels: Size,
    /// `dmDisplayOrientation`, quarter turns.
    pub display_orientation: i32,
    pub display_frequency: i32,
}

/// C#'s `DeviceCaps`, the members read.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WindowsDeviceCaps {
    /// `HORZSIZE` x `VERTSIZE`, in mm. Drivers disagree on whether it follows
    /// the rotation; EDID-less displays report a square placeholder.
    pub size: Size,
    /// `HORZRES` x `VERTRES`: follows the current mode.
    pub resolution: Size,
    /// `LOGPIXELSX` x `LOGPIXELSY`.
    pub log_pixels: Size,
}

//=======================//
// WindowsPhysicalSize   //
//=======================//

/// `WindowsPhysicalSize.InferOrientation`: DEVMODE's orientation, unless the
/// driver rotated below Windows (NVIDIA panel rotation) and left it at 0 while
/// the pixel mode is already transposed (#507). The panel's EDID aspect cannot
/// rotate: when it contradicts the pixel aspect, the display is turned, and
/// this says 90°. Square or empty sizes (EDID-less virtual displays report
/// 0 x 0, #419) decide nothing.
pub fn infer_orientation(mode: &WindowsDisplayMode, edid: Option<&WindowsEdid>) -> i32 {
    if mode.display_orientation != 0 {
        return mode.display_orientation;
    }
    let Some(edid) = edid else {
        return 0;
    };
    if mode.pels.width() == mode.pels.height() {
        return 0;
    }
    if edid.physical_width <= 0.0
        || edid.physical_height <= 0.0
        || edid.physical_width == edid.physical_height
    {
        return 0;
    }
    let pixel_portrait = mode.pels.height() > mode.pels.width();
    let panel_portrait = edid.physical_height > edid.physical_width;
    if pixel_portrait == panel_portrait {
        0
    } else {
        1
    }
}

/// `WindowsPhysicalSize.GetPhysicalSizeInMm`: the panel's intrinsic size in
/// mm, never transposed to the current orientation — the model is shared by
/// every monitor of the make whatever its rotation, and the geometry turns it
/// downstream (#507).
///
/// GDI's `HORZSIZE`/`VERTSIZE` come first (TV EDIDs lie about their size, and
/// the stored models were built from GDI), normalized to the intrinsic
/// orientation against an aspect reference: the EDID's when it has a size, the
/// un-rotated resolution otherwise. A GDI size that fits neither way (the
/// square placeholder of a display without EDID) gives way to the EDID size,
/// then to resolution over DPI, then is taken anyway. A monitor without a
/// current mode gets its EDID size, or nothing.
pub fn physical_size_in_mm(monitor: &WindowsMonitor) -> (f64, f64) {
    let edid = monitor
        .edid
        .as_ref()
        .filter(|e| e.physical_width > 0.0 && e.physical_height > 0.0);
    let display = monitor.active_connection.as_ref().map(|c| &c.adapter);

    if let Some((display, mode)) = display.and_then(|d| d.current_mode.as_ref().map(|m| (d, m))) {
        let caps = &display.capabilities;
        let rotated = mode.display_orientation % 2 != 0;

        // Intrinsic (panel) resolution: HORZRES/VERTRES follow the current mode.
        let (res_w, res_h) = if rotated {
            (caps.resolution.height(), caps.resolution.width())
        } else {
            (caps.resolution.width(), caps.resolution.height())
        };

        // Intrinsic aspect reference (only the aspect is used: mm or px alike).
        let (ref_w, ref_h) = match edid {
            Some(e) => (e.physical_width, e.physical_height),
            None => (res_w, res_h),
        };

        let (gdi_w, gdi_h) = (caps.size.width(), caps.size.height());
        if is_aspect_consistent(gdi_w, gdi_h, ref_w, ref_h) {
            return (gdi_w, gdi_h);
        }
        if is_aspect_consistent(gdi_h, gdi_w, ref_w, ref_h) {
            return (gdi_h, gdi_w);
        }

        if let Some(e) = edid {
            return (e.physical_width, e.physical_height);
        }

        // inches = pixels / dpi
        let (dpi_w, dpi_h) = if rotated {
            (caps.log_pixels.height(), caps.log_pixels.width())
        } else {
            (caps.log_pixels.width(), caps.log_pixels.height())
        };
        if dpi_w > 0.0 && dpi_h > 0.0 {
            return (res_w / dpi_w * 25.4, res_h / dpi_h * 25.4);
        }

        return (gdi_w, gdi_h);
    }

    match edid {
        Some(e) => (e.physical_width, e.physical_height),
        None => (0.0, 0.0),
    }
}

/// `WindowsPhysicalSize.IsAspectConsistent`: the size's aspect within 12 % of
/// the reference's (square pixels). A 1000 x 1000 placeholder against 16:9
/// fails; so does anything not strictly positive.
pub fn is_aspect_consistent(
    width: f64,
    height: f64,
    pixels_width: f64,
    pixels_height: f64,
) -> bool {
    if width <= 0.0 || height <= 0.0 || pixels_width <= 0.0 || pixels_height <= 0.0 {
        return false;
    }
    let size_aspect = width / height;
    let pixel_aspect = pixels_width / pixels_height;
    (size_aspect / pixel_aspect - 1.0).abs() < 0.12
}

//=======================//
// WindowsSourceMapper   //
//=======================//

/// `WindowsSourceMapper.CreateDisplaySource`: a source with the monitor's
/// source id, filled by [`update_display_source`].
pub fn create_display_source(monitor: &WindowsMonitor) -> DisplaySource {
    let mut source = DisplaySource::new(monitor.source_id.clone());
    update_display_source(&mut source, monitor);
    source
}

/// `WindowsSourceMapper.UpdateFrom(DisplaySource, MonitorDevice)`: the
/// interface path, then — only with an active connection — names, primary,
/// attachment, the three DPIs, the mode (frequency, pixel rectangle, inferred
/// orientation; an empty rectangle and frequency 0 without one, the
/// orientation left as it was), the GPU brand and the Settings number.
///
/// C# returns early as well when the connection has no parent; a connection
/// always hangs from its adapter here.
pub fn update_display_source(source: &mut DisplaySource, monitor: &WindowsMonitor) {
    source.interface_path = Some(monitor.interface_path.clone());

    let Some(device) = &monitor.active_connection else {
        return;
    };
    let adapter = &device.adapter;

    source.display_name = Some(adapter.device_name.clone());
    source.device_name = Some(device.device_name.clone());
    source.source_name = Some(format!(
        "{}:{}",
        monitor
            .edid
            .as_ref()
            .and_then(|e| e.video_interface.as_deref())
            .unwrap_or("Unknown"),
        device.device_name
    ));

    source.primary = adapter.primary;
    source.attached_to_desktop = device.attached_to_desktop;

    source.effective_dpi = Ratio::new(adapter.effective_dpi.x, adapter.effective_dpi.y);
    source.dpi_aware_angular_dpi = Ratio::new(adapter.angular_dpi.x, adapter.angular_dpi.y);
    source.raw_dpi = Ratio::new(adapter.raw_dpi.x, adapter.raw_dpi.y);

    match &adapter.current_mode {
        Some(mode) => {
            source.display_frequency = mode.display_frequency;
            source.in_pixel =
                DisplaySize::from_rect(Rect::from_location_size(mode.position, mode.pels));
            source.orientation = infer_orientation(mode, monitor.edid.as_ref());
        }
        None => {
            source.display_frequency = 0;
            source.in_pixel = DisplaySize::from_rect(Rect::from_location_size(
                Point::new(0.0, 0.0),
                Size::new(0.0, 0.0),
            ));
        }
    }

    let (name, logo) = interface_brand_name_and_logo(adapter);
    source.interface_name = Some(name);
    source.interface_logo = Some(logo);

    // UpdateWallpaperFrom: not ported (see the module docs).

    source.source_number = Some(monitor.monitor_number.clone());
}

/// `WindowsSourceMapper.CreatePhysicalMonitorModel`: a model for `pnp_code`
/// with the monitor's intrinsic size (when both sides are positive), its
/// Windows name, and its brand logo.
pub fn create_monitor_model(monitor: &WindowsMonitor, pnp_code: &str) -> MonitorModel {
    let mut model = MonitorModel::new(pnp_code);

    // SetSizeFrom
    let size = &mut model.physical_size;
    let fixed = size.fixed_aspect_ratio();
    size.set_fixed_aspect_ratio(false);
    let (width, height) = physical_size_in_mm(monitor);
    if width > 0.0 && height > 0.0 {
        size.set_width(width);
        size.set_height(height);
    }
    size.set_fixed_aspect_ratio(fixed);

    set_pnp_device_name(&mut model, monitor);
    model.logo = Some(brand_logo(monitor));
    model
}

/// `WindowsSourceMapper.SetPnpDeviceName`: a model that has a name keeps it;
/// otherwise the monitor's Windows name, tidied, and the EDID's model name
/// instead of the generic one a monitor gets when Windows has no driver name
/// for it (a panel, a virtual display).
fn set_pnp_device_name(model: &mut MonitorModel, monitor: &WindowsMonitor) {
    if model
        .pnp_device_name
        .as_deref()
        .is_some_and(|n| !n.is_empty())
    {
        return;
    }
    let mut name = pnp_name_cleanup(
        monitor
            .active_connection
            .as_ref()
            .map_or("", |c| c.device_string.as_str()),
    );
    if simple_lowercase(&name) == "generic pnp monitor" {
        if let Some(edid_model) = monitor
            .edid
            .as_ref()
            .and_then(|e| e.model.as_deref())
            .filter(|m| !m.is_empty())
        {
            name = edid_model.to_owned();
        }
    }
    model.pnp_device_name = Some(name);
}

/// `WindowsSourceMapper.CreatePhysicalMonitor`: a monitor `id` of `model`,
/// with the monitor's device id and its EDID serial string — `"N/A"` when the
/// monitor has no EDID or a block too short to hold it, `""` when the EDID
/// has no serial descriptor.
pub fn create_physical_monitor(
    monitor: &WindowsMonitor,
    id: &str,
    model: &MonitorModel,
) -> Monitor {
    let mut physical = Monitor::new(id, model);
    physical.device_id = Some(monitor.id.clone());
    physical.serial_number = Some(
        monitor
            .edid
            .as_ref()
            .and_then(|e| e.serial_number.clone())
            .unwrap_or_else(|| "N/A".to_owned()),
    );
    physical
}

/// `WindowsSourceMapper.BrandLogo`: Spacedesk and Remote Desktop by their
/// adapter, Aorus by its model name, the EDID manufacturer otherwise (with
/// the LBM icon as the fallback the UI resolves), the LBM icon without EDID.
fn brand_logo(monitor: &WindowsMonitor) -> String {
    if let Some(dev) = monitor
        .active_connection
        .as_ref()
        .map(|c| c.adapter.device_string.as_str())
    {
        if simple_uppercase(dev).contains("SPACEDESK") {
            return "icon/Pnp/Spacedesk".to_owned();
        }
        if dev == "Microsoft Remote Display Adapter" {
            return "icon/Pnp/Microsoft".to_owned();
        }
    }
    let Some(edid) = &monitor.edid else {
        return "icon/Pnp/LBM".to_owned();
    };
    if edid.model.as_deref().is_some_and(|m| m.contains("Aorus")) {
        return "icon/Pnp/Aorus".to_owned();
    }
    format!(
        "icon/Pnp/{}?icon/Pnp/LBM",
        edid.manufacturer_code.as_deref().unwrap_or("")
    )
}

const BRANDS: [&str; 4] = ["intel", "amd", "nvidia", "microsoft"];

/// `WindowsSourceMapper.InterfaceBrandNameAndLogo`: the GPU's name in lower
/// case, and the logo of the first brand it contains (none otherwise).
///
/// C# answers `("detached", "icon/parts/detached")` for an adapter without a
/// parent; the enumeration hangs every adapter from its root, so that answer
/// is never given.
pub fn interface_brand_name_and_logo(adapter: &WindowsAdapter) -> (String, String) {
    let dev = simple_lowercase(&adapter.device_string);
    for brand in BRANDS {
        if dev.contains(brand) {
            return (dev, format!("icon/pnp/{brand}"));
        }
    }
    (dev, String::new())
}

/// `PnpName.Cleanup`: the name Windows reports, without `Drivers` and without
/// the first parenthesized part — every occurrence of it — then trimmed:
/// `Dell U2415 Drivers (DP)` is `Dell U2415`.
pub fn pnp_name_cleanup(name: &str) -> String {
    let mut result = name.replace("Drivers", "");
    // Regex.Match(result, @"\((.*?)\)", Singleline): from the first "(" to the
    // first ")" after it, when there is one; Replace removes every copy.
    if let Some(open) = result.find('(') {
        if let Some(close) = result[open + 1..].find(')') {
            let group = result[open..open + close + 2].to_owned();
            result = result.replace(&group, "");
        }
    }
    result.trim().to_owned()
}

/// `string.ToLower()`, one character at a time (see the module docs).
fn simple_lowercase(s: &str) -> String {
    s.chars()
        .map(|c| {
            let mut lower = c.to_lowercase();
            match (lower.next(), lower.next()) {
                (Some(l), None) => l,
                _ => c,
            }
        })
        .collect()
}

/// `ToUpperInvariant`, one character at a time: what `OrdinalIgnoreCase`
/// compares.
fn simple_uppercase(s: &str) -> String {
    s.chars()
        .map(|c| {
            let mut upper = c.to_uppercase();
            match (upper.next(), upper.next()) {
                (Some(u), None) => u,
                _ => c,
            }
        })
        .collect()
}

/// `StringComparer.OrdinalIgnoreCase.Compare`: UTF-16 code units, upper-cased.
pub fn ordinal_ignore_case_compare(a: &str, b: &str) -> Ordering {
    simple_uppercase(a)
        .encode_utf16()
        .cmp(simple_uppercase(b).encode_utf16())
}

//=======================//
// WindowsLayoutBuilder  //
//=======================//

/// `WindowsLayoutBuilder.AddOrUpdateMonitorDevice`: maps one monitor into the
/// layout.
///
/// - A source with the monitor's device id already there is refreshed in
///   place. `populate` never takes that branch: it maps onto a new layout, and
///   the enumeration gives each device id once.
/// - Otherwise the monitor id is the source id. A new id gets the model of its
///   PnP code (created from this monitor if the code is new: size, name,
///   logo), a monitor, and its source as the active one; the monitor is handed
///   to the layout before the source is registered, as in C#.
/// - An id already taken — two monitors the enumeration could not tell apart —
///   adds a second source to that monitor. Its display source has the same id
///   as the monitor's first, and replaces it among the layout's sources, as in
///   C#'s cache keyed by source id; the monitor lists the id twice. (C#'s
///   monitor keeps reading its first source object, which the layout no longer
///   holds; here both entries read the one the layout holds.)
pub fn add_or_update_monitor_device(layout: &mut Layout, monitor: &WindowsMonitor) {
    // `layout.PhysicalSources.FirstOrDefault(s => s.DeviceId == monitor.Id)`
    let refreshed = layout
        .sorted_sources()
        .into_iter()
        .find(|s| s.device_id == monitor.id)
        .map(|s| s.source.id.clone());
    if let Some(i) = refreshed.and_then(|id| layout.source_index(&id)) {
        update_display_source(&mut layout.sources[i].source, monitor);
        // C#'s caches republish on the source's changes; recomputed here, the
        // primary first since the monitors' values read it.
        layout.parse_display_sources();
        layout.parse_physical_monitors();
        return;
    }

    let id = monitor.source_id.as_str();
    match layout.monitor_index(id) {
        None => {
            // The model first: it defines the physical size.
            let model = layout
                .get_or_add_model(&monitor.pnp_code, |code| {
                    create_monitor_model(monitor, code)
                })
                .clone();
            let mut physical_monitor = create_physical_monitor(monitor, id, &model);
            let source = PhysicalSource::new(
                monitor.id.clone(),
                id.to_owned(),
                create_display_source(monitor),
            );
            physical_monitor.active_source = Some(source.source.id.clone());
            physical_monitor.sources.push(source.source.id.clone());
            layout.attach_source(source.clone());
            layout.add_or_update_monitor(physical_monitor);
            layout.add_or_update_source(source);
        }
        Some(index) => {
            // A new source for an existing monitor.
            let source = PhysicalSource::new(
                monitor.id.clone(),
                id.to_owned(),
                create_display_source(monitor),
            );
            layout.attach_new_source(source.clone());
            layout.monitors[index]
                .sources
                .push(source.source.id.clone());
            layout.add_or_update_source(source);
        }
    }
}

/// `WindowsLayoutBuilder.UpdateFrom`, the domain half of building a layout
/// from the system: the process's DPI awareness first, every monitor mapped
/// but the specialized ones (#364), the id computed, the stored layout loaded
/// by `load`, the monitors the store did not place placed from the system
/// topology, then everything anchored on the primary. Unlike Linux there is
/// no fallback monitor: no monitor is an empty layout. A failed load stops
/// there, as the C# exception does.
pub fn populate<E>(
    layout: &mut Layout,
    dpi_awareness: DpiAwareness,
    monitors: &[WindowsMonitor],
    load: impl FnOnce(&mut Layout) -> Result<(), E>,
) -> Result<(), E> {
    layout.dpi_awareness = dpi_awareness;
    for monitor in monitors {
        if monitor.is_specialized {
            continue;
        }
        add_or_update_monitor_device(layout, monitor);
    }
    layout.id = layout.compute_id();
    load(layout)?;
    layout.set_locations_from_system_configuration(false);
    layout.anchor_on_primary();
    Ok(())
}
