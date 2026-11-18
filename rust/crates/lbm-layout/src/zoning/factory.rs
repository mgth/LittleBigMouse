//! `ZonesLayoutFactory.ComputeZones`: a model layout to zones.

use crate::geo::Vector;
use crate::model::Layout;

use super::{Zone, ZonesLayout};

/// One zone per source that is its monitor's active source and is attached to
/// the desktop, in the layout's device-id order; none for a monitor excluded
/// from the layout unless it is the primary (#504). With loops, each zone gets
/// a clone shifted by the layout's width (and height) on either side; clones
/// only feed the link computation and are never serialized. The options are
/// copied before `init`, which reads the travel limit.
pub fn compute_zones(layout: &Layout) -> ZonesLayout {
    let mut zones = ZonesLayout::default();
    for source in layout.sorted_sources() {
        let Some(monitor) = layout.monitor(&source.monitor) else {
            continue;
        };
        if monitor.excluded_from_layout && !source.source.primary {
            continue;
        }
        let active = monitor.active_source.as_deref() == Some(source.source.id.as_str());
        if !(active && source.source.attached_to_desktop) {
            continue;
        }
        let Some(projection) = layout.depth_projection(monitor) else {
            continue;
        };
        let name = layout
            .model(&monitor.model)
            .and_then(|m| m.pnp_device_name.clone());
        let index = zones.zones.len();
        zones.zones.push(Zone::new(
            index,
            monitor.border_resistance.clone(),
            Some(source.source.id.clone()),
            name,
            source.source.in_pixel.bounds(),
            projection.bounds(),
            None,
        ));
    }

    let main_count = zones.zones.len();
    let bounds = layout.physical_bounds();
    let clone = |zones: &mut ZonesLayout, of: usize, shift: Vector| {
        let original = zones.zones[of].clone();
        let index = zones.zones.len();
        zones.zones.push(Zone::new(
            index,
            original.border_resistance,
            original.device_id,
            original.name,
            original.pixels_bounds,
            original.physical_bounds.translate(shift),
            Some(of),
        ));
    };
    if layout.options.loop_x {
        for i in 0..main_count {
            clone(&mut zones, i, Vector::new(-bounds.width(), 0.0));
            clone(&mut zones, i, Vector::new(bounds.width(), 0.0));
        }
    }
    if layout.options.loop_y {
        for i in 0..main_count {
            clone(&mut zones, i, Vector::new(0.0, -bounds.height()));
            clone(&mut zones, i, Vector::new(0.0, bounds.height()));
        }
    }

    let o = &layout.options;
    zones.max_travel_distance = o.max_travel_distance;
    zones.freelook_check_interval = o.freelook_check_interval;
    zones.freelook_enabled = o.freelook_enabled;
    zones.adjust_pointer = o.adjust_pointer;
    zones.adjust_speed = o.adjust_speed;
    zones.touch_mouse_independent = o.touch_mouse_independent;
    zones.stylus_mouse_independent = o.stylus_mouse_independent;
    zones.restore_keyboard_focus = o.restore_keyboard_focus;
    zones.focus_restore_delay = o.focus_restore_delay;
    zones.focus_restore_on_mouse_move = o.focus_restore_on_mouse_move;
    zones.touch_all_displays = o.touch_all_displays;
    zones.touch_display_ids = o.touch_display_ids.clone();
    zones.touch_override_modifier = o.touch_override_modifier.clone();
    zones.focus_keep_apps = o.focus_keep_apps.replace('\r', "").replace('\n', ";");
    zones.focus_restore_apps = o.focus_restore_apps.replace('\r', "").replace('\n', ";");
    if !o.touch_all_displays {
        zones.touch_display_bounds = layout
            .sorted_sources()
            .into_iter()
            .filter(|s| {
                s.source.attached_to_desktop
                    && layout
                        .monitor(&s.monitor)
                        .is_some_and(|m| m.active_source.as_deref() == Some(s.source.id.as_str()))
                    && o.touch_display_ids.split(';').any(|id| id == s.source.id)
            })
            .map(|s| {
                let b = s.source.in_pixel.bounds();
                format!("{},{},{},{}", b.left(), b.top(), b.width(), b.height())
            })
            .collect::<Vec<_>>()
            .join(";");
    }

    zones.rescue_shortcut = o.rescue_shortcut.clone();
    zones.algorithm = o.algorithm.clone();
    zones.priority = Some(o.priority.clone());
    zones.priority_unhooked = Some(o.priority_unhooked.clone());
    zones.loop_x = o.loop_x;
    zones.loop_y = o.loop_y;
    zones.virtual_layout = layout.is_virtual();
    zones.init();
    zones
}
