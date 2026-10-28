//! The zone compiler: C#'s `LittleBigMouse.Zoning` assembly.
//!
//! A [`ZonesLayout`] is what the hook routes on: one zone per displayed monitor
//! (pixel and millimetre rectangles), and for each edge the list of
//! [`ZoneLink`]s saying where the cursor lands when it leaves through each
//! stretch of that edge. [`ZonesLayout::init`] computes the links;
//! [`ZonesLayout::serialize`] writes the document the daemon's `Load` command
//! carries, byte for byte what the C# `ZoneSerializer` writes.
//!
//! This is the *producer* side. The hook's own parsed, runtime form of the
//! same document is `lbm_zones::ZonesLayout`.

mod factory;
mod xml;

pub use factory::compute_zones;

use crate::geo::dotnet::{max, min};
use crate::geo::Rect;
use crate::model::{BorderResistance, BorderSide, ResistanceValues};

/// One stretch of an edge and where it leads: C#'s `ZoneLink`.
#[derive(Clone, Debug, PartialEq)]
pub struct ZoneLink {
    /// Millimetres to the target, or the travel limit when there is none.
    pub distance: f64,
    /// Millimetre span along the edge, on the perpendicular axis; the outermost
    /// links run to `f64::MIN` / `f64::MAX`.
    pub from: f64,
    pub to: f64,
    pub source_from_pixel: i32,
    pub source_to_pixel: i32,
    pub target_from_pixel: i32,
    pub target_to_pixel: i32,
    /// Resistance opposing a plain move. Named `BorderResistance` on the wire,
    /// its historical name, so a daemon from before the move/drag split reads it.
    pub border_resistance: f64,
    pub move_block: bool,
    pub drag_resistance: f64,
    pub drag_block: bool,
    /// Index of the target zone in [`ZonesLayout::zones`], `None` for a wall.
    pub target: Option<usize>,
}

impl ZoneLink {
    fn resistance(&self) -> ResistanceValues {
        ResistanceValues {
            move_resistance: self.border_resistance,
            move_block: self.move_block,
            drag: self.drag_resistance,
            drag_block: self.drag_block,
        }
    }
}

/// One zone: C#'s `Zone`.
#[derive(Clone, Debug, PartialEq)]
pub struct Zone {
    /// Index in [`ZonesLayout::zones`], assigned by [`ZonesLayout::init`].
    pub id: i32,
    pub device_id: Option<String>,
    pub name: Option<String>,
    pub pixels_bounds: Rect,
    pub physical_bounds: Rect,
    pub border_resistance: BorderResistance,
    /// Index of the zone this one is a loop clone of; its own index when it is
    /// a main zone.
    pub main: usize,
    /// `Zone.Dpi`, computed by `Init` and never serialized.
    pub dpi: f64,
    pub left_links: Vec<ZoneLink>,
    pub top_links: Vec<ZoneLink>,
    pub right_links: Vec<ZoneLink>,
    pub bottom_links: Vec<ZoneLink>,
}

impl Zone {
    /// A zone to push at index `index`, main unless `main` says otherwise.
    pub fn new(
        index: usize,
        border_resistance: BorderResistance,
        device_id: Option<String>,
        name: Option<String>,
        pixels_bounds: Rect,
        physical_bounds: Rect,
        main: Option<usize>,
    ) -> Self {
        Self {
            id: 0,
            device_id,
            name,
            pixels_bounds,
            physical_bounds,
            border_resistance,
            main: main.unwrap_or(index),
            dpi: 0.0,
            left_links: Vec::new(),
            top_links: Vec::new(),
            right_links: Vec::new(),
            bottom_links: Vec::new(),
        }
    }
}

