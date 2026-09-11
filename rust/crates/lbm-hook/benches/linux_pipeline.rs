//! End-to-end timing and allocation profile of the Linux input pipeline.
//!
//! ```text
//! cargo bench --bench linux_pipeline           # full baseline
//! cargo bench --bench linux_pipeline -- --quick  # short mode, fewer iterations
//! ```
//!
//! Where [`evdev_pump`](../evdev_pump.rs) measures the *plumbing* alone (poll set,
//! drain, routing, frame composition) and [`mouse_engine`](../mouse_engine.rs)
//! measures the traversal *engine* alone, this one puts the two together and
//! drives them the way a live session does: a stream of mouse reports at a fixed
//! polling rate, each one accumulated into a frame, run through
//! `MouseEngine::on_mouse_move`, and composed into the pointer/keyboard frames the
//! two virtual devices would be handed.
//!
//! It reproduces the body of `Router::flush_frame` — the acceleration curve, the
//! sub-pixel remainder, the engine call, the clamp, the frame composition — minus
//! exactly the two things that need privilege or hardware: the `uinput` writes and
//! the `Shared`/`try_lock` bookkeeping. No device is opened, no `/dev/uinput` node
//! is created, no grab happens and the real pointer never moves, so this runs
//! anywhere, unprivileged, and never touches the user's mice. Off Linux it prints
//! a line and exits (the evdev types are Linux-only).
//!
//! ## What is driven
//!
//! Four report streams, each a closed loop over the synthetic layouts of
//! `benches/support` (the same 2/6/16-monitor rows and the 4x4 wall the engine
//! benchmarks use), so no fixture is duplicated:
//!
//! * **motion** — the overwhelmingly common case: small moves that never leave
//!   the current monitor;
//! * **crossing** — moves that step over a border back and forth, so the engine
//!   resolves a link, walks the travel path and repositions the cursor on every
//!   report;
//! * **partial** — every other report split across two reads (no `SYN_REPORT` on
//!   the first), the case where the pump flushes a partial frame and completes it
//!   next cycle;
//! * **combined** — motion with a keyboard usage riding the same reports (a
//!   combined receiver node's onboard macro key), so both virtual devices get a
//!   frame.
//!
//! ## What is reported
//!
//! Per stream and per polling rate (125 / 500 / 1000 Hz — the report cadences of a
//! low-end, a mid and a 1 kHz gaming mouse):
//!
//! * **ns/frame** — the time to take one report from raw events to composed
//!   frames. Machine-dependent latency: compare runs on the same host, never
//!   across machines;
//! * **frames/s** — the same figure as throughput, i.e. how many reports per
//!   second one core sustains;
//! * **budget %** — ns/frame against the per-report budget of the rate (8 ms at
//!   125 Hz, 1 ms at 1 kHz). This is the "processing depth / lag" figure the task
//!   asks for: below 100 % the pump keeps up with that mouse; the headroom is what
//!   is left for a burst or a second device on the same core;
//! * **allocs/frame** and **bytes/frame** — exact, identical on every machine (the
//!   counting allocator of `support::alloc`). Zero on the steady-state paths is
//!   the property `PumpBuffers` exists to guarantee; a non-zero here is a
//!   regression to investigate, not a threshold to fail on.
//!
//! No assertion fails on a number: this records a baseline. `Stream::verify` does
//! assert the stream still exercises the branch its name claims (a crossing that
//! decayed into interior moves would report a great, meaningless figure), which is
//! the one thing worth failing on.

mod support;

#[global_allocator]
static ALLOCATOR: support::alloc::Counting = support::alloc::Counting;

#[cfg(not(target_os = "linux"))]
fn main() {
    println!("linux_pipeline: the evdev/uinput pipeline is Linux-only, nothing to measure here.");
}

#[cfg(target_os = "linux")]
fn main() {
    linux::main();
}

#[cfg(target_os = "linux")]
mod linux {
    use std::time::Instant;

    use evdev::{EventType, InputEvent, KeyCode, RelativeAxisCode, SynchronizationCode};

    use littlebigmouse_hook::engine::cursor::CursorEnv;
    use littlebigmouse_hook::engine::event::MouseEventArg;
    use littlebigmouse_hook::engine::MouseEngine;
    use littlebigmouse_hook::geometry::{Point, Rect};
    use littlebigmouse_hook::hook::linux::accel::{PointerAccel, Profile};
    use littlebigmouse_hook::hook::linux::evdev::{EvdevCursor, Frame, PumpBuffers};
    use littlebigmouse_hook::zones::ZonesLayout;

