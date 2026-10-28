//! Changing a layout, with the side effects C# attaches to each change.
//!
//! In C# a change is a property write on a reactive object, and three kinds of
//! consequences follow from subscriptions:
//! - the *saved* flag: `SetUnsavedValue` properties mark their object unsaved
//!   when the value really changes (`EqualityComparer.Default`: NaN equals
//!   NaN), and an unsaved monitor, model, source or options object makes the
//!   layout unsaved; plain `RaiseAndSetIfChanged` properties do not;
//! - the *published* values: a change of any monitor's outside bounds, or a
//!   replaced depth projection, re-runs `ParsePhysicalMonitors`; a change of a
//!   source's effective DPI or primary flag re-runs `ParseDisplaySources`;
//! - the *mirror*: a monitor that does not own its borders follows the model's.
//!
//! Each method here is one C# write, with those consequences.

use crate::geo::{Point, Rect, Thickness, Vector};

use super::size::dotnet_equals;
use super::{BorderResistance, Layout, LayoutOptions, MmSize, Ratio, PER_MONITOR};

/// `Rect` equality as C#'s struct `Equals` sees it: field by field with
/// `double.Equals`.
fn same_rect(a: &Option<Rect>, b: &Option<Rect>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => {
            dotnet_equals(a.x(), b.x())
                && dotnet_equals(a.y(), b.y())
                && dotnet_equals(a.width(), b.width())
                && dotnet_equals(a.height(), b.height())
        }
        (None, None) => true,
        _ => false,
    }
}

fn same_thickness(a: Thickness, b: Thickness) -> bool {
    dotnet_equals(a.left, b.left)
        && dotnet_equals(a.top, b.top)
        && dotnet_equals(a.right, b.right)
        && dotnet_equals(a.bottom, b.bottom)
}

/// The `SetUnsavedValue` fields of `DisplaySizeInMm`: width, height, borders.
/// (X, Y and `FixedAspectRatio` are plain notifications.)
fn same_tracked_size(a: &MmSize, b: &MmSize) -> bool {
    dotnet_equals(a.width(), b.width())
        && dotnet_equals(a.height(), b.height())
        && same_thickness(a.borders(), b.borders())
}

/// The options `LbmOptions` writes through `SetUnsavedValue`. The others —
/// `Enabled`, `AutoUpdate`, `StartMinimized`, `StartElevated`, `DebugTools`,
/// `VcpControl`, `ShowMonitorActionWarning`, `Pinned`, `LoadAtStartup`,
/// `Elevated` — never mark the options unsaved.
fn same_tracked_options(a: &LayoutOptions, b: &LayoutOptions) -> bool {
    a.hide_tray_icon == b.hide_tray_icon
        && a.experimental_features == b.experimental_features
        && a.priority == b.priority
        && a.priority_unhooked == b.priority_unhooked
        && a.loop_x == b.loop_x
        && a.loop_y == b.loop_y
        && a.adjust_pointer == b.adjust_pointer
        && a.adjust_speed == b.adjust_speed
        && a.home_cinema == b.home_cinema
        && dotnet_equals(a.max_travel_distance, b.max_travel_distance)
        && dotnet_equals(a.freelook_check_interval, b.freelook_check_interval)
        && a.freelook_enabled == b.freelook_enabled
        && dotnet_equals(a.minimal_edge_overlap, b.minimal_edge_overlap)
        && a.allow_overlaps == b.allow_overlaps
        && a.allow_discontinuity == b.allow_discontinuity
        && a.algorithm == b.algorithm
        && a.border_values == b.border_values
        && a.touch_mouse_independent == b.touch_mouse_independent
        && a.stylus_mouse_independent == b.stylus_mouse_independent
        && a.restore_keyboard_focus == b.restore_keyboard_focus
        && a.focus_restore_delay == b.focus_restore_delay
        && a.focus_restore_on_mouse_move == b.focus_restore_on_mouse_move
        && a.touch_all_displays == b.touch_all_displays
        && a.touch_display_ids == b.touch_display_ids
        && a.touch_override_modifier == b.touch_override_modifier
        && a.focus_keep_apps == b.focus_keep_apps
        && a.focus_restore_apps == b.focus_restore_apps
        && a.rescue_shortcut == b.rescue_shortcut
        && a.excluded_list == b.excluded_list
}

impl Layout {
    /// Every monitor's outside bounds, to tell afterwards whether a change
    /// moved any of them (which is what re-runs `ParsePhysicalMonitors`).
    fn outside_bounds(&self) -> Vec<Option<Rect>> {
        self.monitors
            .iter()
            .map(|m| self.depth_projection(m).map(|dp| dp.outside_bounds()))
            .collect()
    }

