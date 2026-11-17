//! #747: a section drawn at the end of an edge must also hold a cursor that
//! leaves through the corner, not only one pushed straight at the edge.
//!
//! The layout is the reporter's, compiled by the real link compiler
//! (`lbm-layout`) and read back by the daemon's parser, so the engine sees the
//! exact link list the UI would send — including the link that runs off the
//! end of the edge, which is the one the corner exit used to land on.

use lbm_engine::cursor::CursorEnv;
use lbm_engine::event::MouseEventArg;
use lbm_engine::MouseEngine;
use lbm_geom::{Point, Rect};
use lbm_layout::geo::Rect as LayoutRect;
use lbm_layout::model::{BorderResistance, BorderSection};
use lbm_layout::zoning::{Zone, ZonesLayout as CompiledLayout};
use lbm_zones::ZonesLayout;

/// 23.8" 16:9, primary, 1920x1080 at the origin.
const PRIMARY_MM: (f64, f64) = (527.0, 296.4);
/// 15.6" portrait, 1080x1920 on the primary's right, 22 mm higher.
const PORTRAIT_MM: (f64, f64) = (194.2, 345.3);
/// The drawn section: the top 88 px of the primary's right edge, in mm.
const SECTION_MM: f64 = 88.0 * PRIMARY_MM.1 / 1080.0;

fn primary_px() -> Rect<i32> {
    Rect::new(0, 0, 1920, 1080)
}

fn zone(
    layout: &CompiledLayout,
    borders: BorderResistance,
    name: &str,
    px: LayoutRect,
    mm: LayoutRect,
) -> Zone {
    Zone::new(
        layout.zones.len(),
        borders,
        Some(name.to_owned()),
        Some(name.to_owned()),
        px,
        mm,
        None,
    )
}

/// The reporter's two screens, the primary's right edge carrying `section`.
fn layout(algorithm: &str, section: Option<BorderSection>) -> MouseEngine {
    compile(
        algorithm,
        section,
        LayoutRect::new(1920.0, -406.0, 1080.0, 1920.0),
        LayoutRect::new(PRIMARY_MM.0, -22.0, PORTRAIT_MM.0, PORTRAIT_MM.1),
    )
}

/// The same screens upside down: the portrait sticks out 22 mm *below* the
/// primary, so the corner is the bottom one — the end of the edge, where links
/// are found the other way round.
fn mirrored(algorithm: &str, section: Option<BorderSection>) -> MouseEngine {
    compile(
        algorithm,
        section,
        LayoutRect::new(1920.0, 1160.0 - 1920.0, 1080.0, 1920.0),
        LayoutRect::new(
            PRIMARY_MM.0,
            PRIMARY_MM.1 + 22.0 - PORTRAIT_MM.1,
            PORTRAIT_MM.0,
            PORTRAIT_MM.1,
        ),
    )
}

/// A twin of the primary touching it by its bottom-right corner only: nothing
/// shares an edge with the primary, every crossing goes through the corner.
fn diagonal(algorithm: &str, section: Option<BorderSection>) -> MouseEngine {
    compile(
        algorithm,
        section,
        LayoutRect::new(1920.0, 1080.0, 1920.0, 1080.0),
        LayoutRect::new(PRIMARY_MM.0, PRIMARY_MM.1, PRIMARY_MM.0, PRIMARY_MM.1),
    )
}

/// The primary, whose right edge carries `section`, and one neighbour, through
/// the UI's link compiler and the daemon's parser.
fn compile(
    algorithm: &str,
    section: Option<BorderSection>,
    neighbour_px: LayoutRect,
    neighbour_mm: LayoutRect,
) -> MouseEngine {
    let mut borders = BorderResistance::default();
    borders.right.sections.extend(section);
    compile_with(algorithm, borders, neighbour_px, neighbour_mm)
}

fn compile_with(
    algorithm: &str,
    borders: BorderResistance,
    neighbour_px: LayoutRect,
    neighbour_mm: LayoutRect,
) -> MouseEngine {
    let mut layout = CompiledLayout {
        algorithm: algorithm.to_owned(),
        ..CompiledLayout::default()
    };
    let primary = zone(
        &layout,
        borders,
        "Primary",
        LayoutRect::new(0.0, 0.0, 1920.0, 1080.0),
        LayoutRect::new(0.0, 0.0, PRIMARY_MM.0, PRIMARY_MM.1),
    );
    layout.zones.push(primary);
    let neighbour = zone(
        &layout,
        BorderResistance::default(),
        "Neighbour",
        neighbour_px,
        neighbour_mm,
    );
    layout.zones.push(neighbour);
    layout.init();

    let mut engine = MouseEngine::new();
    engine.load(
        ZonesLayout::from_xml(&layout.serialize()).expect("the daemon parses the UI's layout"),
    );
    engine
}

