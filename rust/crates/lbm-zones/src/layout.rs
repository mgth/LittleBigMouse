//! Port of `Engine/ZonesLayout` — owns the zone arena and parses the XML layout.

use std::collections::HashMap;

use roxmltree::{Document, Node};
use slotmap::SlotMap;

use super::xml::{
    child, get_bool, get_f64, get_f64_opt, get_i32, get_rect_f64, get_rect_i32, get_string,
};
use super::zone::Zone;
use super::zone_link::ZoneLink;
use super::ZoneId;
use crate::geometry::{Point, Rect};
use crate::priority::Priority;

/// C++ `enum Algorithm {Strait, CornerCrossing}`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Algorithm {
    Strait,
    CornerCrossing,
}

pub struct ZonesLayout {
    pub arena: SlotMap<ZoneId, Zone>,
    /// The zones in play, in document order: this machine's, and the other machines'
    /// only while [`Self::set_remote_enterable`] says so. Everything that routes, clips
    /// or draws barriers walks this list or [`Self::main_zones`], so a zone left out of
    /// it does not exist for them — links to it included.
    pub zones: Vec<ZoneId>,
    pub main_zones: Vec<ZoneId>,
    /// Every zone the document described, in play or not.
    pub all_zones: Vec<ZoneId>,

    pub max_travel_distance_squared: f64,
    pub freelook_check_interval_ms: i64,
    /// Freelook detection can misfire (apps hiding or confining the cursor
    /// without being games, #502): the UI exposes an off switch.
    pub freelook_enabled: bool,
    pub adjust_pointer: bool,
    pub adjust_speed: bool,
    pub algorithm: Algorithm,
    pub priority: Priority,
    pub priority_unhooked: Priority,
    /// Windows mouse resumes from its saved position after touchscreen input.
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
    /// The panic shortcut, as the UI spells it (`Ctrl+Alt+Shift+M`). Travels with
    /// the layout like every other daemon-side setting, so it reaches the daemon on
    /// the startup replay too. Empty means "whatever the daemon defaults to".
    pub rescue_shortcut: String,
    pub loop_x: bool,
    pub loop_y: bool,
    /// Zones of a virtual (foreign) layout, shown for inspection on another
    /// machine. The daemon loads them into the engine for validation but
    /// refuses to install the input hook on them (see `daemon::run`).
    pub virtual_layout: bool,

    left: f64,
    top: f64,
    right: f64,
    bottom: f64,

    /// Travel-rect cache, keyed by `(source, target.main)`. Lifted out of `Zone`
    /// (unlike the C++ `Zone::_travels`) so `Move` can mutate it without
    /// aliasing the arena.
    travels: HashMap<(ZoneId, ZoneId), Vec<Rect<i32>>>,

    /// Whether the zones of other machines are in play (docs/multi-pc.md). Off until
    /// someone able to drive another machine turns it on: only the evdev backend can,
    /// and only while its input stream has a client. Every other backend moves the
    /// real cursor in real pixels, and a remote zone's pixels do not exist here.
    remote_enterable: bool,
    /// The other machines whose zones are in play while [`Self::remote_enterable`]:
    /// `None` for every one (a master whose agent names none), else only those
    /// ([`Self::set_drivable_hosts`]).
    drivable: Option<Vec<String>>,
}

/// A point of the engine's pixel space, seen from the machine it is on.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HostPoint<'a> {
    /// The machine id; `None` for this machine.
    pub host: Option<&'a str>,
    /// In that machine's own desktop pixels.
    pub pixel: Point<i32>,
}

impl Default for ZonesLayout {
    fn default() -> Self {
        ZonesLayout {
            arena: SlotMap::with_key(),
            zones: Vec::new(),
            main_zones: Vec::new(),
            all_zones: Vec::new(),
            // C++ ZonesLayout member initializers.
            max_travel_distance_squared: 200.0 * 200.0,
            freelook_check_interval_ms: 100,
            freelook_enabled: true,
            adjust_pointer: false,
            adjust_speed: false,
            algorithm: Algorithm::Strait,
            priority: Priority::Normal,
            priority_unhooked: Priority::Above,
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
            loop_x: false,
            loop_y: false,
            virtual_layout: false,
            left: 0.0,
            top: 0.0,
            right: 0.0,
            bottom: 0.0,
            travels: HashMap::new(),
            remote_enterable: false,
            drivable: None,
        }
    }
}

