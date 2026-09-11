//! TEMPORARY diagnostic: feed the exact zones XML produced by the C# UI
//! (3×4K side by side, LoopX+LoopY, physical 698×393mm each) to the engine
//! and check screen-edge looping in both algorithms.

use littlebigmouse_hook::engine::cursor::CursorEnv;
use littlebigmouse_hook::engine::event::MouseEventArg;
use littlebigmouse_hook::engine::MouseEngine;
use littlebigmouse_hook::geometry::{Point, Rect};
use littlebigmouse_hook::zones::ZonesLayout;

const XML: &str = include_str!("scratch-loop-zones.xml");

struct FakeCursor {
    pos: Point<i32>,
    clip: Rect<i32>,
    desktop: Rect<i32>,
}

impl FakeCursor {
    fn new() -> Self {
        let desktop = Rect::new(0, 0, 11520, 2160);
        FakeCursor {
            pos: Point::new(0, 0),
            clip: desktop,
            desktop,
        }
    }
}

impl CursorEnv for FakeCursor {
    fn get_mouse_location(&self) -> Point<i32> {
        // evdev parity: report the position clamped into the current clip
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

fn engine(algorithm: &str) -> MouseEngine {
    let xml = XML.replace(
        r#"Algorithm="Strait""#,
        &format!(r#"Algorithm="{algorithm}""#),
    );
    let mut e = MouseEngine::new();
    let layout = ZonesLayout::from_xml(&xml).expect("parse zones xml");
    assert!(layout.loop_x && layout.loop_y, "loop flags must be parsed");
    e.load(layout);
    e
}

fn feed(eng: &mut MouseEngine, env: &mut FakeCursor, x: i32, y: i32) -> MouseEventArg {
    let mut ev = MouseEventArg::new(Point::new(x, y));
    eng.on_mouse_move(env, &mut ev);
    ev
}

fn run_case(algorithm: &str) {
    // --- horizontal loop: push through the LEFT edge of the leftmost monitor
    let mut eng = engine(algorithm);
    let mut env = FakeCursor::new();
    feed(&mut eng, &mut env, 100, 1000); // init in zone 0
    let mut crossed = None;
    for i in 1..=40 {
        let ev = feed(&mut eng, &mut env, -i, 1000);
        if ev.handled {
            crossed = Some((i, env.pos));
            break;
        }
    }
    match crossed {
        Some((i, pos)) => {
            println!(
                "[{algorithm}] LEFT edge: crossed after {i} px, landed at ({}, {})",
                pos.x(),
                pos.y()
            );
            assert!(
                pos.x() > 7680,
                "[{algorithm}] left loop should land on rightmost monitor, got {}",
                pos.x()
            );
        }
        None => {
            println!("[{algorithm}] LEFT edge: NEVER crossed (loop broken — cursor stays blocked)")
        }
    }

    // --- horizontal loop: push through the RIGHT edge of the rightmost monitor
    let mut eng = engine(algorithm);
    let mut env = FakeCursor::new();
    feed(&mut eng, &mut env, 11400, 1000); // init in zone 2
    let mut crossed = None;
    for i in 1..=40 {
        let ev = feed(&mut eng, &mut env, 11519 + i, 1000);
        if ev.handled {
            crossed = Some((i, env.pos));
            break;
        }
    }
    match crossed {
        Some((i, pos)) => {
            println!(
                "[{algorithm}] RIGHT edge: crossed after {i} px, landed at ({}, {})",
                pos.x(),
                pos.y()
            );
            assert!(
                pos.x() < 3840,
                "[{algorithm}] right loop should land on leftmost monitor, got {}",
                pos.x()
            );
        }
        None => println!("[{algorithm}] RIGHT edge: NEVER crossed (loop broken)"),
    }

    // --- vertical loop: push through the TOP edge of the middle monitor
    let mut eng = engine(algorithm);
    let mut env = FakeCursor::new();
    feed(&mut eng, &mut env, 5000, 100); // init in zone 1
    let mut crossed = None;
    for i in 1..=40 {
        let ev = feed(&mut eng, &mut env, 5000, -i);
        if ev.handled {
            crossed = Some((i, env.pos));
            break;
        }
    }
    match crossed {
        Some((i, pos)) => {
            println!(
                "[{algorithm}] TOP edge: crossed after {i} px, landed at ({}, {})",
                pos.x(),
                pos.y()
            );
            assert!(
                pos.y() > 2000,
                "[{algorithm}] top loop should land near the bottom, got {}",
                pos.y()
            );
        }
        None => println!("[{algorithm}] TOP edge: NEVER crossed (vertical loop broken)"),
    }

    // --- vertical loop: push through the BOTTOM edge of the middle monitor
    let mut eng = engine(algorithm);
    let mut env = FakeCursor::new();
    feed(&mut eng, &mut env, 5000, 2000); // init in zone 1
    let mut crossed = None;
    for i in 1..=40 {
        let ev = feed(&mut eng, &mut env, 5000, 2159 + i);
        if ev.handled {
            crossed = Some((i, env.pos));
            break;
        }
    }
    match crossed {
        Some((i, pos)) => {
            println!(
                "[{algorithm}] BOTTOM edge: crossed after {i} px, landed at ({}, {})",
                pos.x(),
                pos.y()
            );
            assert!(
                pos.y() < 200,
                "[{algorithm}] bottom loop should land near the top, got {}",
                pos.y()
            );
        }
        None => println!("[{algorithm}] BOTTOM edge: NEVER crossed (vertical loop broken)"),
    }
}

#[test]
fn loop_strait() {
    run_case("Strait");
}

#[test]
fn loop_cross() {
    run_case("Cross");
}
