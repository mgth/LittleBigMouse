//! Map oriented measurements back to the shared model and the monitor border policy.
use std::collections::HashMap;

use lbm_layout::model::Layout;
use lbm_ui::properties::{Edit, Location, Size};

pub fn location(layout: &Layout, id: &str) -> Option<Location> {
    let m = layout.monitor(id)?;
    Some(Location {
        x: m.location().x,
        y: m.location().y,
        scale_x: m.depth_ratio.x * 100.,
        scale_y: m.depth_ratio.y * 100.,
    })
}

pub fn size(layout: &Layout, id: &str) -> Option<Size> {
    let monitor = layout.monitors().iter().find(|m| m.id == id)?;
    let model = layout.model(&monitor.model)?;
    Some(Size {
        physical: layout.physical_rotated(monitor)?,
        locked: model.physical_size.fixed_aspect_ratio(),
        per_monitor_borders: layout.options.per_monitor_borders(),
        model: monitor.model.clone(),
    })
}

/// The model sizes as detected, before any stored profile is applied: what
/// [`lbm_store::layout_migrations::normalize_stored_size`] compares a stored size against
/// when the agent (or this window) applies a saved document to freshly detected screens.
#[derive(Clone, Debug, Default)]
pub struct Detected(HashMap<String, (f64, f64)>);

impl Detected {
    /// Read off a layout that has just been detected — before the store is applied.
    pub fn of(layout: &Layout) -> Self {
        Self(
            layout
                .models()
                .iter()
                .map(|m| {
                    let size = &m.physical_size;
                    (m.pnp_code.clone(), (size.width(), size.height()))
                })
                .collect(),
        )
    }
}

/// `detected` is only read for panel dimensions: see [`Detected`].
pub fn apply(layout: &mut Layout, detected: &Detected, id: &str, edit: Edit) -> Result<(), String> {
    let monitor = layout
        .monitors()
        .iter()
        .find(|m| m.id == id)
        .ok_or("monitor no longer present")?;
    match edit {
        Edit::X(v) | Edit::Y(v) => {
            if !v.is_finite() {
                return Err("position must be finite".into());
            }
            let mut point = monitor.location();
            if matches!(edit, Edit::X(_)) {
                point.x = v;
            } else {
                point.y = v;
            }
            layout.set_location(id, point);
            return Ok(());
        }
        Edit::ScaleX(v) | Edit::ScaleY(v) => {
            if !v.is_finite() || !(5. ..=2000.).contains(&v) {
                return Err("scale must be between 5 and 2000 percent".into());
            }
            let mut ratio = monitor.depth_ratio;
            if matches!(edit, Edit::ScaleX(_)) {
                ratio.x = v / 100.;
            } else {
                ratio.y = v / 100.;
            }
            layout.set_depth_ratio(id, ratio);
            return Ok(());
        }
        _ => {}
    }
    let rotation = monitor
        .active_source
        .as_deref()
        .and_then(|id| layout.source(id))
        .map(|s| s.source.orientation)
        .filter(|r| (0..=3).contains(r))
        .ok_or("no supported orientation for this monitor")?;
    let model_id = monitor.model.clone();
    let mut physical = layout.physical_rotated(monitor).ok_or("no active source")?;
    let current = layout
        .model(&model_id)
        .ok_or("missing monitor model")?
        .physical_size;
    if let Edit::Lock(locked) = edit {
        layout.edit_model(&model_id, |s, _| {
            s.set_fixed_aspect_ratio(locked);
        });
        return Ok(());
    }
    let number = match edit {
        Edit::Width(v)
        | Edit::Height(v)
        | Edit::Left(v)
        | Edit::Top(v)
        | Edit::Right(v)
        | Edit::Bottom(v)
        | Edit::OutsideWidth(v)
        | Edit::OutsideHeight(v) => v,
        Edit::Lock(_) | Edit::X(_) | Edit::Y(_) | Edit::ScaleX(_) | Edit::ScaleY(_) => {
            unreachable!()
        }
    };
    if !number.is_finite() || number < 0.0 {
        return Err("measurements must be finite and non-negative".into());
    }
    match edit {
        Edit::Width(_) | Edit::Height(_) => {
            if number <= 0.0 {
                return Err("panel dimensions must be greater than zero".into());
            }
            let mut changed = current;
            // Do not divide by a missing EDID dimension while the ratio lock is on.
            if current.fixed_aspect_ratio() && (current.width() <= 0.0 || current.height() <= 0.0) {
                return Err("turn off the aspect lock to enter missing panel dimensions".into());
            }
            if matches!(edit, Edit::Width(_)) == (rotation % 2 == 0) {
                changed.set_width(number);
            } else {
                changed.set_height(number);
            }
            if !changed.width().is_finite() || !changed.height().is_finite() {
                return Err("dimensions are too large".into());
            }
            // The store migrates pre-5.4.1 sizes, saved oriented, by transposing any stored
            // size whose landscape/portrait shape contradicts the detected panel (#507). An
            // edit that crosses that line would be saved, then read back — by the agent's
            // preview as by the next load — with its axes swapped: refuse it here, with the
            // very rule the store applies, rather than show one size and run another.
            let (intrinsic_width, intrinsic_height) = detected
                .0
                .get(&model_id)
                .copied()
                .unwrap_or((current.width(), current.height()));
            let read_back = lbm_store::layout_migrations::normalize_stored_size(
                intrinsic_width,
                intrinsic_height,
                changed.width(),
                changed.height(),
            );
            if read_back != (changed.width(), changed.height()) {
                return Err(
                    "a panel cannot turn from landscape to portrait or back: rotate the display instead"
                        .into(),
                );
            }
            layout.edit_model(&model_id, |s, _| *s = changed);
        }
        _ => {
            match edit {
                Edit::Left(v) => physical.left_border = v,
                Edit::Top(v) => physical.top_border = v,
                Edit::Right(v) => physical.right_border = v,
                Edit::Bottom(v) => physical.bottom_border = v,
                Edit::OutsideWidth(v) => {
                    let offset = (v - physical.outside_width()) / 2.0;
                    physical.left_border += offset;
                    physical.right_border += offset;
                }
                Edit::OutsideHeight(v) => physical.bottom_border += v - physical.outside_height(),
                _ => unreachable!(),
            }
            let borders = physical.rotate((4 - rotation) % 4).borders();
            if [borders.left, borders.top, borders.right, borders.bottom]
                .iter()
                .any(|v| !v.is_finite() || *v < 0.0)
                || !physical.outside_width().is_finite()
                || !physical.outside_height().is_finite()
            {
                return Err("outside dimensions must leave non-negative bezels".into());
            }
            if layout.options.per_monitor_borders() {
                layout.set_monitor_borders(id, borders);
            } else {
                layout.edit_model(&model_id, |s, _| {
                    s.set_left_border(borders.left);
                    s.set_top_border(borders.top);
                    s.set_right_border(borders.right);
                    s.set_bottom_border(borders.bottom);
                });
            }
        }
    }
    layout.compact();
    Ok(())
}

