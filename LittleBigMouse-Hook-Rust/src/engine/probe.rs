//! Edge prober — drives the REAL engine over every edge of every main zone and
//! reports, per edge, which pixel ranges cross (and into which zone) and which
//! hit a wall. This is the honest version of a static link dump: it exercises
//! the algorithm actually configured (Strait links, Cross trips, loop shifts),
//! so what it reports is what the cursor would do.
//!
//! Resistance is deliberately bypassed (the probe cursor reports Ctrl held):
//! the report answers "CAN this edge be crossed", not "how hard is it".
//!
//! The probe runs on its own layout instance parsed from the serialized XML, so
//! it never touches the live engine or its tracking state.

use super::cursor::CursorEnv;
use super::event::MouseEventArg;
use super::MouseEngine;
use crate::geometry::{Point, Rect};
use crate::zones::ZonesLayout;

struct ProbeCursor {
    pos: Point<i32>,
    clip: Rect<i32>,
    desktop: Rect<i32>,
}

impl CursorEnv for ProbeCursor {
    fn get_mouse_location(&self) -> Point<i32> {
        Point::new(
            self.pos.x().clamp(self.clip.left(), self.clip.right() - 1),
            self.pos.y().clamp(self.clip.top(), self.clip.bottom() - 1),
        )
    }
    fn set_mouse_location(&mut self, location: Point<i32>) {
        self.pos = location;
    }
    fn get_clip(&self) -> Rect<i32> {
        self.clip
    }
    fn set_clip(&mut self, r: Rect<i32>) {
        self.clip = if r.is_empty() { self.desktop } else { r };
    }
    fn ctrl_down(&self) -> bool {
        true // bypass border resistance: crossability only
    }
    fn buttons_down(&self) -> bool {
        false // moot while ctrl_down() bypasses resistance entirely
    }
    fn cursor_hidden(&self) -> bool {
        false
    }
    fn clip_is_subrect_of_virtual_screen(&self) -> bool {
        false
    }
    fn tick_count(&self) -> u64 {
        0
    }
}

/// One homogeneous stretch of an edge: pixels `from..=to` (desktop coordinates
/// along the edge: y for Left/Right, x for Top/Bottom) all land in zone
/// `target` (the zone's serialized Id), or hit a wall (`target == -1`).
pub struct EdgeRun {
    pub from: i32,
    pub to: i32,
    pub target: i32,
}

pub struct EdgeReport {
    pub side: &'static str,
    pub runs: Vec<EdgeRun>,
}

pub struct ZoneProbe {
    pub id: i32,
    pub name: String,
    /// The zone's display-source id — what the UI uses to map a probed zone
    /// back onto a monitor (zone indices are serialization order, fragile).
    pub device_id: String,
    pub edges: Vec<EdgeReport>,
}

/// Probe a serialized `<ZonesLayout>` and return the report as XML, or `None`
/// if the XML does not parse (the caller reported LoadFailed already).
pub fn probe_xml(xml: &str) -> Option<String> {
    let layout = ZonesLayout::from_xml(xml)?;
    let meta = Meta {
        algorithm: layout.algorithm,
        loop_x: layout.loop_x,
        loop_y: layout.loop_y,
        virtual_layout: layout.virtual_layout,
    };
    let zones = probe(layout);
    Some(to_xml(&meta, &zones))
}

struct Meta {
    algorithm: crate::zones::Algorithm,
    loop_x: bool,
    loop_y: bool,
    virtual_layout: bool,
}

