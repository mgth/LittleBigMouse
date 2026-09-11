//! Timing and allocation profile of the mouse-hook hot path.
//!
//! ```text
//! cargo bench --bench mouse_hook           # allocations + timings
//! cargo bench --bench mouse_hook -- --test # run each mode once, no timing (short)
//! ```
//!
//! This is the layer *above* `mouse_engine`: the per-report code every backend's
//! callback runs before and around `on_mouse_move` — dedup the report against the
//! last location, take the engine under a non-blocking `try_lock`, and act on the
//! outcome. It measures the neutral core in `hook::hot_path`, the exact code the
//! Windows `WH_MOUSE_LL` callback is now a thin shim over, so a regression on the
//! hot path shows up here on any host. No hook is installed, no device opened, the
//! real pointer never moves — the engine talks to `support::BenchCursor`, whose
//! every method is a field read.
//!
//! Four modes are reported, each the distinct branch it names:
//!
//! * **dedup** — a report at the unchanged pixel, dropped before the lock. The
//!   cheapest possible outcome and, on a stationary pointer receiving synthesized
//!   reports, a common one.
//! * **passthrough** — an interior move: dedup passes, the engine runs, nothing
//!   crosses. The overwhelmingly common *working* event, 1000×/s per device.
//! * **crossing** — the report steps over a border: the engine warps the cursor
//!   and the callback swallows the event.
//! * **contended** — a `Load` holds the engine lock. `try_lock` fails, the event
//!   passes straight through. The path that must never block the pump thread.
//!
//! Read the columns knowing what each is worth, exactly as the other benches say:
//!
//! * **allocs / bytes per event** are exact and identical on every machine — this
//!   is the number to diff. The ordinary paths (dedup, passthrough, contended)
//!   must read **0**; a change that starts allocating or logging there shows up
//!   here as a whole number.
//! * **ns per event** is latency, and it is machine-dependent (CPU, governor,
//!   allocator, build flags). Compare runs on the same machine, back to back.

mod support;

#[global_allocator]
static ALLOCATOR: support::alloc::Counting = support::alloc::Counting;

use std::sync::Mutex;
use std::time::Instant;

use littlebigmouse_hook::engine::MouseEngine;
use littlebigmouse_hook::geometry::Point;
use littlebigmouse_hook::hook::hot_path::{count_event, route_move, MoveDedup, Routed};
use littlebigmouse_hook::zones::ZonesLayout;

use support::{desktop_bounds, layout_xml, row_panels, Algo, BenchCursor};

/// One backend callback's worth of state: the deduper, the engine behind its
/// mutex, and the fake cursor the engine drives. A real callback also holds the
/// mutex behind a `static`; here it is a local so each mode gets a clean one.
struct Hook {
    dedup: MoveDedup,
    engine: Mutex<MouseEngine>,
    env: BenchCursor,
}

impl Hook {
    fn new(zones: usize, algo: Algo) -> Hook {
        let panels = row_panels(zones);
        let xml = layout_xml(&panels, algo, 200.0, 0.0);
        let mut engine = MouseEngine::new();
        engine.load(ZonesLayout::from_xml(&xml).expect("generated layout must parse"));
        Hook {
            dedup: MoveDedup::new(),
            engine: Mutex::new(engine),
            env: BenchCursor::new(desktop_bounds(&panels)),
        }
    }

    /// The full callback body over one report: dedup, count, route. Returns the
    /// routing outcome (or `None` when the report was deduped away). This mirrors
    /// `hook/windows/mouse.rs::process` exactly, minus the Win32 message decode.
    ///
    /// Fields are borrowed disjointly (`&engine`, `&mut dedup`, `&mut env`) rather
    /// than through `&mut self`, so the contended runner can hold a guard on
    /// `engine` — the very thing being contended — while this still runs.
    #[inline]
    fn report(&mut self, loc: (i32, i32)) -> Option<Routed> {
        let Hook { dedup, engine, env } = self;
        Self::report_parts(dedup, engine, env, loc)
    }

