//! Offline reproduction harness: replays pixel-by-pixel mouse motion through the
//! engine with the evdev backend's flush semantics, against a real layout XML.
//!
//! Usage: cargo run --example simulate -- <layout.xml> <start_x> <start_y> <dx> <dy> <steps>

use littlebigmouse_hook::engine::cursor::CursorEnv;
use littlebigmouse_hook::engine::event::MouseEventArg;
use littlebigmouse_hook::engine::MouseEngine;
use littlebigmouse_hook::geometry::{Point, Rect};
use littlebigmouse_hook::zones::ZonesLayout;

/// Mirror of the evdev backend's EvdevCursor (clip-or-desktop clamp, virtual pos).
struct SimCursor {
    virtual_pos: Point<i32>,
    clip: Option<Rect<i32>>,
    desktop: Rect<i32>,
}

impl SimCursor {
    fn clamp(&self, p: Point<i32>) -> Point<i32> {
        let r = self.clip.unwrap_or(self.desktop);
        Point::new(
            p.x().clamp(r.left(), r.right() - 1),
            p.y().clamp(r.top(), r.bottom() - 1),
        )
    }
}

impl CursorEnv for SimCursor {
    fn get_mouse_location(&self) -> Point<i32> {
        self.clamp(self.virtual_pos)
    }
    fn set_mouse_location(&mut self, location: Point<i32>) {
        self.virtual_pos = location;
    }
    fn get_clip(&self) -> Rect<i32> {
        self.clip.unwrap_or(self.desktop)
    }
    fn set_clip(&mut self, r: Rect<i32>) {
        if r.is_empty() || r == self.desktop {
            self.clip = None;
            return;
        }
        self.clip = Some(r);
        self.virtual_pos = self.clamp(self.virtual_pos);
    }
    fn ctrl_down(&self) -> bool {
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

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let xml = std::fs::read_to_string(&args[1]).expect("read layout xml");
    let (sx, sy, dx, dy, steps): (i32, i32, i32, i32, i32) = (
        args[2].parse().unwrap(),
        args[3].parse().unwrap(),
        args[4].parse().unwrap(),
        args[5].parse().unwrap(),
        args[6].parse().unwrap(),
    );

    let mut engine = MouseEngine::new();
    engine.load(ZonesLayout::from_xml(&xml).expect("parse layout"));

    // desktop_bounds equivalent: union of main zone pixel bounds.
    let (mut l, mut t, mut r, mut b) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
    for &z in &engine.layout.main_zones {
        let bounds = engine.layout.arena[z].pixels_bounds();
        l = l.min(bounds.left());
        t = t.min(bounds.top());
        r = r.max(bounds.right());
        b = b.max(bounds.bottom());
        println!(
            "zone {:12} pixels [{}..{})x[{}..{})",
            engine.layout.arena[z].name,
            bounds.left(),
            bounds.right(),
            bounds.top(),
            bounds.bottom()
        );
    }
    let mut env = SimCursor {
        virtual_pos: Point::new(sx, sy),
        clip: None,
        desktop: Rect::new(l, t, r - l, b - t),
    };

    // Prime like Router::arm.
    let mut e = MouseEventArg::new(Point::new(sx, sy));
    engine.on_mouse_move(&mut env, &mut e);
    println!("prime at ({sx},{sy}) handled={}", e.handled);

    for i in 0..steps {
        let old = env.virtual_pos;
        // evdev flush_frame candidate (post-fix: unclamped).
        let candidate = Point::new(old.x().saturating_add(dx), old.y().saturating_add(dy));
        let mut e = MouseEventArg::new(candidate);
        engine.on_mouse_move(&mut env, &mut e);
        if !e.handled {
            env.virtual_pos = env.clamp(candidate);
        }
        println!(
            "step {:3}: candidate ({:5},{:5}) -> emit ({:5},{:5}) {} clip={:?}",
            i,
            candidate.x(),
            candidate.y(),
            env.virtual_pos.x(),
            env.virtual_pos.y(),
            if e.handled { "CROSS" } else { "     " },
            env.clip.map(|c| (c.left(), c.right(), c.top(), c.bottom())),
        );
    }
}