/// Sweep every edge of every main zone of `layout`.
pub fn probe(layout: ZonesLayout) -> Vec<ZoneProbe> {
    // Desktop = union of the main zones' pixel bounds (the probe cursor's
    // fallback clip; the engine confines further as it sees fit).
    let mut desktop: Option<Rect<i32>> = None;
    let main: Vec<_> = layout.main_zones.to_vec();
    for &zid in &main {
        let b = layout.arena[zid].pixels_bounds();
        desktop = Some(match desktop {
            None => b,
            Some(d) => union(d, b),
        });
    }
    let desktop = match desktop {
        Some(d) => d,
        None => return Vec::new(),
    };

    let mut engine = MouseEngine::new();
    engine.load(layout);
    let mut env = ProbeCursor {
        pos: Point::new(0, 0),
        clip: desktop,
        desktop,
    };

    let mut zones = Vec::new();
    for &zid in &main {
        let bounds = engine.layout.arena[zid].pixels_bounds();
        let (id, name, device_id) = {
            let z = &engine.layout.arena[zid];
            (z.id, z.name.clone(), z.device_id.clone())
        };

        let edges = [
            // side, coordinate range along the edge, inside point, outside point
            (
                "Left",
                bounds.top(),
                bounds.bottom(),
                EdgeGeometry::Vertical {
                    inside_x: bounds.left() + 1,
                    outside_x: bounds.left() - 1,
                },
            ),
            (
                "Right",
                bounds.top(),
                bounds.bottom(),
                EdgeGeometry::Vertical {
                    inside_x: bounds.right() - 2,
                    outside_x: bounds.right(),
                },
            ),
            (
                "Top",
                bounds.left(),
                bounds.right(),
                EdgeGeometry::Horizontal {
                    inside_y: bounds.top() + 1,
                    outside_y: bounds.top() - 1,
                },
            ),
            (
                "Bottom",
                bounds.left(),
                bounds.right(),
                EdgeGeometry::Horizontal {
                    inside_y: bounds.bottom() - 2,
                    outside_y: bounds.bottom(),
                },
            ),
        ];

        let mut reports = Vec::new();
        for (side, from, to, geometry) in edges {
            let mut runs: Vec<EdgeRun> = Vec::new();
            for c in from..to {
                let (inside, outside) = geometry.points(c);
                let target = probe_point(&mut engine, &mut env, desktop, inside, outside);
                match runs.last_mut() {
                    Some(run) if run.target == target && run.to + 1 == c => run.to = c,
                    _ => runs.push(EdgeRun {
                        from: c,
                        to: c,
                        target,
                    }),
                }
            }
            reports.push(EdgeReport { side, runs });
        }
        zones.push(ZoneProbe {
            id,
            name,
            device_id,
            edges: reports,
        });
    }
    zones
}

enum EdgeGeometry {
    Vertical { inside_x: i32, outside_x: i32 },
    Horizontal { inside_y: i32, outside_y: i32 },
}

impl EdgeGeometry {
    fn points(&self, c: i32) -> (Point<i32>, Point<i32>) {
        match *self {
            EdgeGeometry::Vertical {
                inside_x,
                outside_x,
            } => (Point::new(inside_x, c), Point::new(outside_x, c)),
            EdgeGeometry::Horizontal {
                inside_y,
                outside_y,
            } => (Point::new(c, inside_y), Point::new(c, outside_y)),
        }
    }
}

/// One probe: arm tracking on `inside`, push to `outside`, report the landing
/// zone's Id, or -1 when the engine kept the cursor confined.
fn probe_point(
    engine: &mut MouseEngine,
    env: &mut ProbeCursor,
    desktop: Rect<i32>,
    inside: Point<i32>,
    outside: Point<i32>,
) -> i32 {
    engine.reset();
    env.clip = desktop;
    env.pos = inside;

    let mut arm = MouseEventArg::new(inside);
    engine.on_mouse_move(env, &mut arm);

    let mut push = MouseEventArg::new(outside);
    engine.on_mouse_move(env, &mut push);
    if !push.handled {
        return -1;
    }
    engine
        .layout
        .containing_pixel(env.get_mouse_location())
        .map(|zid| engine.layout.arena[zid].id)
        .unwrap_or(-1)
}

fn union(a: Rect<i32>, b: Rect<i32>) -> Rect<i32> {
    let left = a.left().min(b.left());
    let top = a.top().min(b.top());
    let right = a.right().max(b.right());
    let bottom = a.bottom().max(b.bottom());
    Rect::new(left, top, right - left, bottom - top)
}

