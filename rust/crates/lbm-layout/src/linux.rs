//! How the Linux platform layer's description of an output becomes monitors,
//! models and sources: C#'s `LinuxLayoutMapping.AddMonitor`. Pure: the
//! enumeration (KScreen, xrandr, sysfs EDID) is the caller's business.
//!
//! The way back, applying positions to the compositor, is the platform's too,
//! all but its one pure step: [`snap_to_actual_sizes`].

use crate::geo::dotnet::{compare, max, min};
use crate::geo::{Point, Rect, Size};
use crate::model::{
    DisplaySize, DisplaySource, Layout, Monitor, MonitorModel, PhysicalSource, Ratio,
};
use crate::solve::IdMap;

/// One enumerated output, member for member C#'s `LinuxMonitor`.
#[derive(Clone, Debug, PartialEq)]
pub struct LinuxMonitor {
    pub connector_name: String,
    /// Position and size in the compositor's logical space (the cursor's space
    /// under Wayland).
    pub logical_x: f64,
    pub logical_y: f64,
    pub logical_width: f64,
    pub logical_height: f64,
    /// The current mode.
    pub pixel_width: i32,
    pub pixel_height: i32,
    pub scale: f64,
    /// As the source reports them: oriented.
    pub width_mm: f64,
    pub height_mm: f64,
    pub primary: bool,
    pub enabled: bool,
    /// Quarter turns.
    pub orientation: i32,
    pub frequency: i32,
    pub edid: Option<LinuxEdid>,
}

/// The EDID members `AddMonitor` reads. `None` strings are C# nulls.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LinuxEdid {
    pub manufacturer_code: Option<String>,
    /// Four uppercase hex digits.
    pub product_code: Option<String>,
    /// The binary serial, eight hex digits.
    pub serial: Option<String>,
    /// The 0xFF descriptor string, empty when absent.
    pub serial_number: Option<String>,
    /// The 0xFC descriptor string, empty when absent.
    pub model: Option<String>,
    /// First detailed timing's image size, in mm. Never rotated.
    pub physical_width: f64,
    pub physical_height: f64,
    pub video_interface: Option<String>,
}

fn non_empty(s: &Option<String>) -> Option<&str> {
    s.as_deref().filter(|s| !s.is_empty())
}