/// The bottom 88 px of the primary's right edge, blocked.
fn blocked_bottom_corner() -> Option<BorderSection> {
    Some(BorderSection::new(
        PRIMARY_MM.1 - SECTION_MM,
        PRIMARY_MM.1,
        0.0,
        true,
        0.0,
        false,
    ))
}

fn blocked_corner() -> Option<BorderSection> {
    Some(BorderSection::new(0.0, SECTION_MM, 0.0, true, 0.0, false))
}

struct Cursor {
    pos: Point<i32>,
    clip: Rect<i32>,
    ctrl: bool,
}

impl Cursor {
    fn new() -> Self {
        Cursor {
            pos: Point::new(0, 0),
            clip: Rect::new(-10000, -10000, 30000, 20000),
            ctrl: false,
        }
    }
}

impl CursorEnv for Cursor {
    fn get_mouse_location(&self) -> Point<i32> {
        self.pos
    }
    fn set_mouse_location(&mut self, location: Point<i32>) {
        self.pos = location;
    }
    fn get_clip(&self) -> Rect<i32> {
        self.clip
    }
    fn set_clip(&mut self, r: Rect<i32>) {
        self.clip = r;
    }
    fn ctrl_down(&self) -> bool {
        self.ctrl
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

/// Feeds `from` (to settle the engine on the primary) then `to`; `true` when
/// the engine took the cursor across.
fn crosses(
    engine: &mut MouseEngine,
    cursor: &mut Cursor,
    from: (i32, i32),
    to: (i32, i32),
) -> bool {
    engine.on_mouse_move(cursor, &mut MouseEventArg::new(Point::new(from.0, from.1)));
    let mut event = MouseEventArg::new(Point::new(to.0, to.1));
    engine.on_mouse_move(cursor, &mut event);
    if !event.handled {
        assert_eq!(
            cursor.clip,
            primary_px(),
            "a refused crossing confines the cursor to the primary"
        );
    }
    event.handled
}

// --- Strait ------------------------------------------------------------------

/// The reporter's ✅ case, kept as the reference the corner must match.
#[test]
fn strait_pushing_straight_into_the_blocked_section_holds() {
    let mut engine = layout("Strait", blocked_corner());
    assert!(!crosses(
        &mut engine,
        &mut Cursor::new(),
        (1915, 3),
        (1921, 3)
    ));
}

/// The reporter's first ❌: y is already above the primary when x leaves it.
#[test]
fn strait_a_diagonal_exit_through_the_corner_is_held_by_the_section() {
    let mut engine = layout("Strait", blocked_corner());
    let mut cursor = Cursor::new();
    for _ in 0..1000 {
        assert!(!crosses(&mut engine, &mut cursor, (1915, 3), (1921, -3)));
    }
}

#[test]
fn strait_without_a_section_the_corner_stays_open() {
    let mut engine = layout("Strait", None);
    let mut cursor = Cursor::new();
    assert!(crosses(&mut engine, &mut cursor, (1915, 3), (1921, -3)));
    assert!(
        cursor.pos.x() >= 1920,
        "landed on the portrait screen: {:?}",
        cursor.pos
    );
}

#[test]
fn strait_ctrl_still_opens_a_blocked_corner() {
    let mut engine = layout("Strait", blocked_corner());
    let mut cursor = Cursor::new();
    cursor.ctrl = true;
    assert!(crosses(&mut engine, &mut cursor, (1915, 3), (1921, -3)));
}

// --- Cross -------------------------------------------------------------------

/// A steep up-right flick: the trip leaves the primary through its *top* edge,
/// a few pixels short of the corner, and ends on the portrait screen.
const FLICK: ((i32, i32), (i32, i32)) = ((1915, 10), (1925, -20));

#[test]
fn cross_pushing_straight_into_the_blocked_section_holds() {
    let mut engine = layout("Cross", blocked_corner());
    assert!(!crosses(
        &mut engine,
        &mut Cursor::new(),
        (1915, 3),
        (1925, 3)
    ));
}

/// The reporter's second ❌.
#[test]
fn cross_a_flick_over_the_corner_is_held_by_the_section() {
    let mut engine = layout("Cross", blocked_corner());
    let mut cursor = Cursor::new();
    for _ in 0..1000 {
        assert!(!crosses(&mut engine, &mut cursor, FLICK.0, FLICK.1));
    }
}

#[test]
fn cross_without_a_section_the_flick_still_lands_on_the_neighbour() {
    let mut engine = layout("Cross", None);
    let mut cursor = Cursor::new();
    assert!(crosses(&mut engine, &mut cursor, FLICK.0, FLICK.1));
    assert!(
        cursor.pos.x() >= 1920,
        "landed on the portrait screen: {:?}",
        cursor.pos
    );
}

#[test]
fn cross_ctrl_still_opens_a_blocked_corner() {
    let mut engine = layout("Cross", blocked_corner());
    let mut cursor = Cursor::new();
    cursor.ctrl = true;
    assert!(crosses(&mut engine, &mut cursor, FLICK.0, FLICK.1));
}

// --- the bottom corner: the end of the edge ------------------------------------

#[test]
fn strait_the_bottom_corner_is_held_like_the_top_one() {
    let mut engine = mirrored("Strait", blocked_bottom_corner());
    let mut cursor = Cursor::new();
    assert!(!crosses(
        &mut engine,
        &mut cursor,
        (1915, 1076),
        (1921, 1076)
    ));
    assert!(!crosses(
        &mut engine,
        &mut cursor,
        (1915, 1076),
        (1921, 1082)
    ));
}

#[test]
fn cross_the_bottom_corner_is_held_like_the_top_one() {
    let mut engine = mirrored("Cross", blocked_bottom_corner());
    let mut cursor = Cursor::new();
    assert!(!crosses(
        &mut engine,
        &mut cursor,
        (1915, 1070),
        (1925, 1100)
    ));

    let mut engine = mirrored("Cross", None);
    assert!(crosses(
        &mut engine,
        &mut Cursor::new(),
        (1915, 1070),
        (1925, 1100)
    ));
}

// --- a resistance, not a wall ------------------------------------------------------

/// Pushes from the edge without ever settling back inside (which would re-arm
/// the resistance), cycling through `pushes`; the number of pushes it took.
fn pushes_to_cross(engine: &mut MouseEngine, pushes: &[(i32, i32)]) -> Option<usize> {
    pushes_from(engine, (1915, 3), pushes)
}

fn pushes_from(
    engine: &mut MouseEngine,
    settle: (i32, i32),
    pushes: &[(i32, i32)],
) -> Option<usize> {
    let mut cursor = Cursor::new();
    engine.on_mouse_move(
        &mut cursor,
        &mut MouseEventArg::new(Point::new(settle.0, settle.1)),
    );
    (0..1000).find(|i| {
        let (x, y) = pushes[i % pushes.len()];
        let mut event = MouseEventArg::new(Point::new(x, y));
        engine.on_mouse_move(&mut cursor, &mut event);
        event.handled
    })
}

/// Straight and diagonal pushes into the corner drain one and the same
/// resistance: a diagonal push neither slips through it nor starts it over.
#[test]
fn strait_straight_and_diagonal_pushes_drain_one_resistance() {
    let resisted = || {
        layout(
            "Strait",
            Some(BorderSection::new(0.0, SECTION_MM, 5.0, false, 0.0, false)),
        )
    };
    let straight = pushes_to_cross(&mut resisted(), &[(1921, 3)]);
    assert!(straight.is_some_and(|n| n > 5), "{straight:?}");
    assert_eq!(
        pushes_to_cross(&mut resisted(), &[(1921, 3), (1921, -3)]),
        straight
    );
}

// --- a neighbour reached only through the corner ----------------------------------

/// The off-edge link is the only one leading to a diagonal neighbour: holding
/// corners must not close it.
#[test]
fn a_neighbour_touching_only_the_corner_stays_reachable() {
    for algorithm in ["Strait", "Cross"] {
        let mut engine = diagonal(algorithm, None);
        let mut cursor = Cursor::new();
        assert!(
            crosses(&mut engine, &mut cursor, (1915, 1076), (1921, 1082)),
            "{algorithm}"
        );
        assert!(cursor.pos.y() >= 1080, "{algorithm}: {:?}", cursor.pos);
    }
}

/// …unless the user drew a section on that corner: it is the only place they
/// can say so, and the cursor leaves through it.
#[test]
fn a_section_at_the_corner_holds_the_diagonal_neighbour_too() {
    for algorithm in ["Strait", "Cross"] {
        let mut engine = diagonal(algorithm, blocked_bottom_corner());
        assert!(
            !crosses(&mut engine, &mut Cursor::new(), (1915, 1076), (1921, 1082)),
            "{algorithm}"
        );
    }
}

// --- the other way round: out through a side, round the top ---------------------

/// A neighbour *above* the primary, sticking out 22 mm past its right side, and
/// the right end of the primary's top edge blocked. A flat up-right flick
/// leaves through the primary's *right* side, just under the corner, and ends
/// on that neighbour: the top edge's corner section must hold it.
#[test]
fn cross_a_flick_round_the_top_corner_is_held_by_the_top_section() {
    let section_mm = 88.0 * PRIMARY_MM.0 / 1920.0;
    let above = |blocked: bool| {
        let mut borders = BorderResistance::default();
        if blocked {
            borders.top.sections.push(BorderSection::new(
                PRIMARY_MM.0 - section_mm,
                PRIMARY_MM.0,
                0.0,
                true,
                0.0,
                false,
            ));
        }
        compile_with(
            "Cross",
            borders,
            LayoutRect::new(1920.0 + 80.0 - 1920.0, -1080.0, 1920.0, 1080.0),
            LayoutRect::new(22.0, -PRIMARY_MM.1, PRIMARY_MM.0, PRIMARY_MM.1),
        )
    };
    let flick = ((1910, 5), (1940, -5));
    assert!(crosses(
        &mut above(false),
        &mut Cursor::new(),
        flick.0,
        flick.1
    ));
    assert!(!crosses(
        &mut above(true),
        &mut Cursor::new(),
        flick.0,
        flick.1
    ));
}

// --- review of #760 ----------------------------------------------------------------

/// The last `px` pixels of the primary's top edge, ending at the corner, in mm.
fn top_corner(resistance: f64, blocked: bool) -> BorderSection {
    let length = 88.0 * PRIMARY_MM.0 / 1920.0;
    BorderSection::new(
        PRIMARY_MM.0 - length,
        PRIMARY_MM.0,
        resistance,
        blocked,
        0.0,
        false,
    )
}

/// The reporter's screens with sections on both edges meeting at the corner.
fn both_edges(top: Option<BorderSection>, right: Option<BorderSection>) -> MouseEngine {
    let mut borders = BorderResistance::default();
    borders.top.sections.extend(top);
    borders.right.sections.extend(right);
    compile_with(
        "Cross",
        borders,
        LayoutRect::new(1920.0, -406.0, 1080.0, 1920.0),
        LayoutRect::new(PRIMARY_MM.0, -22.0, PORTRAIT_MM.0, PORTRAIT_MM.1),
    )
}

/// The received point stops short of the portrait (x < 1920): Cross reaches it
/// by projecting the direction further. The corner is cut all the same.
#[test]
fn cross_a_flick_projected_past_the_corner_is_held_by_the_section() {
    let flick = ((1915, 10), (1919, -20));
    let mut open = layout("Cross", None);
    assert!(
        crosses(&mut open, &mut Cursor::new(), flick.0, flick.1),
        "the projection reaches the portrait"
    );
    let mut engine = layout("Cross", blocked_corner());
    assert!(!crosses(&mut engine, &mut Cursor::new(), flick.0, flick.1));
}

/// The corner rule adds the adjacent edge's section; it must not take away
/// the one on the edge the trip actually crossed.
#[test]
fn cross_a_blocked_top_corner_still_holds_with_the_right_edge_open() {
    let mut engine = both_edges(Some(top_corner(0.0, true)), None);
    assert!(!crosses(&mut engine, &mut Cursor::new(), FLICK.0, FLICK.1));
}

/// Both edges resisting: the stricter one governs, whichever edge carries it.
#[test]
fn cross_with_both_edges_resisting_the_stricter_governs() {
    let push = [(1925, -20)];
    let count = |top: f64, right: f64| {
        let top = (top > 0.0).then(|| top_corner(top, false));
        let right =
            (right > 0.0).then(|| BorderSection::new(0.0, SECTION_MM, right, false, 0.0, false));
        pushes_from(&mut both_edges(top, right), (1915, 3), &push)
    };
    let weak = count(20.0, 0.0);
    let strong = count(0.0, 60.0);
    assert!(weak.is_some_and(|n| n > 0), "{weak:?}");
    assert!(strong > weak, "{strong:?} vs {weak:?}");
    assert_eq!(count(20.0, 60.0), strong, "the right edge is stricter");
    assert_eq!(count(60.0, 20.0), count(60.0, 0.0), "the top edge is stricter");
}
