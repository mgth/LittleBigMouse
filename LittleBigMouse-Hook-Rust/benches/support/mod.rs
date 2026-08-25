//! Shared fixtures for the engine benchmarks — synthetic layouts and a fake
//! cursor. Nothing here touches the OS: no hook, no `SetCursorPos`, no device
//! enumeration. The benchmarks measure the pure traversal core, which is exactly
//! the part `CursorEnv` was introduced to isolate.
//!
//! ## Why the layouts are generated rather than hand-written
//!
//! Zone layouts only enter the daemon as XML from the C# UI, and
//! [`ZonesLayout::from_xml`] is their only constructor. Generating that XML is
//! therefore the only way to build a 16-monitor fixture without checking in a
//! 25 kB file. [`layout_xml`] reproduces the shape the real serializer emits
//! (`tests/border-sections-zones.xml` is a captured sample of it):
//!
//! * every side carries a leading catch-all (`From = -f64::MAX`), then the real
//!   links sorted along the edge, then a trailing catch-all (`To = f64::MAX`),
//!   so the `at_physical_index` / `at_pixel_index` walk has the same length it
//!   has in production;
//! * `From`/`To` are the *target's* physical span along the edge, in absolute
//!   millimetres;
//! * `SourceFromPixel`/`SourceToPixel` are the source pixels that map onto that
//!   span, `TargetFromPixel`/`TargetToPixel` the target's own pixel span.
//!
//! Feeding the mixed-DPI numbers of the checked-in fixtures through
//! [`link_attrs`] reproduces their attributes exactly, which is what makes these
//! generated layouts credible stand-ins for real ones.

// Each bench target uses a subset of this module; both would otherwise report
// the other's helpers as dead code.
#![allow(dead_code)]

use littlebigmouse_hook::engine::cursor::CursorEnv;
use littlebigmouse_hook::engine::event::MouseEventArg;
use littlebigmouse_hook::engine::MouseEngine;
use littlebigmouse_hook::geometry::{Point, Rect};
use littlebigmouse_hook::zones::{ZoneId, ZonesLayout};

/// `f64::MAX` as the C# `InvariantCulture` round-trip format spells it.
const F64_MAX: &str = "1.7976931348623157E+308";
const F64_MIN: &str = "-1.7976931348623157E+308";

// --- fake cursor ------------------------------------------------------------

/// The evdev backend's `EvdevCursor` semantics, minus the device: a virtual
/// position clamped into the active clip. Every method is a field read, so the
/// measurement is engine time and not environment time.
///
/// `cursor_hidden` and `clip_is_subrect_of_virtual_screen` answer "no" so the
/// freelook gate stays open — a benchmark that silently fell into freelook would
/// be measuring an early return.
pub struct BenchCursor {
    pos: Point<i32>,
    clip: Rect<i32>,
    desktop: Rect<i32>,
}

impl BenchCursor {
    pub fn new(desktop: Rect<i32>) -> Self {
        BenchCursor {
            pos: Point::new(desktop.left(), desktop.top()),
            clip: desktop,
            desktop,
        }
    }

    fn clamp(&self, p: Point<i32>) -> Point<i32> {
        Point::new(
            p.x().clamp(self.clip.left(), self.clip.right() - 1),
            p.y().clamp(self.clip.top(), self.clip.bottom() - 1),
        )
    }
}