    fn republish_if_moved(&mut self, before: &[Option<Rect>]) {
        let after = self.outside_bounds();
        if before.len() != after.len() || before.iter().zip(&after).any(|(a, b)| !same_rect(a, b)) {
            self.parse_physical_monitors();
        }
    }

    //==================//
    // Models           //
    //==================//

    /// Edits a model's physical size and name. A change of width, height or
    /// borders marks the model — hence its monitors and the layout — unsaved; a
    /// border change is mirrored onto every monitor of the model that does not
    /// own its borders (a mirror write marks that monitor unsaved too).
    pub fn edit_model(
        &mut self,
        pnp_code: &str,
        edit: impl FnOnce(&mut MmSize, &mut Option<String>),
    ) {
        let Some(index) = self.models.iter().position(|m| m.pnp_code == pnp_code) else {
            return;
        };
        let before = self.outside_bounds();
        let old_size = self.models[index].physical_size;
        let old_name = self.models[index].pnp_device_name.clone();
        {
            let model = &mut self.models[index];
            edit(&mut model.physical_size, &mut model.pnp_device_name);
        }
        let model = &self.models[index];
        if !same_tracked_size(&old_size, &model.physical_size) || old_name != model.pnp_device_name
        {
            self.saved = false;
        }
        let borders = model.physical_size.borders();
        if !same_thickness(old_size.borders(), borders) {
            for monitor in self.monitors.iter_mut() {
                if monitor.model == pnp_code
                    && !monitor.borders_customized
                    && !same_thickness(monitor.borders, borders)
                {
                    // `DisplayBorders` floors each side at zero, `Math.Max(0.0, v)`.
                    monitor.borders = non_negative(borders);
                    self.saved = false;
                }
            }
        }
        self.republish_if_moved(&before);
    }

    /// Sets a model's logo (a plain notification: never unsaved).
    pub fn set_model_logo(&mut self, pnp_code: &str, logo: Option<String>) {
        if let Some(model) = self.models.iter_mut().find(|m| m.pnp_code == pnp_code) {
            model.logo = logo;
        }
    }

    //==================//
    // Monitors         //
    //==================//

    fn edit_monitor<R>(
        &mut self,
        id: &str,
        edit: impl FnOnce(&mut super::Monitor) -> R,
    ) -> Option<R> {
        let index = self.monitor_index(id)?;
        Some(edit(&mut self.monitors[index]))
    }

    /// `DepthProjection.X/Y` (the `DisplayLocate` node): the monitor's
    /// top-left in mm.
    pub fn set_location(&mut self, id: &str, location: Point) {
        let before = self.outside_bounds();
        let changed = self
            .edit_monitor(id, |m| {
                let changed = !dotnet_equals(m.location.x, location.x)
                    || !dotnet_equals(m.location.y, location.y);
                m.location = location;
                changed
            })
            .unwrap_or(false);
        if changed {
            self.saved = false;
        }
        self.republish_if_moved(&before);
    }

    /// `projection.X += dx; projection.Y += dy`.
    pub fn offset_location(&mut self, id: &str, offset: Vector) {
        if let Some(m) = self.monitor(id) {
            let location = Point::new(m.location.x + offset.x, m.location.y + offset.y);
            self.set_location(id, location);
        }
    }

    /// `DepthRatio.Set(x, y)`.
    pub fn set_depth_ratio(&mut self, id: &str, ratio: Ratio) {
        let before = self.outside_bounds();
        let changed = self
            .edit_monitor(id, |m| {
                let changed = !dotnet_equals(m.depth_ratio.x, ratio.x)
                    || !dotnet_equals(m.depth_ratio.y, ratio.y);
                m.depth_ratio = ratio;
                changed
            })
            .unwrap_or(false);
        if changed {
            self.saved = false;
        }
        self.republish_if_moved(&before);
    }

    /// `ActiveSource`: a `SetUnsavedValue`, and the depth projection is rebuilt
    /// on the new source's orientation (a fresh projection is unsaved).
    pub fn set_active_source(&mut self, id: &str, source: Option<String>) {
        let changed = self
            .edit_monitor(id, |m| {
                let changed = m.active_source != source;
                m.active_source = source;
                changed
            })
            .unwrap_or(false);
        if changed {
            self.saved = false;
            self.parse_physical_monitors();
        }
    }

    /// `Placed`: a plain notification.
    pub fn set_placed(&mut self, id: &str, placed: bool) {
        self.edit_monitor(id, |m| m.placed = placed);
    }

    /// `ExcludedFromLayout`: a `SetUnsavedValue`.
    pub fn set_excluded(&mut self, id: &str, excluded: bool) {
        if self
            .edit_monitor(id, |m| {
                std::mem::replace(&mut m.excluded_from_layout, excluded) != excluded
            })
            .unwrap_or(false)
        {
            self.saved = false;
        }
    }