impl ZonesLayout {
    /// Parse a `<ZonesLayout>` document (or any XML containing one).
    pub fn from_xml(xml: &str) -> Option<ZonesLayout> {
        let doc = Document::parse(xml).ok()?;
        let root = doc.root_element();
        let layout_el = if root.has_tag_name("ZonesLayout") {
            root
        } else {
            root.descendants().find(|n| n.has_tag_name("ZonesLayout"))?
        };
        Some(Self::load_from_element(layout_el))
    }

    fn load_from_element(el: Node) -> ZonesLayout {
        let mut layout = ZonesLayout::default();

        let max_travel = get_f64(el, "MaxTravelDistance");
        layout.max_travel_distance_squared = max_travel * max_travel;
        // Missing (older UI) -> 0 -> re-check every event (pre-throttle behavior).
        layout.freelook_check_interval_ms = get_i32(el, "FreelookCheckInterval") as i64;
        layout.freelook_enabled = get_bool(el, "FreelookEnabled", true);
        layout.adjust_pointer = get_bool(el, "AdjustPointer", false);
        layout.adjust_speed = get_bool(el, "AdjustSpeed", false);
        layout.loop_x = get_bool(el, "LoopX", false);
        layout.loop_y = get_bool(el, "LoopY", false);
        // Absent on layouts from older UIs -> false: only a UI that actually
        // flags virtual layouts can produce one.
        layout.virtual_layout = get_bool(el, "Virtual", false);
        // "Cross" is the wire value: it is what every shipped UI writes, and the only
        // spelling this parser used to accept. "CornerCrossing" is tolerated as an alias
        // because it is what the C# doc comment and the persistence fixtures said for a
        // long time — no release ever wrote it, but a hand-edited config or a migration
        // that trusted the documentation would, and silently getting Strait instead is
        // exactly the kind of thing nobody thinks to report. Anything else is still
        // Strait: an unknown algorithm must not be an error.
        layout.algorithm = match get_string(el, "Algorithm").as_str() {
            "Cross" | "CornerCrossing" => Algorithm::CornerCrossing,
            _ => Algorithm::Strait,
        };
        layout.priority = Priority::parse(&get_string(el, "Priority"));
        layout.priority_unhooked = Priority::parse(&get_string(el, "PriorityUnhooked"));
        layout.touch_mouse_independent = get_bool(el, "TouchMouseIndependent", false);
        layout.stylus_mouse_independent = get_bool(el, "StylusMouseIndependent", false);
        layout.restore_keyboard_focus = get_bool(el, "RestoreKeyboardFocus", false);
        layout.focus_restore_delay = el
            .attribute("FocusRestoreDelay")
            .and_then(|v| v.parse::<i32>().ok())
            .unwrap_or(120)
            .clamp(0, 5000);
        layout.focus_restore_on_mouse_move = get_bool(el, "FocusRestoreOnMouseMove", false);
        layout.touch_all_displays = get_bool(el, "TouchAllDisplays", true);
        layout.touch_display_ids = get_string(el, "TouchDisplayIds");
        layout.touch_override_modifier = el
            .attribute("TouchOverrideModifier")
            .unwrap_or("None")
            .to_owned();
        layout.focus_keep_apps = get_string(el, "FocusKeepApps");
        layout.focus_restore_apps = get_string(el, "FocusRestoreApps");
        layout.touch_display_bounds = get_string(el, "TouchDisplayBounds");
        layout.rescue_shortcut = get_string(el, "RescueShortcut");

        if let Some(main_zones) = child(el, "MainZones") {
            // Track (pixel bounds -> first ZoneId) for clone detection.
            let mut by_bounds: Vec<(Rect<i32>, ZoneId)> = Vec::new();

            for zone_el in main_zones.children().filter(|c| c.has_tag_name("Zone")) {
                let id = get_i32(zone_el, "Id");
                let device_id = get_string(zone_el, "DeviceId");
                let name = get_string(zone_el, "Name");
                let pixels_bounds = get_rect_i32(zone_el, "PixelsBounds");
                let physical_bounds = get_rect_f64(zone_el, "PhysicalBounds");
                let left = parse_links(child(zone_el, "LeftLinks"));
                let top = parse_links(child(zone_el, "TopLinks"));
                let right = parse_links(child(zone_el, "RightLinks"));
                let bottom = parse_links(child(zone_el, "BottomLinks"));

                // C++ GetNewZone: a new zone whose PixelsBounds equal an earlier
                // one's is a clone whose Main is that earlier zone.
                let clone_main = by_bounds
                    .iter()
                    .find(|(b, _)| *b == pixels_bounds)
                    .map(|(_, id)| *id);

                // Not in C#: the machine a zone is on (docs/multi-pc.md). Absent: this
                // machine's, as in every document from before the multi-PC mode.
                let host = zone_el.attribute("Host").map(str::to_owned);
                let host_offset = Point::new(get_i32(zone_el, "HostX"), get_i32(zone_el, "HostY"));

                let key = layout.arena.insert_with_key(|k| {
                    Zone::new(
                        id,
                        device_id,
                        name,
                        pixels_bounds,
                        physical_bounds,
                        clone_main.unwrap_or(k),
                        left,
                        top,
                        right,
                        bottom,
                    )
                });
                if host.is_some() {
                    let zone = &mut layout.arena[key];
                    zone.host = host;
                    zone.host_offset = host_offset;
                }
                by_bounds.push((pixels_bounds, key));
                layout.all_zones.push(key);
            }
        }

        layout.init();
        layout
    }