impl CursorEnv for BenchCursor {
    fn get_mouse_location(&self) -> Point<i32> {
        self.clamp(self.pos)
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
        false
    }
    fn buttons_down(&self) -> bool {
        false
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

// --- panels -----------------------------------------------------------------

/// One monitor: its pixel rect on the virtual desktop and its physical rect in
/// millimetres. The ratio between the two is the DPI the engine has to correct.
#[derive(Debug, Clone, Copy)]
pub struct Panel {
    pub pixels: Rect<i32>,
    pub physical: Rect<f64>,
}

impl Panel {
    /// The source pixel row/column that maps onto absolute physical `mm` along
    /// the given axis — the serializer's `SourceFromPixel`/`SourceToPixel`.
    fn pixel_at_mm(&self, mm: f64, vertical: bool) -> i32 {
        let (p0, plen, m0, mlen) = if vertical {
            (
                self.pixels.top(),
                self.pixels.height(),
                self.physical.top(),
                self.physical.height(),
            )
        } else {
            (
                self.pixels.left(),
                self.pixels.width(),
                self.physical.left(),
                self.physical.width(),
            )
        };
        p0 + ((mm - m0) * plen as f64 / mlen) as i32
    }
}

/// `n` panels side by side, physically centred on a common midline, with
/// alternating panel sizes so consecutive borders have mismatched DPI — the
/// case the whole engine exists for. Pixel geometry stays uniform (3840x2160)
/// because the OS reports a gapless pixel desktop.
pub fn row_panels(n: usize) -> Vec<Panel> {
    const PIXEL_W: i32 = 3840;
    const PIXEL_H: i32 = 2160;
    // A 32" and a 24" 4K panel: 139 vs 185 dpi.
    const SIZES: [(f64, f64); 2] = [(698.0, 393.0), (527.0, 296.0)];
    let tallest = SIZES[0].1;

    let mut panels = Vec::with_capacity(n);
    let mut mm_x = 0.0;
    for i in 0..n {
        let (w, h) = SIZES[i % SIZES.len()];
        panels.push(Panel {
            pixels: Rect::new(i as i32 * PIXEL_W, 0, PIXEL_W, PIXEL_H),
            physical: Rect::new(mm_x, (tallest - h) / 2.0, w, h),
        });
        mm_x += w;
    }
    panels
}

/// A `cols` x `rows` wall of identical 1920x1080 / 480x270 mm panels. Uniform
/// DPI on purpose: what this fixture stresses is the number of zone rectangles
/// a single trip line has to be intersected against, not the remapping.
pub fn grid_panels(cols: usize, rows: usize) -> Vec<Panel> {
    const PIXEL_W: i32 = 1920;
    const PIXEL_H: i32 = 1080;
    const MM_W: f64 = 480.0;
    const MM_H: f64 = 270.0;

    let mut panels = Vec::with_capacity(cols * rows);
    for r in 0..rows {
        for c in 0..cols {
            panels.push(Panel {
                pixels: Rect::new(c as i32 * PIXEL_W, r as i32 * PIXEL_H, PIXEL_W, PIXEL_H),
                physical: Rect::new(c as f64 * MM_W, r as f64 * MM_H, MM_W, MM_H),
            });
        }
    }
    panels
}

/// The union of every panel's pixel rect — what the backends call the desktop.
pub fn desktop_bounds(panels: &[Panel]) -> Rect<i32> {
    let l = panels.iter().map(|p| p.pixels.left()).min().unwrap_or(0);
    let t = panels.iter().map(|p| p.pixels.top()).min().unwrap_or(0);
    let r = panels.iter().map(|p| p.pixels.right()).max().unwrap_or(0);
    let b = panels.iter().map(|p| p.pixels.bottom()).max().unwrap_or(0);
    Rect::new(l, t, r - l, b - t)
}

// --- XML generation ---------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Algo {
    Strait,
    Cross,
}

impl Algo {
    fn as_attr(self) -> &'static str {
        match self {
            // The historical spelling, kept because it is part of the wire format.
            Algo::Strait => "Strait",
            Algo::Cross => "Cross",
        }
    }
}

/// Which edge of the source panel a link sits on.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Edge {
    Left,
    Top,
    Right,
    Bottom,
}

impl Edge {
    fn element(self) -> &'static str {
        match self {
            Edge::Left => "LeftLinks",
            Edge::Top => "TopLinks",
            Edge::Right => "RightLinks",
            Edge::Bottom => "BottomLinks",
        }
    }

    /// Left/Right edges are walked along y, Top/Bottom along x.
    fn vertical(self) -> bool {
        matches!(self, Edge::Left | Edge::Right)
    }

    /// Whether `b` sits immediately on this edge of `a`, sharing a run of pixels.
    fn adjacent(self, a: &Panel, b: &Panel) -> bool {
        let (ap, bp) = (a.pixels, b.pixels);
        let overlap_y = ap.top().max(bp.top()) < ap.bottom().min(bp.bottom());
        let overlap_x = ap.left().max(bp.left()) < ap.right().min(bp.right());
        match self {
            Edge::Left => ap.left() == bp.right() && overlap_y,
            Edge::Right => ap.right() == bp.left() && overlap_y,
            Edge::Top => ap.top() == bp.bottom() && overlap_x,
            Edge::Bottom => ap.bottom() == bp.top() && overlap_x,
        }
    }
}