    use crate::support::{self, desktop_bounds, layout_xml, row_panels, Algo, Panel};

    /// The report cadences we sweep: a low-end mouse, a mid one, and a 1 kHz
    /// gaming mouse. The per-report budget is `1 / rate`, and the pump has to
    /// finish a report inside it to keep up with that device on one core.
    const RATES_HZ: [u32; 3] = [125, 500, 1000];

    /// A stream constructor and the zone count to build it at — one row of the
    /// sweep. Named so the sweep table below is not a "very complex type".
    type Case = (fn(usize) -> Stream, usize);

    // --- event helpers --------------------------------------------------------

    fn rel(axis: RelativeAxisCode, value: i32) -> InputEvent {
        InputEvent::new(EventType::RELATIVE.0, axis.0, value)
    }

    fn key(code: KeyCode, value: i32) -> InputEvent {
        InputEvent::new(EventType::KEY.0, code.0, value)
    }

    fn syn() -> InputEvent {
        InputEvent::new(
            EventType::SYNCHRONIZATION.0,
            SynchronizationCode::SYN_REPORT.0,
            0,
        )
    }

    // --- the pipeline ---------------------------------------------------------

    /// The whole Linux pipeline for one grabbed mouse, minus the OS: the reused
    /// buffers, the authoritative cursor, the traversal engine, the per-device
    /// acceleration curve and the sub-pixel remainder.
    ///
    /// [`cycle`](Self::cycle) is `Router::pump` + `Router::flush_frame` with the
    /// two privileged steps removed — no `uinput.emit`, no `Shared::try_lock` — so
    /// what is timed is every allocation and every branch a real report pays
    /// except the two syscalls a benchmark cannot make.
    struct Pipeline {
        bufs: PumpBuffers,
        env: EvdevCursor,
        engine: MouseEngine,
        accel: PointerAccel,
        rem: (f64, f64),
        desktop: Rect<i32>,
        /// Where a loop begins. The pipeline is delta-driven, and the engine
        /// repositions the cursor on a crossing, so a raw back-and-forth would
        /// drift away from the border over the laps. Pinning the position here at
        /// the top of each loop (a field write, no allocation) makes every stream
        /// a genuinely closed loop: the same reports produce the same crossings on
        /// every lap, which is what `Stream::verify` relies on.
        start: Point<i32>,
    }

    impl Pipeline {
        /// Load `panels` as the daemon would (through the XML the C# UI sends),
        /// place the cursor at `start`, and prime the engine there so the first
        /// measured report takes the steady-state path rather than `ExtFirst`.
        fn new(panels: &[Panel], algo: Algo, start: Point<i32>) -> Pipeline {
            let desktop = desktop_bounds(panels);
            let xml = layout_xml(panels, algo, 200.0, 0.0);
            let layout = ZonesLayout::from_xml(&xml).expect("generated layout must parse");

            let mut engine = MouseEngine::new();
            engine.load(layout);

            let mut env = EvdevCursor::new(desktop, start);
            // Two events: the first only resolves the starting zone (ExtFirst),
            // exactly as the engine benchmarks prime their harness.
            for _ in 0..2 {
                let mut e = MouseEventArg::new(start);
                engine.on_mouse_move(&mut env, &mut e);
            }

            Pipeline {
                bufs: PumpBuffers::new(),
                env,
                engine,
                // Flat curve, unit speed: a deterministic multiplier, not the
                // kcminputrc-derived one (the pump must stay I/O-free; loading a
                // config would read the filesystem). The pipeline's shape is what
                // is measured, not any particular acceleration setting.
                accel: PointerAccel::new(Profile::Flat, 0.0),
                rem: (0.0, 0.0),
                desktop,
                start,
            }
        }

        /// Pin the cursor back to the loop's start. A field write on the cursor —
        /// no allocation, no engine call — so it does not distort the per-frame
        /// figures; it only guarantees the delta-driven loop stays closed across
        /// laps despite the engine's repositioning on a crossing.
        #[inline]
        fn rewind(&mut self) {
            self.env.set_mouse_location(self.start);
            self.rem = (0.0, 0.0);
        }