    /// C++ `ZonesLayout::Init`: compute overall bounds, collect main zones, and
    /// resolve each link's target from its `target_id`.
    ///
    /// Over the zones in play only, and run again whenever that set changes: a link
    /// whose target is out of play resolves to no target, which is the wall the UI's
    /// compiler would have written had that zone not been there.
    fn init(&mut self) {
        let remote = self.remote_enterable;
        let drivable = self.drivable.as_deref();
        let arena = &self.arena;
        self.zones = self
            .all_zones
            .iter()
            .copied()
            .filter(|&z| match arena[z].host.as_deref() {
                None => true,
                Some(host) => remote && drivable.is_none_or(|d| d.iter().any(|h| h == host)),
            })
            .collect();
        self.main_zones.clear();
        self.travels.clear();
        (self.left, self.top, self.right, self.bottom) = (0.0, 0.0, 0.0, 0.0);
        let zids = self.zones.clone();

        // First occurrence of each zone id (C++ InitZoneLinks scans in order).
        let mut id_map: HashMap<i32, ZoneId> = HashMap::new();
        for &zid in &zids {
            id_map.entry(self.arena[zid].id).or_insert(zid);
        }

        for &zid in &zids {
            let pb = self.arena[zid].physical_bounds();
            if pb.left() < self.left {
                self.left = pb.left();
            }
            if pb.top() < self.top {
                self.top = pb.top();
            }
            if pb.right() > self.right {
                self.right = pb.right();
            }
            if pb.bottom() > self.bottom {
                self.bottom = pb.bottom();
            }
            if self.arena[zid].main == zid {
                self.main_zones.push(zid);
            }
        }

        for &zid in &zids {
            let zone = &mut self.arena[zid];
            for link in zone
                .left
                .iter_mut()
                .chain(zone.top.iter_mut())
                .chain(zone.right.iter_mut())
                .chain(zone.bottom.iter_mut())
            {
                link.target = id_map.get(&link.target_id).copied();
            }
        }
    }

    /// Whether the zones of other machines are in play.
    pub fn remote_enterable(&self) -> bool {
        self.remote_enterable
    }

    /// Puts the zones of other machines in play, or takes them out. Rebuilds the zone
    /// lists, the extent and the links when it changes anything, so it is not for a
    /// per-report path; a call that changes nothing costs a comparison.
    pub fn set_remote_enterable(&mut self, enterable: bool) {
        if self.remote_enterable == enterable {
            return;
        }
        self.remote_enterable = enterable;
        self.init();
    }