/// One `<ZoneLink>`. Named fields rather than eight positional arguments: the
/// source and target pixel pairs are interchangeable to the compiler, and
/// swapping them would produce a layout that parses and crosses to the wrong
/// place.
struct Link<'a> {
    /// Physical span along the edge, in absolute millimetres.
    span_mm: (&'a str, &'a str),
    /// The source zone's pixels covering that span.
    source_px: (i32, i32),
    /// The target zone's pixels the span maps onto.
    target_px: (i32, i32),
    resistance: f64,
    /// Zone `Id` of the target, or `-1` for a catch-all with nothing behind it.
    target_id: i32,
}

impl Link<'_> {
    /// Serialize in the attribute order the C# serializer uses.
    fn to_xml(&self) -> String {
        let (from, to) = self.span_mm;
        let (source_from, source_to) = self.source_px;
        let (target_from, target_to) = self.target_px;
        let resistance = self.resistance;
        let target_id = self.target_id;
        format!(
            r#"<ZoneLink From="{from}" To="{to}" SourceFromPixel="{source_from}" SourceToPixel="{source_to}" TargetFromPixel="{target_from}" TargetToPixel="{target_to}" BorderResistance="{resistance}" MoveBlock="False" DragResistance="{resistance}" DragBlock="False" TargetId="{target_id}"></ZoneLink>"#
        )
    }
}

/// The `<XxxLinks>` element for one edge of one panel: leading catch-all, the
/// real neighbours sorted along the edge, trailing catch-all.
fn edge_links(panels: &[Panel], index: usize, edge: Edge, resistance: f64) -> String {
    let a = &panels[index];

    let mut neighbours: Vec<usize> = (0..panels.len())
        .filter(|&j| j != index && edge.adjacent(a, &panels[j]))
        .collect();
    let span = |p: &Panel| {
        if edge.vertical() {
            (p.physical.top(), p.physical.bottom())
        } else {
            (p.physical.left(), p.physical.right())
        }
    };
    neighbours.sort_by(|&x, &y| span(&panels[x]).0.total_cmp(&span(&panels[y]).0));

    let target_pixel_span = |p: &Panel| {
        if edge.vertical() {
            (p.pixels.top(), p.pixels.bottom())
        } else {
            (p.pixels.left(), p.pixels.right())
        }
    };

    let mut out = String::new();
    let mut cursor_mm = F64_MIN.to_string();
    let mut cursor_px = i32::MIN;

    for j in neighbours {
        let b = &panels[j];
        let (mm_from, mm_to) = span(b);
        let (tgt_from, tgt_to) = target_pixel_span(b);
        let src_from = a.pixel_at_mm(mm_from, edge.vertical());
        let src_to = a.pixel_at_mm(mm_to, edge.vertical());

        // Catch-all covering the run of edge before this neighbour starts.
        out.push_str(
            &Link {
                span_mm: (&cursor_mm, &fmt_mm(mm_from)),
                source_px: (cursor_px, src_from),
                target_px: (i32::MIN, src_from),
                resistance: 0.0,
                target_id: -1,
            }
            .to_xml(),
        );
        out.push_str(
            &Link {
                span_mm: (&fmt_mm(mm_from), &fmt_mm(mm_to)),
                source_px: (src_from, src_to),
                target_px: (tgt_from, tgt_to),
                resistance,
                target_id: j as i32, // zones are emitted in panel order, so index == Id
            }
            .to_xml(),
        );
        cursor_mm = fmt_mm(mm_to);
        cursor_px = src_to;
    }

    // Trailing catch-all: the run past the last neighbour (or the whole edge
    // when the panel has none, i.e. the outer rim of the desktop).
    out.push_str(
        &Link {
            span_mm: (&cursor_mm, F64_MAX),
            source_px: (cursor_px, i32::MAX),
            target_px: (i32::MIN, i32::MAX),
            resistance: 0.0,
            target_id: -1,
        }
        .to_xml(),
    );

    format!("<{el}>{out}</{el}>", el = edge.element())
}

/// C# `InvariantCulture` double formatting, close enough for the values used
/// here (no exponent, `.` as the decimal separator).
fn fmt_mm(v: f64) -> String {
    if v == v.trunc() {
        format!("{}", v as i64)
    } else {
        format!("{v}")
    }
}