        /// One pump cycle over the events a single `poll` + drain handed us: route
        /// each event, and on every `SYN_REPORT` run the engine and compose the
        /// frames. Returns whether the engine took over (a crossing) on this
        /// cycle, which `Stream::verify` checks.
        fn cycle(&mut self, events: &[InputEvent]) -> bool {
            self.bufs.refill_events(events.iter().copied());
            let mut handled = false;
            for k in 0..self.bufs.event_count() {
                let ev = self.bufs.event(k);
                if self.bufs.push(ev, &mut self.env, 0) == Frame::Complete {
                    handled |= self.flush();
                }
            }
            // A partial frame (no trailing SYN) leaves motion pending: emit it, as
            // the pump does at the end of a cycle rather than holding it.
            handled | self.flush()
        }

        /// `Router::flush_frame` minus the uinput writes and the `Shared` lock:
        /// take the motion, apply the acceleration curve and the sub-pixel
        /// remainder, run the engine over the candidate point, clamp, then compose
        /// the pointer and keyboard frames.
        fn flush(&mut self) -> bool {
            if !self.bufs.frame_pending() {
                return false;
            }

            let mut handled = false;
            let acc = self.bufs.take_motion();
            if acc != (0, 0) {
                let now = self.env.tick_count() * 1000; // micros, monotonic enough here
                let (ax, ay) = self.accel.apply(acc.0 as f64, acc.1 as f64, now);
                let sx = ax + self.rem.0;
                let sy = ay + self.rem.1;
                let (dx, dy) = (sx.trunc() as i32, sy.trunc() as i32);
                self.rem = (sx - dx as f64, sy - dy as f64);

                let old = self.env.get_mouse_location();
                let candidate = Point::new(old.x().saturating_add(dx), old.y().saturating_add(dy));

                let mut e = MouseEventArg::new(candidate);
                self.engine.on_mouse_move(&mut self.env, &mut e);
                if !e.handled {
                    self.env.set_mouse_location(candidate);
                }
                handled = e.handled;
            }

            // Compose the pointer frame (ABS point + passthrough) exactly as
            // `emit_absolute` does, minus the `virt.emit`.
            let ax = self.env.get_mouse_location().x() - self.desktop.left();
            let ay = self.env.get_mouse_location().y() - self.desktop.top();
            std::hint::black_box(self.bufs.pointer_frame(ax, ay));

            // Hand the keyboard usages their own frame, minus the `virt_kbd.emit`.
            self.bufs.take_keyboard_frame(|frame| {
                std::hint::black_box(frame);
            });

            handled
        }
    }

    // --- report streams -------------------------------------------------------

    /// One report stream: a closed loop of pump cycles (the outer `Vec`), each the
    /// events one `poll` + drain hands the pipeline. Replaying it returns the
    /// pipeline to its starting state, so it can be iterated without a reset.
    struct Stream {
        name: &'static str,
        pipeline: Pipeline,
        cycles: Vec<Vec<InputEvent>>,
        /// Whether at least one cycle of the loop must cross a border. A stream
        /// named "crossing" that stopped crossing would benchmark nothing.
        expect_crossing: bool,
    }

    impl Stream {
        /// Run the whole loop once, from the pinned start.
        #[inline]
        fn run(&mut self) {
            self.pipeline.rewind();
            for cycle in &self.cycles {
                std::hint::black_box(self.pipeline.cycle(cycle));
            }
        }

        /// How many pump cycles (poll + drain) the loop is, i.e. the unit the
        /// per-frame figures divide by.
        fn cycles_per_iter(&self) -> u64 {
            self.cycles.len() as u64
        }

        /// Replay the loop and check it still exercises the branch its name
        /// claims — on every lap, so a stream that decayed after the first is
        /// caught. Crossing streams must cross at least once per lap; the others
        /// must never cross (they would be measuring the wrong path).
        fn verify(&mut self) {
            for lap in 0..8 {
                self.pipeline.rewind();
                let mut crossed = false;
                for cycle in self.cycles.clone() {
                    crossed |= self.pipeline.cycle(&cycle);
                }
                assert_eq!(
                    crossed, self.expect_crossing,
                    "{}: lap {lap} crossed={crossed}, expected {}",
                    self.name, self.expect_crossing
                );
            }
        }
    }

