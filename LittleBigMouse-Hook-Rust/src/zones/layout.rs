//! Port of `Engine/ZonesLayout` — owns the zone arena and parses the XML layout.

use std::collections::HashMap;

use roxmltree::{Document, Node};
use slotmap::SlotMap;

use super::xml::{child, get_bool, get_f64, get_i32, get_rect_f64, get_rect_i32, get_string};
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
    pub zones: Vec<ZoneId>,
    pub main_zones: Vec<ZoneId>,

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
    pub loop_x: bool,
    pub loop_y: bool,

    left: f64,
    top: f64,
    right: f64,
    bottom: f64,

    /// Travel-rect cache, keyed by `(source, target.main)`. Lifted out of `Zone`
    /// (unlike the C++ `Zone::_travels`) so `Move` can mutate it without
    /// aliasing the arena.
    travels: HashMap<(ZoneId, ZoneId), Vec<Rect<i32>>>,
}

impl Default for ZonesLayout {
    fn default() -> Self {
        ZonesLayout {
            arena: SlotMap::with_key(),
            zones: Vec::new(),
            main_zones: Vec::new(),
            // C++ ZonesLayout member initializers.
            max_travel_distance_squared: 200.0 * 200.0,
            freelook_check_interval_ms: 100,
            freelook_enabled: true,
            adjust_pointer: false,
            adjust_speed: false,
            algorithm: Algorithm::Strait,
            priority: Priority::Normal,
            priority_unhooked: Priority::Above,
            loop_x: false,
            loop_y: false,
            left: 0.0,
            top: 0.0,
            right: 0.0,
            bottom: 0.0,
            travels: HashMap::new(),
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
        layout.algorithm = if get_string(el, "Algorithm") == "Cross" {
            Algorithm::CornerCrossing
        } else {
            Algorithm::Strait
        };
        layout.priority = Priority::parse(&get_string(el, "Priority"));
        layout.priority_unhooked = Priority::parse(&get_string(el, "PriorityUnhooked"));

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
                by_bounds.push((pixels_bounds, key));
                layout.zones.push(key);
            }
        }

        layout.init();
        layout
    }

    /// C++ `ZonesLayout::Init`: compute overall bounds, collect main zones, and
    /// resolve each link's target from its `target_id`.
    fn init(&mut self) {
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
            ZoneLink::new(
                get_f64(zl, "From"),
                get_f64(zl, "To"),
                get_i32(zl, "SourceFromPixel"),
                get_i32(zl, "SourceToPixel"),
                get_i32(zl, "TargetFromPixel"),
                get_i32(zl, "TargetToPixel"),
                get_f64(zl, "BorderResistance"),
                get_i32(zl, "TargetId"),
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

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
            .zones
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
}