/// Serialize `panels` into the `<ZonesLayout>` XML the daemon receives.
///
/// `resistance` is applied to every real link (0 = free borders, the default a
/// benchmark wants: a resisted border short-circuits into `no_zone_matches` and
/// would measure the cheap path instead of the crossing).
pub fn layout_xml(panels: &[Panel], algo: Algo, max_travel: f64, resistance: f64) -> String {
    let mut zones = String::new();
    for (i, panel) in panels.iter().enumerate() {
        let mut links = String::new();
        for edge in [Edge::Left, Edge::Top, Edge::Right, Edge::Bottom] {
            links.push_str(&edge_links(panels, i, edge, resistance));
        }

        zones.push_str(&format!(
            concat!(
                r#"<Zone Id="{id}" Name="M{id}" DeviceId="M{id}">"#,
                r#"<PixelsBounds><Rect Left="{pl}" Top="{pt}" Width="{pw}" Height="{ph}"></Rect></PixelsBounds>"#,
                r#"<PhysicalBounds><Rect Left="{ml}" Top="{mt}" Width="{mw}" Height="{mh}"></Rect></PhysicalBounds>"#,
                r#"{links}</Zone>"#,
            ),
            id = i,
            pl = panel.pixels.left(),
            pt = panel.pixels.top(),
            pw = panel.pixels.width(),
            ph = panel.pixels.height(),
            ml = fmt_mm(panel.physical.left()),
            mt = fmt_mm(panel.physical.top()),
            mw = fmt_mm(panel.physical.width()),
            mh = fmt_mm(panel.physical.height()),
            links = links,
        ));
    }

    format!(
        concat!(
            r#"<ZonesLayout AdjustPointer="False" AdjustSpeed="False" LoopX="False" LoopY="False" "#,
            r#"Virtual="False" Priority="Normal" PriorityUnhooked="Below" Algorithm="{algo}" "#,
            r#"MaxTravelDistance="{max_travel}" FreelookCheckInterval="100" FreelookEnabled="True">"#,
            r#"<MainZones>{zones}</MainZones></ZonesLayout>"#,
        ),
        algo = algo.as_attr(),
        max_travel = fmt_mm(max_travel),
        zones = zones,
    )
}

// --- engine harness ---------------------------------------------------------

/// An engine loaded with `panels`, its fake cursor, and the zone ids in panel
/// order — already primed so the first benchmarked event takes the steady-state
/// path rather than `ExtFirst`.
pub struct Harness {
    pub engine: MouseEngine,
    pub env: BenchCursor,
    pub zones: Vec<ZoneId>,
}

impl Harness {
    pub fn new(panels: &[Panel], algo: Algo, max_travel: f64, prime_at: Point<i32>) -> Harness {
        let xml = layout_xml(panels, algo, max_travel, 0.0);
        let layout = ZonesLayout::from_xml(&xml).expect("generated layout must parse");
        assert_eq!(
            layout.main_zones.len(),
            panels.len(),
            "every generated panel must become a main zone"
        );

        let mut engine = MouseEngine::new();
        let zones = layout.zones.clone();
        engine.load(layout);

        let env = BenchCursor::new(desktop_bounds(panels));
        let mut harness = Harness { engine, env, zones };
        // Two events: the first only resolves the starting zone (ExtFirst).
        harness.feed(prime_at);
        harness.feed(prime_at);
        harness
    }

    /// One mouse event through the engine. Returns whether the engine took over
    /// (repositioned the cursor) — the flag the backends use to swallow the event.
    pub fn feed(&mut self, p: Point<i32>) -> bool {
        let mut event = MouseEventArg::new(p);
        self.engine.on_mouse_move(&mut self.env, &mut event);
        event.handled
    }

    pub fn pixels_of(&self, zone: usize) -> Rect<i32> {
        self.engine.layout.arena[self.zones[zone]].pixels_bounds()
    }
}

// --- scenarios --------------------------------------------------------------

/// A closed loop of mouse events: replaying it leaves the engine in the state it
/// started from, so a benchmark can iterate it without a per-iteration reset.
pub struct Scenario {
    pub name: &'static str,
    pub harness: Harness,
    pub path: Vec<Point<i32>>,
    /// What `handled` must be for every event of the path. Asserted by
    /// [`Scenario::verify`], which is how the suite proves each benchmark
    /// exercises the branch its name claims.
    pub expect_handled: bool,
}