    /// A motion report: two axes and a SYN. Small enough to stay inside a monitor.
    fn motion(dx: i32, dy: i32) -> Vec<InputEvent> {
        vec![
            rel(RelativeAxisCode::REL_X, dx),
            rel(RelativeAxisCode::REL_Y, dy),
            syn(),
        ]
    }

    /// Ordinary motion that never leaves the starting monitor: a small closed
    /// square, the case the daemon pays on nearly every report.
    fn interior_stream(zones: usize) -> Stream {
        let panels = row_panels(zones);
        let b = panels[0].pixels;
        let start = Point::new(b.left() + b.width() / 2, b.top() + b.height() / 2);
        let pipeline = Pipeline::new(&panels, Algo::Cross, start);
        Stream {
            name: "interior",
            pipeline,
            // A closed square of 8-count moves: back to the origin every loop, so
            // the accumulated remainder and cursor position do not drift.
            cycles: vec![motion(8, 0), motion(0, 8), motion(-8, 0), motion(0, -8)],
            expect_crossing: false,
        }
    }

    /// Reports that step over the first border and back: every loop resolves a
    /// link, walks the travel path and repositions the cursor. The moves are large
    /// enough to clear the border from just inside it.
    fn crossing_stream(zones: usize) -> Stream {
        let panels = row_panels(zones);
        let b = panels[0].pixels;
        let y = b.top() + b.height() / 2;
        // Start a few counts left of the border so one move steps over it and the
        // next steps back — a closed two-cycle loop.
        let start = Point::new(b.right() - 3, y);
        let pipeline = Pipeline::new(&panels, Algo::Cross, start);
        Stream {
            name: "crossing",
            pipeline,
            cycles: vec![motion(6, 0), motion(-6, 0)],
            expect_crossing: true,
        }
    }

    /// The same interior motion, but every report split across two reads: the
    /// first cycle carries the axes with no SYN (the pump flushes a partial
    /// frame), the second carries only the SYN (completing it). Exercises the
    /// partial-frame path on every single report.
    fn partial_stream(zones: usize) -> Stream {
        let panels = row_panels(zones);
        let b = panels[0].pixels;
        let start = Point::new(b.left() + b.width() / 2, b.top() + b.height() / 2);
        let pipeline = Pipeline::new(&panels, Algo::Cross, start);
        let split = |dx: i32, dy: i32| -> Vec<Vec<InputEvent>> {
            vec![
                vec![
                    rel(RelativeAxisCode::REL_X, dx),
                    rel(RelativeAxisCode::REL_Y, dy),
                ],
                vec![syn()],
            ]
        };
        let mut cycles = Vec::new();
        for (dx, dy) in [(8, 0), (0, 8), (-8, 0), (0, -8)] {
            cycles.extend(split(dx, dy));
        }
        Stream {
            name: "partial",
            pipeline,
            cycles,
            expect_crossing: false,
        }
    }

    /// Interior motion with a keyboard usage riding the same reports — a combined
    /// receiver node whose onboard macro key emits KEY_A while the pointer moves.
    /// Both virtual devices get a frame every cycle. Press and release alternate so
    /// the loop stays closed.
    fn combined_stream(zones: usize) -> Stream {
        let panels = row_panels(zones);
        let b = panels[0].pixels;
        let start = Point::new(b.left() + b.width() / 2, b.top() + b.height() / 2);
        let pipeline = Pipeline::new(&panels, Algo::Cross, start);
        let with_key = |dx: i32, dy: i32, value: i32| -> Vec<InputEvent> {
            vec![
                key(KeyCode::KEY_A, value),
                rel(RelativeAxisCode::REL_X, dx),
                rel(RelativeAxisCode::REL_Y, dy),
                syn(),
            ]
        };
        Stream {
            name: "combined",
            pipeline,
            cycles: vec![
                with_key(8, 0, 1),
                with_key(0, 8, 0),
                with_key(-8, 0, 1),
                with_key(0, -8, 0),
            ],
            expect_crossing: false,
        }
    }

    // --- measurement ----------------------------------------------------------

    struct Row {
        stream: &'static str,
        zones: usize,
        rate_hz: u32,
        ns_per_frame: f64,
        allocs_per_frame: f64,
        bytes_per_frame: f64,
    }

    impl Row {
        /// Reports one core sustains per second: the reciprocal of the latency.
        fn frames_per_s(&self) -> f64 {
            1e9 / self.ns_per_frame
        }