/// A compiled layout: C#'s `ZonesLayout`, with its defaults.
#[derive(Clone, Debug, PartialEq)]
pub struct ZonesLayout {
    pub adjust_pointer: bool,
    pub adjust_speed: bool,
    pub loop_x: bool,
    pub loop_y: bool,
    /// Zones of a virtual layout: the daemon refuses to hook them.
    pub virtual_layout: bool,
    pub touch_mouse_independent: bool,
    pub stylus_mouse_independent: bool,
    pub restore_keyboard_focus: bool,
    pub focus_restore_delay: i32,
    pub focus_restore_on_mouse_move: bool,
    pub touch_all_displays: bool,
    pub touch_display_ids: String,
    pub touch_override_modifier: String,
    pub focus_keep_apps: String,
    pub focus_restore_apps: String,
    pub touch_display_bounds: String,
    pub rescue_shortcut: String,
    pub priority: Option<String>,
    pub priority_unhooked: Option<String>,
    pub algorithm: String,
    pub max_travel_distance: f64,
    pub freelook_check_interval: f64,
    pub freelook_enabled: bool,
    /// Main zones first, then loop clones.
    pub zones: Vec<Zone>,
    /// Indices of the main zones, filled by [`ZonesLayout::init`].
    pub main_zones: Vec<usize>,
}

impl Default for ZonesLayout {
    fn default() -> Self {
        Self {
            adjust_pointer: false,
            adjust_speed: false,
            loop_x: false,
            loop_y: false,
            virtual_layout: false,
            touch_mouse_independent: false,
            stylus_mouse_independent: false,
            restore_keyboard_focus: false,
            focus_restore_delay: 120,
            focus_restore_on_mouse_move: false,
            touch_all_displays: true,
            touch_display_ids: "".to_owned(),
            touch_override_modifier: "None".to_owned(),
            focus_keep_apps: "".to_owned(),
            focus_restore_apps: "".to_owned(),
            touch_display_bounds: String::new(),
            rescue_shortcut: String::new(),
            priority: None,
            priority_unhooked: None,
            algorithm: "Strait".to_owned(),
            max_travel_distance: 200.0,
            freelook_check_interval: 100.0,
            freelook_enabled: true,
            zones: Vec::new(),
            main_zones: Vec::new(),
        }
    }
}

/// Which edge of a zone a link list is computed for, and the accessors C#
/// passes to `ComputeLinks` for it.
#[derive(Clone, Copy)]
enum Edge {
    Left,
    Top,
    Right,
    Bottom,
}

impl Edge {
    fn side(self, r: &BorderResistance) -> &BorderSide {
        match self {
            Edge::Left => &r.left,
            Edge::Top => &r.top,
            Edge::Right => &r.right,
            Edge::Bottom => &r.bottom,
        }
    }
    /// The coordinate of a candidate target facing this edge.
    fn near(self, z: &Zone) -> f64 {
        match self {
            Edge::Left => z.physical_bounds.right(),
            Edge::Top => z.physical_bounds.bottom(),
            Edge::Right => z.physical_bounds.left(),
            Edge::Bottom => z.physical_bounds.top(),
        }
    }
    /// The coordinate of this edge itself.
    fn far(self, z: &Zone) -> f64 {
        match self {
            Edge::Left => z.physical_bounds.left(),
            Edge::Top => z.physical_bounds.top(),
            Edge::Right => z.physical_bounds.right(),
            Edge::Bottom => z.physical_bounds.bottom(),
        }
    }
    /// Start of the span along the edge, in mm.
    fn start(self, z: &Zone) -> f64 {
        match self {
            Edge::Left | Edge::Right => z.physical_bounds.top(),
            Edge::Top | Edge::Bottom => z.physical_bounds.left(),
        }
    }
    /// End of the span along the edge, in mm.
    fn end(self, z: &Zone) -> f64 {
        match self {
            Edge::Left | Edge::Right => z.physical_bounds.bottom(),
            Edge::Top | Edge::Bottom => z.physical_bounds.right(),
        }
    }
    /// `(int)z.PixelsBounds.Top` and friends.
    fn start_pixel(self, z: &Zone) -> i32 {
        match self {
            Edge::Left | Edge::Right => z.pixels_bounds.top() as i32,
            Edge::Top | Edge::Bottom => z.pixels_bounds.left() as i32,
        }
    }
    fn end_pixel(self, z: &Zone) -> i32 {
        match self {
            Edge::Left | Edge::Right => z.pixels_bounds.bottom() as i32,
            Edge::Top | Edge::Bottom => z.pixels_bounds.right() as i32,
        }
    }
    fn direction(self) -> f64 {
        match self {
            Edge::Left | Edge::Top => -1.0,
            Edge::Right | Edge::Bottom => 1.0,
        }
    }
}