impl Scenario {
    /// Run the path once.
    #[inline]
    pub fn run(&mut self) {
        for i in 0..self.path.len() {
            let p = self.path[i];
            std::hint::black_box(self.harness.feed(p));
        }
    }

    /// Replay the loop and check that it really is one: every event must take
    /// the expected branch, on every lap. A path that decayed into interior
    /// moves after the first lap would otherwise benchmark nothing.
    pub fn verify(&mut self) {
        for lap in 0..8 {
            for (i, &p) in self.path.clone().iter().enumerate() {
                let handled = self.harness.feed(p);
                assert_eq!(
                    handled,
                    self.expect_handled,
                    "{}: event {i} of lap {lap} at ({}, {}) was handled={handled}",
                    self.name,
                    p.x(),
                    p.y(),
                );
            }
        }
    }
}

/// Motion that never leaves the current zone — the overwhelmingly common event,
/// and the one whose cost the daemon pays on every single mouse report.
pub fn interior(algo: Algo, zones: usize) -> Scenario {
    let panels = row_panels(zones);
    let bounds = panels[0].pixels;
    let (cx, cy) = (
        bounds.left() + bounds.width() / 2,
        bounds.top() + bounds.height() / 2,
    );
    let harness = Harness::new(&panels, algo, 200.0, Point::new(cx, cy));
    Scenario {
        name: "interior",
        harness,
        // A small square well away from the edge columns, so no border is even
        // a candidate and the resistance re-arm guard is exercised.
        path: vec![
            Point::new(cx + 8, cy),
            Point::new(cx + 8, cy + 8),
            Point::new(cx, cy + 8),
            Point::new(cx, cy),
        ],
        expect_handled: false,
    }
}

/// Crossing the border between the first two zones, back and forth. Every event
/// resolves a link, drains resistance, walks the travel rects and repositions
/// the cursor.
pub fn crossing(algo: Algo, zones: usize) -> Scenario {
    let panels = row_panels(zones);
    let border = panels[0].pixels.right();
    let y = panels[0].pixels.top() + panels[0].pixels.height() / 2;
    let harness = Harness::new(
        &panels,
        algo,
        200.0,
        Point::new(panels[0].pixels.left() + 100, y),
    );
    Scenario {
        name: "crossing",
        harness,
        // One pixel past the border in each direction: the minimal overshoot a
        // real mouse report produces, and the one that must not ping-pong.
        path: vec![Point::new(border, y), Point::new(border - 1, y)],
        expect_handled: true,
    }
}

/// Pushing against the outer rim of the desktop: the search scans every zone,
/// matches none, and the cursor is clipped back. This is the worst case of
/// `find_target_zone` — no early exit is possible, so its cost is the full
/// zone count.
pub fn no_target(algo: Algo, zones: usize) -> Scenario {
    let panels = row_panels(zones);
    let last = panels[zones - 1].pixels;
    let y = last.top() + last.height() / 2;
    let harness = Harness::new(&panels, algo, 200.0, Point::new(last.right() - 100, y));
    Scenario {
        name: "no_target",
        harness,
        // Past the rightmost edge, where there is nothing to cross into.
        // `no_zone_matches` leaves `old_point` untouched, so one event is
        // already a closed loop.
        path: vec![Point::new(last.right(), y)],
        expect_handled: false,
    }
}

/// A long diagonal trip across a monitor wall: the trip line is intersected
/// against every zone rectangle, most of those intersections are real, and the
/// nearest-exit comparison has to run on all of them. `MaxTravelDistance` is
/// deliberately large so nothing is rejected early.
pub fn complex_intersection() -> Scenario {
    let panels = grid_panels(4, 4);
    let first = panels[0].pixels;
    let start = Point::new(first.left() + 40, first.top() + 40);
    let harness = Harness::new(&panels, Algo::Cross, 4000.0, start);
    let far = panels[panels.len() - 1].pixels;
    Scenario {
        name: "complex_intersection",
        harness,
        path: vec![
            Point::new(far.right() - 40, far.bottom() - 40),
            Point::new(first.left() + 40, first.top() + 40),
        ],
        expect_handled: true,
    }
}