/// `LinuxLayoutMapping.AddMonitor`.
///
/// - PnP code: EDID manufacturer + product code, else the connector name.
/// - Monitor id: `{pnp}_{serial string, else binary serial}` with an EDID, the
///   connector name without; `@{connector}` appended when the id is taken
///   (identical monitors reporting the same serial).
/// - The model is created once per PnP code, with the *intrinsic* size: the
///   EDID's, or the source's un-rotated back (#511).
/// - Effective DPI is 96 per scale unit; raw DPI is mode pixels over oriented
///   millimetres.
/// - The monitor is handed to the layout before its source is registered, as
///   in C#, which matters for what the layout publishes in between.
pub fn add_monitor(layout: &mut Layout, monitor: &LinuxMonitor) {
    let edid = monitor.edid.as_ref();
    let connector = monitor.connector_name.as_str();
    let manufacturer = edid.and_then(|e| non_empty(&e.manufacturer_code));
    let pnp_code = match manufacturer {
        Some(m) => format!(
            "{m}{}",
            edid.and_then(|e| e.product_code.as_deref()).unwrap_or("")
        ),
        None => connector.to_owned(),
    };
    let mut monitor_id = match edid {
        Some(e) => format!(
            "{pnp_code}_{}",
            non_empty(&e.serial_number)
                .or(e.serial.as_deref())
                .unwrap_or("")
        ),
        None => connector.to_owned(),
    };
    if layout.monitors().iter().any(|m| m.id == monitor_id) {
        monitor_id = format!("{monitor_id}@{connector}");
    }

    let model = layout
        .get_or_add_model(&pnp_code, |code| {
            let mut model = MonitorModel::new(code);
            model.pnp_device_name = Some(
                edid.and_then(|e| non_empty(&e.model))
                    .unwrap_or(connector)
                    .to_owned(),
            );
            let (width_mm, height_mm) = match edid {
                Some(e) if e.physical_width > 0.0 && e.physical_height > 0.0 => {
                    (e.physical_width, e.physical_height)
                }
                _ if monitor.orientation % 2 != 0 => (monitor.height_mm, monitor.width_mm),
                _ => (monitor.width_mm, monitor.height_mm),
            };
            if width_mm > 0.0 && height_mm > 0.0 {
                let size = &mut model.physical_size;
                let fixed = size.fixed_aspect_ratio();
                size.set_fixed_aspect_ratio(false);
                size.set_width(width_mm);
                size.set_height(height_mm);
                size.set_fixed_aspect_ratio(fixed);
            }
            if let Some(m) = manufacturer {
                model.logo = Some(format!("icon/Pnp/{m}?icon/Pnp/LBM"));
            }
            model
        })
        .clone();

    let mut physical_monitor = Monitor::new(monitor_id.clone(), &model);
    physical_monitor.device_id = Some(connector.to_owned());
    physical_monitor.serial_number = Some(
        edid.and_then(|e| non_empty(&e.serial_number))
            .or(edid.and_then(|e| e.serial.as_deref()))
            .unwrap_or("N/A")
            .to_owned(),
    );

    let mut source = DisplaySource::new(monitor_id.clone());
    source.interface_path = Some(connector.to_owned());
    source.device_name = Some(connector.to_owned());
    source.display_name = Some(connector.to_owned());
    source.source_name = Some(format!(
        "{}:{connector}",
        edid.and_then(|e| e.video_interface.as_deref())
            .unwrap_or("Unknown")
    ));
    source.source_number = Some((layout.monitors().len() + 1).to_string());
    source.primary = monitor.primary;
    source.attached_to_desktop = monitor.enabled;
    source.orientation = monitor.orientation;
    source.display_frequency = monitor.frequency;
    let rect = Rect::from_location_size(
        Point::new(monitor.logical_x, monitor.logical_y),
        Size::new(monitor.logical_width, monitor.logical_height),
    );
    source.in_pixel = DisplaySize::from_rect(rect);
    let effective_dpi = 96.0 * monitor.scale;
    source.effective_dpi = Ratio::uniform(effective_dpi);
    source.dpi_aware_angular_dpi = Ratio::uniform(effective_dpi);
    let mut width_for_dpi = match edid {
        Some(e) if e.physical_width > 0.0 => e.physical_width,
        _ => monitor.width_mm,
    };
    let mut height_for_dpi = match edid {
        Some(e) if e.physical_height > 0.0 => e.physical_height,
        _ => monitor.height_mm,
    };
    if monitor.orientation % 2 != 0 && edid.is_some() {
        std::mem::swap(&mut width_for_dpi, &mut height_for_dpi);
    }
    source.raw_dpi = Ratio::new(
        if width_for_dpi > 0.0 {
            f64::from(monitor.pixel_width) * 25.4 / width_for_dpi
        } else {
            effective_dpi
        },
        if height_for_dpi > 0.0 {
            f64::from(monitor.pixel_height) * 25.4 / height_for_dpi
        } else {
            effective_dpi
        },
    );

    let physical_source = PhysicalSource::new(connector, monitor_id.clone(), source);
    physical_monitor.active_source = Some(monitor_id.clone());
    physical_monitor.sources.push(monitor_id);
    layout.attach_source(physical_source.clone());
    layout.add_or_update_monitor(physical_monitor);
    layout.add_or_update_source(physical_source);
}

impl LinuxMonitor {
    /// The output `Populate` invents when discovery found none: a plausible
    /// 1920 x 1080, 527 x 296 mm primary on a connector named `FALLBACK`.
    pub fn fallback() -> Self {
        Self {
            connector_name: "FALLBACK".to_owned(),
            logical_x: 0.0,
            logical_y: 0.0,
            logical_width: 1920.0,
            logical_height: 1080.0,
            pixel_width: 1920,
            pixel_height: 1080,
            scale: 1.0,
            width_mm: 527.0,
            height_mm: 296.0,
            primary: true,
            enabled: true,
            orientation: 0,
            frequency: 0,
            edid: None,
        }
    }
}

/// `LinuxLayoutFactory.Populate`, the domain half of building a layout from
/// the system: every output mapped (a fallback one when there is none), the
/// id computed, the stored layout loaded by `load`, the monitors the store did
/// not place placed from the system topology, then everything anchored on the
/// primary. A failed load stops there, as the C# exception does.
pub fn populate<E>(
    layout: &mut Layout,
    monitors: &[LinuxMonitor],
    load: impl FnOnce(&mut Layout) -> Result<(), E>,
) -> Result<(), E> {
    let fallback;
    let monitors = if monitors.is_empty() {
        fallback = [LinuxMonitor::fallback()];
        &fallback[..]
    } else {
        monitors
    };
    for monitor in monitors {
        add_monitor(layout, monitor);
    }
    layout.id = layout.compute_id();
    load(layout)?;
    layout.set_locations_from_system_configuration(false);
    layout.anchor_on_primary();
    Ok(())
}