    /// Which other machines' zones may be in play: `None` for every one, as before an
    /// agent named any; `Some` for those only — the machines the agent can relay to
    /// now (`Command::Drivable`). A zone of any other machine is out of play, and a
    /// border to it a wall, even while [`Self::remote_enterable`]. On the routing path:
    /// compared as given, borrowed, and copied only when it changed, so the caller
    /// hands the same normalised list every time (the hook sorts it on arrival). The
    /// zones are rebuilt only then, like [`Self::set_remote_enterable`].
    pub fn set_drivable_hosts(&mut self, hosts: Option<&[String]>) {
        if self.drivable.as_deref() == hosts {
            return;
        }
        self.drivable = hosts.map(<[String]>::to_vec);
        self.init();
    }

    /// Whether the document has zones of other machines at all, in play or not.
    pub fn has_remote_zones(&self) -> bool {
        self.all_zones.iter().any(|&z| self.arena[z].host.is_some())
    }

    /// C++ `ZonesLayout::Containing(Point<long>)`.
    pub fn containing_pixel(&self, pixel: Point<i32>) -> Option<ZoneId> {
        self.main_zones
            .iter()
            .copied()
            .find(|&z| self.arena[z].contains_pixel(pixel))
    }

    /// C++ `ZonesLayout::Containing(Point<double>)`.
    pub fn containing_mm(&self, mm: Point<f64>) -> Option<ZoneId> {
        self.main_zones
            .iter()
            .copied()
            .find(|&z| self.arena[z].contains_mm(mm))
    }

    /// Which machine a point of the engine's pixel space is on, and the same point on
    /// that machine's own desktop (docs/multi-pc.md, "Engine pixel space"). `None`
    /// outside every zone in play.
    pub fn host_point(&self, pixel: Point<i32>) -> Option<HostPoint<'_>> {
        let zone = &self.arena[self.containing_pixel(pixel)?];
        Some(HostPoint {
            host: zone.host.as_deref(),
            pixel: zone.to_host_pixel(pixel),
        })
    }

    pub fn width(&self) -> f64 {
        40.0 + self.right - self.left
    }

    pub fn height(&self) -> f64 {
        40.0 + self.bottom - self.top
    }

    /// C++ `Zone::TravelPixels` — the cached clip-rect path from `source` to
    /// `target` (keyed, as in the C++, by the target's `Main` so clones share a
    /// path). Computed lazily.
    pub fn travel_pixels(&mut self, source: ZoneId, target: ZoneId) -> Vec<Rect<i32>> {
        let target_main = self.arena[target].main;
        let key = (source, target_main);
        if let Some(cached) = self.travels.get(&key) {
            return cached.clone();
        }
        let computed = self.compute_travel_pixels(source, target);
        self.travels.insert(key, computed.clone());
        computed
    }

    /// C++ `Zone::GetTravelPixels`.
    fn compute_travel_pixels(&self, source: ZoneId, target: ZoneId) -> Vec<Rect<i32>> {
        let bounds: Vec<Rect<i32>> = self
            .main_zones
            .iter()
            .map(|&z| self.arena[z].pixels_bounds())
            .collect();
        super::travel::travel(
            self.arena[source].pixels_bounds(),
            self.arena[target].pixels_bounds(),
            &bounds,
        )
    }
}

fn parse_links(container: Option<Node>) -> Vec<ZoneLink> {
    let Some(container) = container else {
        return Vec::new();
    };
    container
        .children()
        .filter(|c| c.has_tag_name("ZoneLink"))
        .map(|zl| {
            // `BorderResistance` is the historical single value. A UI that
            // predates the move/drag split emits it alone, and omitting
            // `DragResistance` must then behave exactly as before — hence the
            // fallback rather than a 0.0 default.
            let move_resistance = get_f64(zl, "BorderResistance");
            let drag_resistance = get_f64_opt(zl, "DragResistance").unwrap_or(move_resistance);
            ZoneLink::new(
                get_f64(zl, "From"),
                get_f64(zl, "To"),
                get_i32(zl, "SourceFromPixel"),
                get_i32(zl, "SourceToPixel"),
                get_i32(zl, "TargetFromPixel"),
                get_i32(zl, "TargetToPixel"),
                block_or(get_bool(zl, "MoveBlock", false), move_resistance),
                block_or(get_bool(zl, "DragBlock", false), drag_resistance),
                get_i32(zl, "TargetId"),
            )
        })
        .collect()
}

