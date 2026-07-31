//! End-to-end contract for border sections (#458, #143, #250).
//!
//! Sections never reach the daemon as a concept: the C# link compiler turns them
//! into extra cut points in the per-side `ZoneLink` list it already builds. The
//! fixture here is therefore not hand-written — it is the **real payload emitted
//! by `ZonesLayout.Serialize()`** for two 1920x1080 monitors whose shared edge
//! carries one section blocking its top half. Pinning the actual wire bytes is
//! what makes this a C#-to-Rust contract rather than two sides agreeing in
//! isolation: regenerate it if the serializer changes, and the diff shows exactly
//! what the daemon will now receive.
//!
//! `sliding_along_a_blocked_run_reaches_the_open_one` is the load-bearing one.
//! When resistance holds, `no_zone_matches` clips the cursor to the *whole*
//! monitor, so it was not obvious that pushing against the wall and sliding down
//! to the gap would still cross — and that gesture is the entire point of #458.

use littlebigmouse_hook::engine::cursor::CursorEnv;
use littlebigmouse_hook::engine::event::MouseEventArg;
use littlebigmouse_hook::engine::MouseEngine;
use littlebigmouse_hook::geometry::{Point, Rect};
use littlebigmouse_hook::zones::ZonesLayout;

const XML: &str = include_str!("border-sections-zones.xml");

/// Two 1920x1080 monitors of equal DPI side by side, so the edge mapping is the
/// identity and every expected landing point is obvious. Left's right edge is
/// split at y=540: 0..540 blocked, 540..1080 free.
const SPLIT_Y: i32 = 540;

struct FakeCursor {
    pos: Point<i32>,
    clip: Rect<i32>,
    desktop: Rect<i32>,
    buttons: bool,
}

impl FakeCursor {
    fn new() -> Self {
        let desktop = Rect::new(-1920, 0, 3840, 1080);
        FakeCursor {
            pos: Point::new(0, 0),
            clip: desktop,
            desktop,
            buttons: false,
        }
    }
}

impl CursorEnv for FakeCursor {
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
        false
    }
    fn buttons_down(&self) -> bool {
        self.buttons
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

fn engine() -> MouseEngine {
    let mut eng = MouseEngine::new();
    eng.load(ZonesLayout::from_xml(XML).expect("fixture parses"));
    eng
}

fn feed(eng: &mut MouseEngine, env: &mut FakeCursor, x: i32, y: i32) -> bool {
    let mut ev = MouseEventArg::new(Point::new(x, y));
    eng.on_mouse_move(env, &mut ev);
    ev.handled
}

#[test]
fn blocked_run_confines_while_the_open_run_still_crosses() {
    // Top half: walled off.
    let mut eng = engine();
    let mut env = FakeCursor::new();
    feed(&mut eng, &mut env, -100, 200);
    assert!(
        !feed(&mut eng, &mut env, 0, 200),
        "the blocked half of the edge must not let the cursor through"
    );

    // Bottom half of the very same edge: free, and the crossing lands 1:1.
    let mut eng = engine();
    let mut env = FakeCursor::new();
    feed(&mut eng, &mut env, -100, 800);
    assert!(feed(&mut eng, &mut env, 0, 800));
    assert_eq!(env.pos, Point::new(0, 800));
}

#[test]
fn sliding_along_a_blocked_run_reaches_the_open_one() {
    // Push into the wall, then slide down the border into the gap without ever
    // backing off — the gesture #458 is asking for. One continuous sweep: every
    // frame above the split must be refused, and the first frame below it must
    // cross.
    let mut eng = engine();
    let mut env = FakeCursor::new();
    feed(&mut eng, &mut env, -100, 200);

    let mut crossed_at = None;
    for y in (200..=800).step_by(20) {
        if feed(&mut eng, &mut env, 0, y) {
            crossed_at = Some(y);
            break;
        }
        assert!(
            y < SPLIT_Y,
            "the open run below y={SPLIT_Y} must let the cursor through, but y={y} was refused"
        );
    }

    assert_eq!(
        crossed_at,
        Some(SPLIT_Y),
        "the crossing must happen on the first frame that reaches the open run"
    );
    assert_eq!(env.pos, Point::new(0, SPLIT_Y));
}

#[test]
fn the_block_is_what_holds_the_cursor_back() {
    // Control for the tests above: with the very same fixture minus the block
    // attributes, the top half crosses immediately. Without this, a broken
    // fixture (no target, bad pixel ranges) would make the block tests pass for
    // entirely the wrong reason.
    let unblocked = XML
        .replace(r#"MoveBlock="True""#, r#"MoveBlock="False""#)
        .replace(r#"DragBlock="True""#, r#"DragBlock="False""#);
    assert_ne!(unblocked, XML, "the block attributes must be there to clear");

    let mut eng = MouseEngine::new();
    eng.load(ZonesLayout::from_xml(&unblocked).expect("fixture parses"));
    let mut env = FakeCursor::new();
    feed(&mut eng, &mut env, -100, 200);
    assert!(feed(&mut eng, &mut env, 0, 200));
    assert_eq!(env.pos, Point::new(0, 200));
}

#[test]
fn blocking_one_direction_leaves_the_other_open() {
    // #143: links are per source zone and per side, so a wall on Left's right
    // edge says nothing about Right's left edge. Crossing Right -> Left over the
    // very same pixels stays free.
    let mut eng = engine();
    let mut env = FakeCursor::new();
    feed(&mut eng, &mut env, 100, 200);
    assert!(
        feed(&mut eng, &mut env, -1, 200),
        "the reverse direction must be unaffected by the forward block"
    );
    assert_eq!(env.pos, Point::new(-1, 200));
}

#[test]
fn a_held_button_does_not_change_a_border_blocked_in_both_modes() {
    let mut eng = engine();
    let mut env = FakeCursor::new();
    env.buttons = true;
    feed(&mut eng, &mut env, -100, 200);
    assert!(!feed(&mut eng, &mut env, 0, 200));

    // …and the open run stays open while dragging.
    let mut eng = engine();
    let mut env = FakeCursor::new();
    env.buttons = true;
    feed(&mut eng, &mut env, -100, 800);
    assert!(feed(&mut eng, &mut env, 0, 800));
}