    /// `SerialNumber`: a `SetUnsavedValue`.
    pub fn set_serial_number(&mut self, id: &str, serial: Option<String>) {
        if self
            .edit_monitor(id, |m| {
                let changed = m.serial_number != serial;
                m.serial_number = serial;
                changed
            })
            .unwrap_or(false)
        {
            self.saved = false;
        }
    }

    /// One of the monitor's own borders (`DisplayBorders`, floored at zero).
    /// In per-monitor mode an edit makes the monitor own its borders from then
    /// on; any change marks it unsaved (`BordersChanged`).
    pub fn set_monitor_borders(&mut self, id: &str, borders: Thickness) {
        let before = self.outside_bounds();
        let per_monitor = self.options.border_values == PER_MONITOR;
        let changed = self
            .edit_monitor(id, |m| {
                let value = non_negative(borders);
                let changed = !same_thickness(m.borders, value);
                m.borders = value;
                if changed && per_monitor {
                    m.borders_customized = true;
                }
                changed
            })
            .unwrap_or(false);
        if changed {
            self.saved = false;
        }
        self.republish_if_moved(&before);
    }

    /// `BordersCustomized`: whether the monitor owns its borders (a plain
    /// notification). Set by the store when it holds borders for the monitor.
    pub fn set_borders_customized(&mut self, id: &str, customized: bool) {
        self.edit_monitor(id, |m| m.borders_customized = customized);
    }

    /// Edits the border-resistance sections; any change marks the monitor
    /// unsaved.
    pub fn edit_border_resistance(&mut self, id: &str, edit: impl FnOnce(&mut BorderResistance)) {
        let changed = self
            .edit_monitor(id, |m| {
                let old = m.border_resistance.clone();
                edit(&mut m.border_resistance);
                old != m.border_resistance
            })
            .unwrap_or(false);
        if changed {
            self.saved = false;
        }
    }

    //==================//
    // Sources          //
    //==================//

    /// `Primary`: a plain notification that re-runs `ParseDisplaySources`.
    pub fn set_source_primary(&mut self, source_id: &str, primary: bool) {
        if let Some(i) = self.source_index(source_id) {
            if self.sources[i].source.primary != primary {
                self.sources[i].source.primary = primary;
                // `AutoRefresh(e => e.Source.Primary)` lives on the layout's
                // source cache: only a registered source re-runs the parse.
                if self.sources[i].registered {
                    self.parse_display_sources();
                }
            }
        }
    }

    /// `Orientation`: the monitors showing this source rebuild their depth
    /// projection on it (unsaved, and republished).
    pub fn set_source_orientation(&mut self, source_id: &str, orientation: i32) {
        let Some(i) = self.source_index(source_id) else {
            return;
        };
        if self.sources[i].source.orientation == orientation {
            return;
        }
        self.sources[i].source.orientation = orientation;
        let shown = self
            .monitors
            .iter()
            .any(|m| m.active_source.as_deref() == Some(source_id));
        if shown {
            self.saved = false;
            self.parse_physical_monitors();
        }
    }

    /// `InPixel.Set(rect)`: the source's pixel rectangle. Nothing in C#
    /// propagates the source's saved flag, so the layout stays as it was.
    pub fn set_source_in_pixel(&mut self, source_id: &str, rect: Rect) {
        if let Some(i) = self.source_index(source_id) {
            let px = &mut self.sources[i].source.in_pixel;
            px.width = rect.width();
            px.height = rect.height();
            px.x = rect.x();
            px.y = rect.y();
        }
    }

    //==================//
    // Options          //
    //==================//

    /// Edits the options. A change of a `SetUnsavedValue` option marks the
    /// layout unsaved; a change of "Border values" rebuilds every monitor's
    /// depth projection on the other size (unsaved, republished).
    pub fn edit_options(&mut self, edit: impl FnOnce(&mut LayoutOptions)) {
        let before = self.options.clone();
        edit(&mut self.options);
        if !same_tracked_options(&before, &self.options) {
            self.saved = false;
        }
        if before.border_values != self.options.border_values {
            self.parse_physical_monitors();
        }
    }

    //==================//
    // Saved            //
    //==================//

    /// The store's `MarkSaved` walk: the layout and every source saved.
    pub fn mark_saved(&mut self) {
        self.saved = true;
        for source in &mut self.sources {
            source.saved = true;
        }
    }
}

/// `DisplayBorders.NonNegative`, per side: `Math.Max(0.0, value)`.
fn non_negative(t: Thickness) -> Thickness {
    use crate::geo::dotnet::max;
    Thickness::new(
        max(0.0, t.left),
        max(0.0, t.top),
        max(0.0, t.right),
        max(0.0, t.bottom),
    )
}