/// One output placed by the pixel solver, on its way to the compositor: C#'s
/// `LinuxDisplayController.PlacedOutput`. The intended position comes with the
/// logical size the solver believed in (the *predicted* `round(native / scale)`)
/// and the one the compositor actually settled on.
#[derive(Clone, Debug, PartialEq)]
pub struct PlacedOutput {
    /// The connector name.
    pub name: String,
    pub x: f64,
    pub y: f64,
    pub predicted_width: f64,
    pub predicted_height: f64,
    pub actual_width: f64,
    pub actual_height: f64,
}

impl PlacedOutput {
    /// The positional constructor
    /// `PlacedOutput(Name, X, Y, PredictedWidth, PredictedHeight, ActualWidth, ActualHeight)`.
    pub fn new(
        name: impl Into<String>,
        x: f64,
        y: f64,
        predicted_width: f64,
        predicted_height: f64,
        actual_width: f64,
        actual_height: f64,
    ) -> Self {
        Self {
            name: name.into(),
            x,
            y,
            predicted_width,
            predicted_height,
            actual_width,
            actual_height,
        }
    }
}

/// `LinuxDisplayController.SnapToActualSizes`: rebuilds the intended edge
/// contacts with the actual output sizes. Wherever the solver meant two outputs
/// to touch (edge to edge within 1.5 px in its own predicted space, sharing more
/// than half a pixel of the perpendicular span), they are chained flush with the
/// sizes the compositor really applied, so a one-pixel rounding difference never
/// turns a contact into a gap or an overlap. An output with no contact on an axis
/// keeps its intended coordinate there.
///
/// Two passes, X then Y, each over the outputs ordered by that axis then the other
/// (a stable sort, like LINQ's `OrderBy().ThenBy()`). A contact found replaces the
/// intended coordinate, gaps closed as well as overlaps; with several neighbours
/// the farthest (`Math.Max`) wins. Neighbours chain on the positions already
/// snapped in the same pass. The result is keyed by name, in input order.
///
/// # Panics
///
/// On two outputs with the same name, where C#'s `ToDictionary` throws.
pub fn snap_to_actual_sizes(placed: &[PlacedOutput]) -> IdMap<Point> {
    const CONTACT_TOLERANCE: f64 = 1.5;

    // Does the pair share an edge span on the perpendicular axis, in intended space?
    fn overlap(a_lo: f64, a_length: f64, b_lo: f64, b_length: f64) -> bool {
        min(a_lo + a_length, b_lo + b_length) - max(a_lo, b_lo) > 0.5
    }

    let mut result = IdMap::new();
    for p in placed {
        assert!(
            !result.contains_key(&p.name),
            "two placed outputs are named {:?}",
            p.name
        );
        result.insert(&p.name, Point::new(p.x, p.y));
    }

    let mut by_x: Vec<&PlacedOutput> = placed.iter().collect();
    by_x.sort_by(|a, b| compare(a.x, b.x).then_with(|| compare(a.y, b.y)));
    for item in by_x {
        let mut chained: Option<f64> = None;
        for prior in placed {
            if prior.name == item.name {
                continue;
            }
            if !overlap(
                prior.y,
                prior.predicted_height,
                item.y,
                item.predicted_height,
            ) {
                continue;
            }
            if (prior.x + prior.predicted_width - item.x).abs() > CONTACT_TOLERANCE {
                continue;
            }
            let x = result[prior.name.as_str()].x + prior.actual_width;
            chained = Some(chained.map_or(x, |c| max(c, x)));
        }
        let y = result[item.name.as_str()].y;
        result.insert(&item.name, Point::new(chained.unwrap_or(item.x), y));
    }

    let mut by_y: Vec<&PlacedOutput> = placed.iter().collect();
    by_y.sort_by(|a, b| compare(a.y, b.y).then_with(|| compare(a.x, b.x)));
    for item in by_y {
        let mut chained: Option<f64> = None;
        for prior in placed {
            if prior.name == item.name {
                continue;
            }
            if !overlap(prior.x, prior.predicted_width, item.x, item.predicted_width) {
                continue;
            }
            if (prior.y + prior.predicted_height - item.y).abs() > CONTACT_TOLERANCE {
                continue;
            }
            let y = result[prior.name.as_str()].y + prior.actual_height;
            chained = Some(chained.map_or(y, |c| max(c, y)));
        }
        let x = result[item.name.as_str()].x;
        result.insert(&item.name, Point::new(x, chained.unwrap_or(item.y)));
    }

    result
}
