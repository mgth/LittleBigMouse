//! `ZoneSerializer`: the XML the C# UI puts in the daemon's `Load` command.
//!
//! C# derives the names from the members it is handed, in the order it is
//! handed them: attributes for scalars (in that order), then one element per
//! nested object, list or `Rect`. Elements are never self-closing. A null
//! string is an empty attribute. Doubles use .NET's shortest round-trip
//! formatting, booleans `True`/`False`, and attribute values are escaped the
//! way `SecurityElement.Escape` does it.

use std::fmt::Write;

use crate::geo::dotnet::format_double;
use crate::geo::Rect;

use super::{Zone, ZoneLink, ZonesLayout};

/// `SecurityElement.Escape`.
fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            '&' => out.push_str("&amp;"),
            _ => out.push(c),
        }
    }
    out
}

fn bool_text(b: bool) -> &'static str {
    if b {
        "True"
    } else {
        "False"
    }
}

struct Element {
    attributes: String,
    inside: String,
}

impl Element {
    fn new() -> Self {
        Self {
            attributes: String::new(),
            inside: String::new(),
        }
    }

    fn attr(&mut self, name: &str, value: &str) -> &mut Self {
        let _ = write!(self.attributes, " {name}=\"{}\"", escape(value));
        self
    }

    fn double(&mut self, name: &str, value: f64) -> &mut Self {
        self.attr(name, &format_double(value))
    }

    fn int(&mut self, name: &str, value: i32) -> &mut Self {
        self.attr(name, &value.to_string())
    }

    fn boolean(&mut self, name: &str, value: bool) -> &mut Self {
        self.attr(name, bool_text(value))
    }

    fn string(&mut self, name: &str, value: Option<&str>) -> &mut Self {
        self.attr(name, value.unwrap_or(""))
    }

    fn child(&mut self, name: &str, content: &str) -> &mut Self {
        let _ = write!(self.inside, "<{name}>{content}</{name}>");
        self
    }

    fn finish(&self, name: &str) -> String {
        format!("<{name}{}>{}</{name}>", self.attributes, self.inside)
    }
}

fn rect(r: &Rect) -> String {
    Element::new()
        .double("Left", r.left())
        .double("Top", r.top())
        .double("Width", r.width())
        .double("Height", r.height())
        .finish("Rect")
}

fn link(layout: &ZonesLayout, l: &ZoneLink) -> String {
    let target_id = l.target.map_or(-1, |t| layout.zones[t].id);
    Element::new()
        .double("From", l.from)
        .double("To", l.to)
        .int("SourceFromPixel", l.source_from_pixel)
        .int("SourceToPixel", l.source_to_pixel)
        .int("TargetFromPixel", l.target_from_pixel)
        .int("TargetToPixel", l.target_to_pixel)
        .double("BorderResistance", l.border_resistance)
        .boolean("MoveBlock", l.move_block)
        .double("DragResistance", l.drag_resistance)
        .boolean("DragBlock", l.drag_block)
        .int("TargetId", target_id)
        .finish("ZoneLink")
}

fn links(layout: &ZonesLayout, links: &[ZoneLink]) -> String {
    links.iter().map(|l| link(layout, l)).collect()
}

fn zone(layout: &ZonesLayout, z: &Zone) -> String {
    Element::new()
        .int("Id", z.id)
        .string("Name", z.name.as_deref())
        .string("DeviceId", z.device_id.as_deref())
        .child("PixelsBounds", &rect(&z.pixels_bounds))
        .child("PhysicalBounds", &rect(&z.physical_bounds))
        .child("LeftLinks", &links(layout, &z.left_links))
        .child("TopLinks", &links(layout, &z.top_links))
        .child("RightLinks", &links(layout, &z.right_links))
        .child("BottomLinks", &links(layout, &z.bottom_links))
        .finish("Zone")
}

pub(super) fn serialize(layout: &ZonesLayout) -> String {
    let zones: String = layout
        .main_zones
        .iter()
        .map(|&i| zone(layout, &layout.zones[i]))
        .collect();
    Element::new()
        .boolean("AdjustPointer", layout.adjust_pointer)
        .boolean("AdjustSpeed", layout.adjust_speed)
        .boolean("LoopX", layout.loop_x)
        .boolean("LoopY", layout.loop_y)
        .boolean("Virtual", layout.virtual_layout)
        .string("RescueShortcut", Some(&layout.rescue_shortcut))
        .boolean("TouchMouseIndependent", layout.touch_mouse_independent)
        .boolean("StylusMouseIndependent", layout.stylus_mouse_independent)
        .boolean("RestoreKeyboardFocus", layout.restore_keyboard_focus)
        .int("FocusRestoreDelay", layout.focus_restore_delay)
        .boolean(
            "FocusRestoreOnMouseMove",
            layout.focus_restore_on_mouse_move,
        )
        .boolean("TouchAllDisplays", layout.touch_all_displays)
        .string("TouchDisplayIds", Some(&layout.touch_display_ids))
        .string(
            "TouchOverrideModifier",
            Some(&layout.touch_override_modifier),
        )
        .string("FocusKeepApps", Some(&layout.focus_keep_apps))
        .string("FocusRestoreApps", Some(&layout.focus_restore_apps))
        .string("TouchDisplayBounds", Some(&layout.touch_display_bounds))
        .string("Priority", layout.priority.as_deref())
        .string("PriorityUnhooked", layout.priority_unhooked.as_deref())
        .string("Algorithm", Some(&layout.algorithm))
        .double("MaxTravelDistance", layout.max_travel_distance)
        .double("FreelookCheckInterval", layout.freelook_check_interval)
        .boolean("FreelookEnabled", layout.freelook_enabled)
        .child("MainZones", &zones)
        .finish("ZonesLayout")
}