/// Flatten structured discovery data without confusing it with the edited model.
pub fn report(value: &serde_json::Value) -> Vec<(String, String)> {
    fn walk(prefix: &str, value: &serde_json::Value, rows: &mut Vec<(String, String)>) {
        match value {
            serde_json::Value::Object(object) => {
                for (key, value) in object {
                    let name = if prefix.is_empty() {
                        key.clone()
                    } else {
                        format!("{prefix} / {key}")
                    };
                    walk(&name, value, rows);
                }
            }
            serde_json::Value::Null => rows.push((prefix.into(), "Not reported".into())),
            serde_json::Value::String(s) => rows.push((prefix.into(), s.clone())),
            other => rows.push((prefix.into(), other.to_string())),
        }
    }
    let mut rows = Vec::new();
    walk("", value, &mut rows);
    rows
}

pub fn layout_info(layout: &Layout, id: &str) -> Vec<(String, String)> {
    let Some(m) = layout.monitors().iter().find(|m| m.id == id) else {
        return Vec::new();
    };
    let mut rows = vec![
        ("Monitor".into(), m.id.clone()),
        ("Model".into(), m.model.clone()),
        (
            "Serial".into(),
            m.serial_number
                .clone()
                .unwrap_or_else(|| "Not reported".into()),
        ),
        (
            "Excluded from mouse layout".into(),
            m.excluded_from_layout.to_string(),
        ),
    ];
    if let Some(size) = layout.physical_rotated(m) {
        rows.push((
            "Panel (edited, mm)".into(),
            format!("{:.2} × {:.2}", size.width, size.height),
        ));
        rows.push((
            "Outside (edited, mm)".into(),
            format!("{:.2} × {:.2}", size.outside_width(), size.outside_height()),
        ));
    }
    rows
}

/// Data for the original C# monitor overlays; unavailable values remain absent.
pub fn monitor_info(layout: &Layout, id: &str) -> Option<lbm_ui::monitor_info::Info> {
    let m = layout.monitor(id)?;
    let physical = layout.source(m.active_source.as_deref()?)?;
    let source = &physical.source;
    let model = layout.model(&m.model)?;
    Some(lbm_ui::monitor_info::Info {
        number: source
            .source_number
            .clone()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| {
                (layout
                    .monitors()
                    .iter()
                    .position(|m| m.id == id)
                    .unwrap_or(0)
                    + 1)
                .to_string()
            }),
        pnp: model.pnp_code.clone(),
        name: model.pnp_device_name.clone().unwrap_or_default(),
        connector: source
            .interface_name
            .clone()
            .or_else(|| source.source_name.clone())
            .unwrap_or_default(),
        serial: m.serial_number.clone().unwrap_or_default(),
        resolution: format!(
            "{:.0} × {:.0} px",
            source.in_pixel.width, source.in_pixel.height
        ),
        density: layout
            .real_dpi(physical)
            .filter(|d| d.x.is_finite() && d.y.is_finite())
            .map(|d| format!("{:.0} dpi", (d.x + d.y) / 2.))
            .unwrap_or_default(),
        diagonal: layout
            .diagonal(m)
            .filter(|d| d.is_finite() && *d > 0.)
            .map(|d| format!("{:.1} in", d / 25.4))
            .unwrap_or_default(),
        refresh: if source.display_frequency > 0 {
            format!("{} Hz", source.display_frequency)
        } else {
            String::new()
        },
        primary: source.primary,
        excluded: m.excluded_from_layout,
    })
}