    #[inline]
    fn report_parts(
        dedup: &mut MoveDedup,
        engine: &Mutex<MouseEngine>,
        env: &mut BenchCursor,
        loc: (i32, i32),
    ) -> Option<Routed> {
        if !dedup.accept(loc) {
            return None;
        }
        count_event();
        let routed = route_move(engine, env, Point::new(loc.0, loc.1));
        if routed.handled() {
            // The engine warped the cursor; the next report is a genuine change.
            dedup.reset();
        }
        Some(routed)
    }
}

// --- modes ------------------------------------------------------------------

/// The pixel geometry the row fixture always uses (see `support::row_panels`).
const PIXEL_W: i32 = 3840;
const PIXEL_H: i32 = 2160;

/// Build a hook primed in the first zone, and return it with the two-report
/// closed loop that drives the given mode. Priming takes the first `ExtFirst`
/// event out of the measured path.
fn mode(name: &'static str, zones: usize, algo: Algo) -> Mode {
    let y = PIXEL_H / 2;
    let interior_a = (PIXEL_W / 2, y);
    let interior_b = (PIXEL_W / 2 + 8, y);
    let border = (PIXEL_W, y); // first pixel of the second zone

    let mut hook = Hook::new(zones, algo);
    // Prime: ExtFirst resolves the starting zone without moving anything.
    hook.report(interior_a);
    hook.report(interior_a);

    let (path, expect): (Vec<(i32, i32)>, ModeExpect) = match name {
        // Same pixel every time: dedup drops it, nothing else runs.
        "dedup" => (vec![interior_a], ModeExpect::Deduped),
        // Two interior pixels, neither equal to the prime position, so dedup
        // accepts both and the loop closes (…b, a, b, a…): both pass the engine,
        // neither crosses.
        "passthrough" => (vec![interior_b, interior_a], ModeExpect::Passed),
        // One pixel past the border and back: each report crosses.
        "crossing" => (vec![border, (border.0 - 1, y)], ModeExpect::Crossed),
        // The lock is held for the whole run (see the contended runner), so the
        // engine never runs; a moving path (…b, a…) keeps every report past dedup
        // so each one actually reaches — and fails — the `try_lock`.
        "contended" => (vec![interior_b, interior_a], ModeExpect::Contended),
        other => panic!("unknown mode {other}"),
    };

    Mode {
        name,
        hook,
        path,
        expect,
        events_per_lap: 0, // filled by verify()
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ModeExpect {
    Deduped,
    Passed,
    Crossed,
    Contended,
}

struct Mode {
    name: &'static str,
    hook: Hook,
    path: Vec<(i32, i32)>,
    expect: ModeExpect,
    events_per_lap: u64,
}

impl Mode {
    /// Drive one lap of the mode's report loop. `contended` holds the engine lock
    /// for the whole lap so every route fails `try_lock`; the others let it run.
    #[inline]
    fn run(&mut self) {
        let Hook { dedup, engine, env } = &mut self.hook;
        // Shared reborrow: `route_move` takes `&Mutex`, and locking through a
        // shared reference is what makes the concurrent `try_lock` below actually
        // contend (a `&mut Mutex::lock` would be a no-op the compiler could elide).
        let engine: &Mutex<MouseEngine> = engine;
        if self.expect == ModeExpect::Contended {
            // A second lock on the same mutex is exactly what a `Load` does while
            // swapping the layout — every route below then hits `WouldBlock`.
            let held = engine.lock().unwrap();
            for &loc in &self.path {
                std::hint::black_box(Hook::report_parts(dedup, engine, env, loc));
            }
            drop(held);
        } else {
            for &loc in &self.path {
                std::hint::black_box(Hook::report_parts(dedup, engine, env, loc));
            }
        }
    }

    /// Replay the loop and assert every report takes the branch the mode is named
    /// after — the guard that keeps a benchmark from silently decaying into a
    /// cheaper path. Records the events per lap for the per-event divisor.
    fn verify(&mut self) {
        let mut events = 0u64;
        for lap in 0..8 {
            events = 0;
            let path = self.path.clone();
            let contended = self.expect == ModeExpect::Contended;
            let Hook { dedup, engine, env } = &mut self.hook;
            let engine: &Mutex<MouseEngine> = engine;
            let held = contended.then(|| engine.lock().unwrap());
            for (i, &loc) in path.iter().enumerate() {
                let routed = Hook::report_parts(dedup, engine, env, loc);
                events += 1;
                match self.expect {
                    ModeExpect::Deduped => assert_eq!(
                        routed, None,
                        "{}: report {i} of lap {lap} must be deduped",
                        self.name
                    ),
                    ModeExpect::Passed => assert_eq!(
                        routed,
                        Some(Routed::Passed),
                        "{}: report {i} of lap {lap} must pass through the engine",
                        self.name
                    ),
                    ModeExpect::Crossed => assert_eq!(
                        routed,
                        Some(Routed::Crossed),
                        "{}: report {i} of lap {lap} must cross",
                        self.name
                    ),
                    ModeExpect::Contended => assert_eq!(
                        routed,
                        Some(Routed::Contended),
                        "{}: report {i} of lap {lap} must find the lock held",
                        self.name
                    ),
                }
            }
            drop(held);
        }
        self.events_per_lap = events;
    }
}

// --- measurement ------------------------------------------------------------

struct Row {
    mode: &'static str,
    allocs_per_event: f64,
    bytes_per_event: f64,
    ns_per_event: f64,
}

/// Warm up, count allocations over a long run, then time a separate run — the
/// same split `evdev_pump` uses: the counters are exact whatever the scheduler
/// does, and the timing wants the minimum of several repetitions.
fn measure(mut mode: Mode, test_only: bool) -> Row {
    mode.verify();
    let per_lap = mode.events_per_lap;

    let warmup = if test_only { 1 } else { 1_000 };
    let iters: u64 = if test_only { 1 } else { 200_000 };
    let reps = if test_only { 1 } else { 7 };

    for _ in 0..warmup {
        mode.run();
    }

    support::alloc::reset();
    for _ in 0..iters {
        mode.run();
    }
    let (allocs, bytes) = support::alloc::counts();

    let mut best = f64::MAX;
    for _ in 0..reps {
        let start = Instant::now();
        for _ in 0..iters {
            mode.run();
        }
        let ns = start.elapsed().as_nanos() as f64 / (iters * per_lap) as f64;
        best = best.min(ns);
    }

    let events = (iters * per_lap) as f64;
    Row {
        mode: mode.name,
        allocs_per_event: allocs as f64 / events,
        bytes_per_event: bytes as f64 / events,
        ns_per_event: if test_only { 0.0 } else { best },
    }
}

fn main() {
    // Short mode for `cargo test` / CI smoke: run each branch once, no timing.
    let test_only = std::env::args().any(|a| a == "--test");

    // Take stdout's one-off buffer allocation before any measurement.
    println!(
        "mouse hook hot path — one event = one report through dedup + non-blocking route.\n\
         2 zones, Strait; the ordinary paths (dedup, passthrough, contended) must read 0 allocs.\n"
    );

    let modes = [
        mode("dedup", 2, Algo::Strait),
        mode("passthrough", 2, Algo::Strait),
        mode("crossing", 2, Algo::Strait),
        mode("contended", 2, Algo::Strait),
    ];

    let rows: Vec<Row> = modes.into_iter().map(|m| measure(m, test_only)).collect();

    println!(
        "{:<14}  {:>13}  {:>12}  {:>10}",
        "mode", "allocs/event", "bytes/event", "ns/event"
    );
    println!("{:-<14}  {:->13}  {:->12}  {:->10}", "", "", "", "");
    for row in &rows {
        println!(
            "{:<14}  {:>13.2}  {:>12.1}  {:>10}",
            row.mode,
            row.allocs_per_event,
            row.bytes_per_event,
            if test_only {
                "—".to_string()
            } else {
                format!("{:.1}", row.ns_per_event)
            },
        );
    }

    // A regression guard the benchmark can afford because these counts are exact:
    // the ordinary per-event paths must not allocate. Crossing is allowed its one
    // cached travel-path Vec clone (see mouse_engine's alloc profile) and is not
    // asserted here.
    for row in &rows {
        if matches!(row.mode, "dedup" | "passthrough" | "contended") {
            assert_eq!(
                row.allocs_per_event, 0.0,
                "{} must not allocate on the hot path (got {} allocs/event)",
                row.mode, row.allocs_per_event
            );
        }
    }
}