/// Serialize the report. Runs ride the daemon event payload, so this stays a
/// compact, standalone XML document.
fn to_xml(meta: &Meta, zones: &[ZoneProbe]) -> String {
    let mut xml = format!(
        r#"<ProbeReport Algorithm="{}" LoopX="{}" LoopY="{}" Virtual="{}">"#,
        match meta.algorithm {
            crate::zones::Algorithm::Strait => "Strait",
            crate::zones::Algorithm::CornerCrossing => "Cross",
        },
        xml_bool(meta.loop_x),
        xml_bool(meta.loop_y),
        xml_bool(meta.virtual_layout),
    );
    for zone in zones {
        xml.push_str(&format!(
            r#"<Zone Id="{}" Name="{}" DeviceId="{}">"#,
            zone.id,
            escape(&zone.name),
            escape(&zone.device_id)
        ));
        for edge in &zone.edges {
            xml.push_str(&format!(r#"<Edge Side="{}">"#, edge.side));
            for run in &edge.runs {
                xml.push_str(&format!(
                    r#"<Run From="{}" To="{}" Target="{}"/>"#,
                    run.from, run.to, run.target
                ));
            }
            xml.push_str("</Edge>");
        }
        xml.push_str("</Zone>");
    }
    xml.push_str("</ProbeReport>");
    xml
}

fn xml_bool(value: bool) -> &'static str {
    if value {
        "True"
    } else {
        "False"
    }
}

fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;

    // Two 1920x1080 zones side by side, physically adjacent: Left's right edge
    // links to Right and vice versa; every outer edge is a wall.
    const TWO_ZONES: &str = concat!(
        r#"<ZonesLayout Algorithm="Strait" MaxTravelDistance="200"><MainZones>"#,
        r#"<Zone Id="0" Name="Left"><PixelsBounds><Rect Left="0" Top="0" Width="1920" Height="1080"></Rect></PixelsBounds><PhysicalBounds><Rect Left="0" Top="0" Width="500" Height="280"></Rect></PhysicalBounds>"#,
        r#"<RightLinks><ZoneLink From="0" To="280" SourceFromPixel="0" SourceToPixel="1080" TargetFromPixel="0" TargetToPixel="1080" BorderResistance="0" TargetId="1"></ZoneLink></RightLinks></Zone>"#,
        r#"<Zone Id="1" Name="Right"><PixelsBounds><Rect Left="1920" Top="0" Width="1920" Height="1080"></Rect></PixelsBounds><PhysicalBounds><Rect Left="500" Top="0" Width="500" Height="280"></Rect></PhysicalBounds>"#,
        r#"<LeftLinks><ZoneLink From="0" To="280" SourceFromPixel="0" SourceToPixel="1080" TargetFromPixel="0" TargetToPixel="1080" BorderResistance="0" TargetId="0"></ZoneLink></LeftLinks></Zone>"#,
        r#"</MainZones></ZonesLayout>"#,
    );

    fn edge<'a>(zones: &'a [ZoneProbe], zone: &str, side: &str) -> &'a EdgeReport {
        zones
            .iter()
            .find(|z| z.name == zone)
            .unwrap()
            .edges
            .iter()
            .find(|e| e.side == side)
            .unwrap()
    }

    #[test]
    fn adjacent_edges_cross_and_outer_edges_wall() {
        let zones = probe(ZonesLayout::from_xml(TWO_ZONES).unwrap());

        // The shared border crosses over its full height, in both directions.
        let right = edge(&zones, "Left", "Right");
        assert_eq!(right.runs.len(), 1, "one homogeneous run expected");
        assert_eq!(
            (right.runs[0].from, right.runs[0].to, right.runs[0].target),
            (0, 1079, 1)
        );

        let left = edge(&zones, "Right", "Left");
        assert_eq!(left.runs.len(), 1);
        assert_eq!(left.runs[0].target, 0);

        // Outer edges are walls end to end.
        for (zone, side) in [
            ("Left", "Left"),
            ("Left", "Top"),
            ("Left", "Bottom"),
            ("Right", "Right"),
            ("Right", "Top"),
            ("Right", "Bottom"),
        ] {
            let e = edge(&zones, zone, side);
            assert_eq!(e.runs.len(), 1, "{zone}/{side}");
            assert_eq!(e.runs[0].target, -1, "{zone}/{side} must be a wall");
        }
    }

    #[test]
    fn loop_links_are_reported_as_crossings() {
        // The real 3x4K loop fixture (generated by the C# ComputeZones with
        // LoopX/LoopY): every outer edge must cross somewhere, no wall runs.
        let xml = include_str!("../../tests/scratch-loop-zones.xml");
        let zones = probe(ZonesLayout::from_xml(xml).unwrap());
        assert_eq!(zones.len(), 3);
        for zone in &zones {
            for e in &zone.edges {
                assert!(
                    e.runs.iter().all(|r| r.target != -1),
                    "zone {} {} has a wall run with loop enabled",
                    zone.name,
                    e.side
                );
            }
        }
        // And the horizontal loop lands on the OPPOSITE monitor.
        let z0 = zones.iter().find(|z| z.id == 0).unwrap();
        let left = z0.edges.iter().find(|e| e.side == "Left").unwrap();
        assert!(
            left.runs.iter().all(|r| r.target == 2),
            "left loop must land on zone 2"
        );
    }

    #[test]
    fn report_serializes_as_wellformed_xml() {
        let report = probe_xml(TWO_ZONES).expect("probe");
        let doc = roxmltree::Document::parse(&report).expect("well-formed");
        let root = doc.root_element();
        assert_eq!(root.tag_name().name(), "ProbeReport");
        assert_eq!(root.attribute("Algorithm"), Some("Strait"));
        // 2 zones, 4 edges each.
        assert_eq!(
            root.children().filter(|c| c.has_tag_name("Zone")).count(),
            2
        );
    }
}