/// A blocked border is an infinite resistance — see `zone_link`.
fn block_or(blocked: bool, resistance: f64) -> f64 {
    if blocked {
        f64::INFINITY
    } else {
        resistance
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::zone_link::{MODE_DRAG, MODE_MOVE};

    // Three zones: "Left", "Right", and a clone of "Left" (identical pixel
    // bounds). Exercises attribute parsing (bools, InvariantCulture doubles incl.
    // the ±E+308 / i32 sentinels), nested <Rect>, links, target resolution and
    // clone detection.
    const FIXTURE: &str = concat!(
        r#"<ZonesLayout AdjustPointer="False" AdjustSpeed="False" LoopX="False" LoopY="False" Priority="Normal" PriorityUnhooked="Below" Algorithm="Strait" MaxTravelDistance="200"><MainZones>"#,
        r#"<Zone Id="0" Name="Left"><PixelsBounds><Rect Left="-3840" Top="0" Width="3840" Height="2160"></Rect></PixelsBounds><PhysicalBounds><Rect Left="-567" Top="30.920075223319227" Width="527" Height="296"></Rect></PhysicalBounds>"#,
        r#"<RightLinks><ZoneLink From="0" To="393" SourceFromPixel="-225" SourceToPixel="2642" TargetFromPixel="0" TargetToPixel="2160" BorderResistance="0" TargetId="1"></ZoneLink>"#,
        r#"<ZoneLink From="393" To="1.7976931348623157E+308" SourceFromPixel="2642" SourceToPixel="2147483647" TargetFromPixel="-2147483648" TargetToPixel="2147483647" BorderResistance="0" TargetId="-1"></ZoneLink></RightLinks></Zone>"#,
        r#"<Zone Id="1" Name="Right"><PixelsBounds><Rect Left="0" Top="0" Width="3840" Height="2160"></Rect></PixelsBounds><PhysicalBounds><Rect Left="0" Top="0" Width="698" Height="393"></Rect></PhysicalBounds></Zone>"#,
        r#"<Zone Id="2" Name="CloneOfLeft"><PixelsBounds><Rect Left="-3840" Top="0" Width="3840" Height="2160"></Rect></PixelsBounds><PhysicalBounds><Rect Left="-567" Top="30.920075223319227" Width="527" Height="296"></Rect></PhysicalBounds></Zone>"#,
        r#"</MainZones></ZonesLayout>"#,
    );

    fn zone_by_name<'a>(layout: &'a ZonesLayout, name: &str) -> (ZoneId, &'a Zone) {
        let zid = *layout
            .all_zones
            .iter()
            .find(|&&z| layout.arena[z].name == name)
            .unwrap();
        (zid, &layout.arena[zid])
    }

    #[test]
    fn parses_attributes_and_zone_count() {
        let layout = ZonesLayout::from_xml(FIXTURE).expect("parse");
        assert_eq!(layout.zones.len(), 3);
        // Left and Right are main; the clone is not.
        assert_eq!(layout.main_zones.len(), 2);
        assert_eq!(layout.algorithm, Algorithm::Strait);
        assert_eq!(layout.priority, Priority::Normal);
        assert_eq!(layout.priority_unhooked, Priority::Below);
        assert_eq!(layout.max_travel_distance_squared, 40000.0);
        // Absent FreelookCheckInterval -> 0.
        assert_eq!(layout.freelook_check_interval_ms, 0);
        assert!(!layout.loop_x && !layout.loop_y);
    }

    #[test]
    fn detects_clone_and_resolves_links() {
        let layout = ZonesLayout::from_xml(FIXTURE).expect("parse");

        let (left_id, left) = zone_by_name(&layout, "Left");
        let (right_id, _) = zone_by_name(&layout, "Right");
        let (clone_id, clone) = zone_by_name(&layout, "CloneOfLeft");

        // The clone's Main points at the original Left zone; Left is its own Main.
        assert_eq!(left.main, left_id);
        assert_eq!(clone.main, left_id);
        assert_ne!(clone.main, clone_id);

        // Left's first RightLink targets zone id 1 (Right); the catch-all (-1)
        // resolves to None.
        assert_eq!(left.right[0].target, Some(right_id));
        assert_eq!(left.right[1].target, None);
        assert_eq!(left.right[1].to, f64::MAX);
        assert_eq!(left.right[1].source_to_px, i32::MAX);
        // ToTargetPixel maps the source span onto the target span.
        assert_eq!(left.right[0].to_target_pixel(-225), 0);
        assert_eq!(left.right[0].to_target_pixel(2642), 2160);
    }

    #[test]
    fn transforms_round_trip() {
        let layout = ZonesLayout::from_xml(FIXTURE).expect("parse");
        let (_, right) = zone_by_name(&layout, "Right"); // pixels (0,0,3840,2160), physical (0,0,698,393)

        // DPI ~ 139.7 for a 3840px / 698mm panel.
        assert!((139.0..140.0).contains(&right.dpi), "dpi was {}", right.dpi);

        assert!(right.contains_pixel(Point::new(100, 100)));
        assert!(!right.contains_pixel(Point::new(-1, 100)));
        assert!(!right.contains_pixel(Point::new(3840, 100))); // == right edge is outside

        // Pixel -> physical -> pixel round-trips back to the source pixel.
        let px = Point::new(1000, 500);
        let mm = right.to_physical(px);
        let back = right.to_pixels(mm);
        assert_eq!(back.x(), px.x());
        assert_eq!(back.y(), px.y());
    }

    #[test]
    fn absent_drag_attribute_falls_back_to_border_resistance() {
        // The wire-compatibility guarantee: a layout produced by a UI that
        // predates the move/drag split must behave exactly as it did, i.e. the
        // single resistance governs both modes.
        let xml = FIXTURE.replace(
            r#"BorderResistance="0" TargetId="1""#,
            r#"BorderResistance="42" TargetId="1""#,
        );
        let layout = ZonesLayout::from_xml(&xml).expect("parse");
        let (_, left) = zone_by_name(&layout, "Left");
        let link = &left.right[0];

        assert_eq!(link.border_resistance[MODE_MOVE], 42.0);
        assert_eq!(link.border_resistance[MODE_DRAG], 42.0);
        assert_eq!(
            link.border_resistance_px[MODE_MOVE],
            link.border_resistance_px[MODE_DRAG]
        );
        assert!(link.border_resistance_px[MODE_MOVE] > 0);
    }

    #[test]
    fn block_attributes_yield_an_undrainable_resistance() {
        let xml = FIXTURE.replace(
            r#"BorderResistance="0" TargetId="1""#,
            r#"BorderResistance="10" DragBlock="True" TargetId="1""#,
        );
        let layout = ZonesLayout::from_xml(&xml).expect("parse");
        let (_, left) = zone_by_name(&layout, "Left");
        let link = &left.right[0];

        assert!(link.border_resistance[MODE_MOVE].is_finite());
        assert_eq!(link.border_resistance[MODE_DRAG], f64::INFINITY);
        assert_eq!(link.border_resistance_px[MODE_DRAG], i64::MAX);

        // The catch-all link spans f64::MIN..f64::MAX, so its `to - from` is
        // already infinite: blocking it must not fall through the ratio and
        // land on NaN (which casts to 0 — a silently unblocked border).
        let xml = FIXTURE.replace(
            r#"BorderResistance="0" TargetId="-1""#,
            r#"BorderResistance="0" MoveBlock="True" TargetId="-1""#,
        );
        let layout = ZonesLayout::from_xml(&xml).expect("parse");
        let (_, left) = zone_by_name(&layout, "Left");
        assert_eq!(left.right[1].border_resistance_px[MODE_MOVE], i64::MAX);
    }

    #[test]
    fn containing_selects_main_zone() {
        let layout = ZonesLayout::from_xml(FIXTURE).expect("parse");
        let (left_id, _) = zone_by_name(&layout, "Left");
        let (right_id, _) = zone_by_name(&layout, "Right");

        // A pixel inside the Left panel resolves to Left (not its clone).
        assert_eq!(
            layout.containing_pixel(Point::new(-100, 100)),
            Some(left_id)
        );
        assert_eq!(
            layout.containing_pixel(Point::new(100, 100)),
            Some(right_id)
        );
        assert_eq!(layout.containing_pixel(Point::new(99999, 99999)), None);
    }

    #[test]
    fn a_zone_without_a_host_is_this_machines_and_its_points_are_its_own() {
        let layout = ZonesLayout::from_xml(FIXTURE).expect("parse");
        let (_, right) = zone_by_name(&layout, "Right");
        assert_eq!(right.host, None);
        assert_eq!(right.host_offset, Point::new(0, 0));
        let point = layout.host_point(Point::new(100, 100)).unwrap();
        assert_eq!(point.host, None);
        assert_eq!(point.pixel, Point::new(100, 100));
    }

    #[test]
    fn a_remote_zones_point_is_given_back_in_its_machines_desktop() {
        let xml = concat!(
            r#"<ZonesLayout><MainZones>"#,
            r#"<Zone Id="0" Name="Here"><PixelsBounds><Rect Left="0" Top="0" Width="1920" Height="1080"></Rect></PixelsBounds><PhysicalBounds><Rect Left="0" Top="0" Width="527" Height="296"></Rect></PhysicalBounds></Zone>"#,
            r#"<Zone Id="1" Name="There" Host="abcd" HostX="4920" HostY="-20"><PixelsBounds><Rect Left="3000" Top="-20" Width="1920" Height="1080"></Rect></PixelsBounds><PhysicalBounds><Rect Left="527" Top="0" Width="527" Height="296"></Rect></PhysicalBounds></Zone>"#,
            r#"</MainZones></ZonesLayout>"#,
        );
        let mut layout = ZonesLayout::from_xml(xml).expect("parse");
        layout.set_remote_enterable(true);
        let (_, there) = zone_by_name(&layout, "There");
        assert_eq!(there.host.as_deref(), Some("abcd"));
        assert_eq!(there.host_offset, Point::new(4920, -20));

        let point = layout.host_point(Point::new(3010, 0)).unwrap();
        assert_eq!(point.host, Some("abcd"));
        assert_eq!(point.pixel, Point::new(-1910, 20));
        assert_eq!(layout.host_point(Point::new(2500, 0)), None);
    }

    // Here (0..1920) links right to There (a remote at 3000..4920), which links back.
    const TWO_MACHINES: &str = concat!(
        r#"<ZonesLayout LoopX="True"><MainZones>"#,
        r#"<Zone Id="0" Name="Here"><PixelsBounds><Rect Left="0" Top="0" Width="1920" Height="1080"></Rect></PixelsBounds><PhysicalBounds><Rect Left="0" Top="0" Width="527" Height="296"></Rect></PhysicalBounds>"#,
        r#"<RightLinks><ZoneLink From="0" To="296" SourceFromPixel="0" SourceToPixel="1080" TargetFromPixel="0" TargetToPixel="1080" BorderResistance="0" TargetId="1"></ZoneLink></RightLinks></Zone>"#,
        r#"<Zone Id="1" Name="There" Host="abcd" HostX="3000" HostY="0"><PixelsBounds><Rect Left="3000" Top="0" Width="1920" Height="1080"></Rect></PixelsBounds><PhysicalBounds><Rect Left="527" Top="0" Width="527" Height="296"></Rect></PhysicalBounds>"#,
        r#"<LeftLinks><ZoneLink From="0" To="296" SourceFromPixel="0" SourceToPixel="1080" TargetFromPixel="0" TargetToPixel="1080" BorderResistance="0" TargetId="0"></ZoneLink></LeftLinks></Zone>"#,
        r#"</MainZones></ZonesLayout>"#,
    );

    #[test]
    fn another_machines_zones_are_out_of_play_until_they_are_put_in() {
        let layout = ZonesLayout::from_xml(TWO_MACHINES).expect("parse");
        let (here, _) = zone_by_name(&layout, "Here");

        // As if the document had described this machine's screen alone: no zone, no
        // link target, no extent and no point of the other machine.
        assert_eq!(layout.all_zones.len(), 2);
        assert_eq!(layout.zones, vec![here]);
        assert_eq!(layout.main_zones, vec![here]);
        assert_eq!(layout.arena[here].right[0].target, None);
        assert_eq!(layout.width(), 40.0 + 527.0);
        assert_eq!(layout.host_point(Point::new(3010, 0)), None);
        assert!(layout.has_remote_zones());
        assert!(!layout.remote_enterable());
    }

    // Here (0..1920) links right to A (3000..4920), which links right to B (6000..7920).
    const THREE_MACHINES: &str = concat!(
        r#"<ZonesLayout><MainZones>"#,
        r#"<Zone Id="0" Name="Here"><PixelsBounds><Rect Left="0" Top="0" Width="1920" Height="1080"></Rect></PixelsBounds><PhysicalBounds><Rect Left="0" Top="0" Width="527" Height="296"></Rect></PhysicalBounds>"#,
        r#"<RightLinks><ZoneLink From="0" To="296" SourceFromPixel="0" SourceToPixel="1080" TargetFromPixel="0" TargetToPixel="1080" BorderResistance="0" TargetId="1"></ZoneLink></RightLinks></Zone>"#,
        r#"<Zone Id="1" Name="A" Host="aaaa" HostX="3000" HostY="0"><PixelsBounds><Rect Left="3000" Top="0" Width="1920" Height="1080"></Rect></PixelsBounds><PhysicalBounds><Rect Left="527" Top="0" Width="527" Height="296"></Rect></PhysicalBounds>"#,
        r#"<RightLinks><ZoneLink From="0" To="296" SourceFromPixel="0" SourceToPixel="1080" TargetFromPixel="0" TargetToPixel="1080" BorderResistance="0" TargetId="2"></ZoneLink></RightLinks></Zone>"#,
        r#"<Zone Id="2" Name="B" Host="bbbb" HostX="6000" HostY="0"><PixelsBounds><Rect Left="6000" Top="0" Width="1920" Height="1080"></Rect></PixelsBounds><PhysicalBounds><Rect Left="1054" Top="0" Width="527" Height="296"></Rect></PhysicalBounds></Zone>"#,
        r#"</MainZones></ZonesLayout>"#,
    );

    #[test]
    fn only_the_drivable_machines_zones_are_in_play_once_the_agent_names_them() {
        let mut layout = ZonesLayout::from_xml(THREE_MACHINES).expect("parse");
        let (here, _) = zone_by_name(&layout, "Here");
        let (a, _) = zone_by_name(&layout, "A");
        let (b, _) = zone_by_name(&layout, "B");
        layout.set_remote_enterable(true);
        // Named by nobody: every machine, as before.
        assert_eq!(layout.main_zones, vec![here, a, b]);

        let only_a = ["aaaa".to_owned()];
        layout.set_drivable_hosts(Some(&only_a));
        assert_eq!(layout.main_zones, vec![here, a]);
        // The same list again, as at every report: kept as it is, nothing copied.
        let kept = layout.drivable.as_ref().unwrap().as_ptr();
        layout.set_drivable_hosts(Some(&only_a));
        assert_eq!(layout.drivable.as_ref().unwrap().as_ptr(), kept);
        assert_eq!(layout.arena[a].right[0].target, None, "B is a wall");
        assert_eq!(layout.host_point(Point::new(6010, 0)), None);

        layout.set_drivable_hosts(Some(&["aaaa".to_owned(), "bbbb".to_owned()]));
        assert_eq!(layout.main_zones, vec![here, a, b]);
        assert_eq!(layout.arena[a].right[0].target, Some(b));

        layout.set_drivable_hosts(Some(&[]));
        assert_eq!(layout.main_zones, vec![here]);
        // Without a client, none, whatever the agent named.
        layout.set_drivable_hosts(None);
        layout.set_remote_enterable(false);
        assert_eq!(layout.main_zones, vec![here]);
    }

    #[test]
    fn putting_another_machines_zones_in_play_links_them_and_taking_them_out_unlinks_them() {
        let mut layout = ZonesLayout::from_xml(TWO_MACHINES).expect("parse");
        let (here, _) = zone_by_name(&layout, "Here");
        let (there, _) = zone_by_name(&layout, "There");

        layout.set_remote_enterable(true);
        assert_eq!(layout.main_zones, vec![here, there]);
        assert_eq!(layout.arena[here].right[0].target, Some(there));
        assert_eq!(layout.width(), 40.0 + 1054.0);
        assert_eq!(
            layout.host_point(Point::new(3010, 5)),
            Some(HostPoint {
                host: Some("abcd"),
                pixel: Point::new(10, 5)
            })
        );
        // A travel computed while they were in play must not outlive them.
        assert!(!layout.travel_pixels(here, there).is_empty());

        layout.set_remote_enterable(false);
        assert_eq!(layout.main_zones, vec![here]);
        assert_eq!(layout.arena[here].right[0].target, None);
        assert!(layout.travels.is_empty());
    }
}