impl ZonesLayout {
    /// `ZonesLayout.Init`: main zones collected, every zone given its index as
    /// id and its DPI, then the links of every main zone computed.
    pub fn init(&mut self) {
        self.main_zones = (0..self.zones.len())
            .filter(|&i| self.zones[i].main == i)
            .collect();
        for i in 0..self.zones.len() {
            let zone = &mut self.zones[i];
            zone.id = i as i32;
            let dpi_x = zone.pixels_bounds.width() / (zone.physical_bounds.width() / 25.4);
            let dpi_y = zone.pixels_bounds.height() / (zone.physical_bounds.height() / 25.4);
            zone.dpi = (dpi_x * dpi_x + dpi_y * dpi_y).sqrt() / 2f64.sqrt();
            if self.zones[i].main == i {
                let left = self.compute_links(i, Edge::Left);
                let top = self.compute_links(i, Edge::Top);
                let right = self.compute_links(i, Edge::Right);
                let bottom = self.compute_links(i, Edge::Bottom);
                let zone = &mut self.zones[i];
                zone.left_links = left;
                zone.top_links = top;
                zone.right_links = right;
                zone.bottom_links = bottom;
            }
        }
    }

    /// `Zone.ComputeLinks` for one edge of zone `this`: a 1-D sweep along the
    /// edge, cut wherever another zone (clones included) starts or ends and
    /// wherever a section of this edge starts or ends; each interval gets the
    /// nearest zone fully covering it in the edge's direction, within the
    /// travel limit (the later zone wins a tie), and the resistance of the
    /// section covering its midpoint. Adjacent intervals merge only when they
    /// lead to the same zone with the same resistances.
    fn compute_links(&self, this: usize, edge: Edge) -> Vec<ZoneLink> {
        let me = &self.zones[this];
        let mut values = vec![f64::MIN, f64::MAX];
        // `if (!values.Contains(v)) values.Add(v)`: `double.Equals`, so NaN
        // is found and -0 is +0.
        let mut add = |v: f64| {
            if !values.iter().any(|&x| x == v || (x.is_nan() && v.is_nan())) {
                values.push(v);
            }
        };
        for (i, zone) in self.zones.iter().enumerate() {
            if i == this {
                continue;
            }
            add(edge.start(zone));
            add(edge.end(zone));
        }
        let side = edge.side(&me.border_resistance);
        let edge_origin = edge.start(me);
        let edge_end = edge.end(me);
        for section in &side.sections {
            add(edge_origin + section.from());
            add(edge_origin + section.to());
        }
        // `values.OrderBy(e => e)`: stable, `double.CompareTo` order.
        values.sort_by(|a, b| crate::geo::dotnet::compare(*a, *b));

        let resistance_over = |interval_from: f64, interval_to: f64| -> ResistanceValues {
            let low = max(interval_from, edge_origin);
            let high = min(interval_to, edge_end);
            if low < high {
                let probe = low + (high - low) / 2.0;
                for section in &side.sections {
                    if probe < edge_origin + section.from() {
                        continue;
                    }
                    if probe >= edge_origin + section.to() {
                        continue;
                    }
                    return ResistanceValues {
                        move_resistance: section.move_resistance(),
                        move_block: section.move_block(),
                        drag: section.drag(),
                        drag_block: section.drag_block(),
                    };
                }
            }
            ResistanceValues::default()
        };

        let mut links: Vec<ZoneLink> = Vec::new();
        for pair in values.windows(2) {
            let (from, to) = (pair[0], pair[1]);
            let mut target: Option<usize> = None;
            let mut best = self.max_travel_distance;
            if from <= edge.end(me) && to >= edge.start(me) {
                for (i, next) in self.zones.iter().enumerate() {
                    if i == this {
                        continue;
                    }
                    if edge.direction() * (edge.far(next) - edge.far(me)) < 0.0 {
                        continue;
                    }
                    if from < edge.start(next) || to > edge.end(next) {
                        continue;
                    }
                    let distance = edge.direction() * (edge.near(next) - edge.far(me));
                    if distance > best {
                        continue;
                    }
                    target = Some(i);
                    best = distance;
                }
                target = target.map(|t| self.zones[t].main);
            }

            let source_from = edge.start(me);
            let source_to = edge.end(me);
            let (target_from, target_to) = target.map_or((0.0, 0.0), |t| {
                (edge.start(&self.zones[t]), edge.end(&self.zones[t]))
            });
            let source_from_pixel = edge.start_pixel(me);
            let source_to_pixel = edge.end_pixel(me);
            let (target_from_pixel, target_to_pixel) = target.map_or((0, 0), |t| {
                (
                    edge.start_pixel(&self.zones[t]),
                    edge.end_pixel(&self.zones[t]),
                )
            });
            let resistance = resistance_over(from, to);

            match links.last_mut() {
                Some(last) if last.target == target && last.resistance().same_as(&resistance) => {
                    last.to = to;
                    last.source_to_pixel = interpolate(
                        to,
                        source_from,
                        source_to,
                        source_from_pixel,
                        source_to_pixel,
                    );
                    last.target_to_pixel = interpolate(
                        to,
                        target_from,
                        target_to,
                        target_from_pixel,
                        target_to_pixel,
                    );
                }
                _ => links.push(ZoneLink {
                    border_resistance: resistance.move_resistance,
                    move_block: resistance.move_block,
                    drag_resistance: resistance.drag,
                    drag_block: resistance.drag_block,
                    distance: best,
                    from,
                    to,
                    source_from_pixel: interpolate(
                        from,
                        source_from,
                        source_to,
                        source_from_pixel,
                        source_to_pixel,
                    ),
                    source_to_pixel: interpolate(
                        to,
                        source_from,
                        source_to,
                        source_from_pixel,
                        source_to_pixel,
                    ),
                    target_from_pixel: interpolate(
                        from,
                        target_from,
                        target_to,
                        target_from_pixel,
                        target_to_pixel,
                    ),
                    target_to_pixel: interpolate(
                        to,
                        target_from,
                        target_to,
                        target_from_pixel,
                        target_to_pixel,
                    ),
                    target,
                }),
            }
        }

        if links.is_empty() {
            links.push(ZoneLink {
                distance: f64::MAX,
                from: f64::MIN,
                to: f64::MAX,
                source_from_pixel: 0,
                source_to_pixel: 0,
                target_from_pixel: 0,
                target_to_pixel: 0,
                border_resistance: 0.0,
                move_block: false,
                drag_resistance: 0.0,
                drag_block: false,
                target: None,
            });
        }
        links
    }

    /// `ZonesLayout.Serialize`: the XML the daemon's `Load` command carries.
    pub fn serialize(&self) -> String {
        xml::serialize(self)
    }
}

/// `Interpolate`: the pixel at `value` on a millimetre span mapped onto a pixel
/// span. The sentinels map to `int.MaxValue` / `int.MinValue`. The double is
/// cast with .NET 9's saturating conversion (`as` in Rust), and the pixel
/// offset added with C#'s unchecked, wrapping `int` addition.
fn interpolate(value: f64, from_mm: f64, to_mm: f64, pixel_from: i32, pixel_to: i32) -> i32 {
    if value >= f64::MAX {
        return i32::MAX;
    }
    if value <= f64::MIN {
        return i32::MIN;
    }
    let length = to_mm - from_mm;
    let pixel_length = pixel_to.wrapping_sub(pixel_from);
    (((value - from_mm) * f64::from(pixel_length) / length) as i32).wrapping_add(pixel_from)
}