        /// The per-report time as a fraction of this rate's budget. Below 100 %
        /// the pump keeps up with that mouse on one core; the rest is headroom.
        fn budget_pct(&self) -> f64 {
            let budget_ns = 1e9 / self.rate_hz as f64;
            100.0 * self.ns_per_frame / budget_ns
        }
    }

    /// Count allocations over a long run, then time a separate run for the best of
    /// several repetitions. The two runs are separate on purpose (see
    /// `evdev_pump`): the counters are exact whatever the scheduler does, while the
    /// timing wants the minimum, a pump cycle being short enough that every
    /// disturbance can only make it look slower.
    fn measure(stream: &mut Stream, zones: usize, rate_hz: u32, iters: u64, reps: u32) -> Row {
        let cycles_per_iter = stream.cycles_per_iter();

        // Warm-up: settle the buffer capacities and any lazy static.
        for _ in 0..1_000 {
            stream.run();
        }

        support::alloc::reset();
        for _ in 0..iters {
            stream.run();
        }
        let (allocs, bytes) = support::alloc::counts();

        let mut best = f64::MAX;
        for _ in 0..reps {
            let start = Instant::now();
            for _ in 0..iters {
                stream.run();
            }
            let ns = start.elapsed().as_nanos() as f64 / (iters * cycles_per_iter) as f64;
            best = best.min(ns);
        }

        let frames = (iters * cycles_per_iter) as f64;
        Row {
            stream: stream.name,
            zones,
            rate_hz,
            ns_per_frame: best,
            allocs_per_frame: allocs as f64 / frames,
            bytes_per_frame: bytes as f64 / frames,
        }
    }

    pub fn main() {
        let quick = std::env::args().any(|a| a == "--quick");
        let (iters, reps) = if quick { (20_000, 3) } else { (200_000, 7) };

        // Take stdout's one-off buffer allocation before any measurement.
        println!(
            "Linux pipeline — one frame = one report end to end (drain → route → engine → compose).\n\
             ns/frame is latency (machine-dependent); allocs/frame is exact.\n\
             budget %% is ns/frame against the report budget of the rate — below 100%% keeps up on one core.\n"
        );

        // The engine cost dominates and scales with the zone count, so sweep a
        // couple: a laptop-plus-screen and a normal multi-monitor desk. The rate
        // does not change the work per frame — it only sets the budget — so the
        // ns/frame and allocs/frame repeat across rates; the budget column is what
        // moves. Kept explicit so a run is self-describing.
        let builders: [Case; 8] = [
            (interior_stream, 2),
            (interior_stream, 6),
            (crossing_stream, 2),
            (crossing_stream, 6),
            (partial_stream, 2),
            (partial_stream, 6),
            (combined_stream, 2),
            (combined_stream, 6),
        ];

        let mut rows = Vec::new();
        for (build, zones) in builders {
            // Verify once on a fresh stream, then measure the same shape at each
            // rate. The pipeline is stateful, so each rate gets its own instance
            // to keep the closed loop honest.
            {
                let mut s = build(zones);
                s.verify();
            }
            for rate in RATES_HZ {
                let mut s = build(zones);
                rows.push(measure(&mut s, zones, rate, iters, reps));
            }
        }

        let name_w = rows
            .iter()
            .map(|r| r.stream.len())
            .max()
            .unwrap_or(0)
            .max("stream".len());
        println!(
            "{:<name_w$}  {:>5}  {:>6}  {:>10}  {:>12}  {:>8}  {:>11}  {:>10}",
            "stream",
            "zones",
            "rate",
            "ns/frame",
            "frames/s",
            "budget%",
            "allocs/frame",
            "bytes/frame"
        );
        println!(
            "{:-<name_w$}  {:->5}  {:->6}  {:->10}  {:->12}  {:->8}  {:->11}  {:->10}",
            "", "", "", "", "", "", "", ""
        );
        for r in &rows {
            println!(
                "{:<name_w$}  {:>5}  {:>4}Hz  {:>10.1}  {:>12.0}  {:>7.2}%  {:>11.2}  {:>10.1}",
                r.stream,
                r.zones,
                r.rate_hz,
                r.ns_per_frame,
                r.frames_per_s(),
                r.budget_pct(),
                r.allocs_per_frame,
                r.bytes_per_frame,
            );
        }
    }
}
