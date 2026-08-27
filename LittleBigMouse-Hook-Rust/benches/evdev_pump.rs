//! Allocation and timing profile of the Linux evdev pump's per-report path.
//!
//! ```text
//! cargo bench --bench evdev_pump
//! ```
//!
//! What is measured is the *plumbing* of one pump cycle — build the poll set,
//! drain a device's events into a buffer, route each of them to the pointer or
//! the keyboard batch, compose the uinput frames — and nothing else. There is no
//! device, no `/dev/uinput`, no grab and no cursor movement, so this runs
//! anywhere, unprivileged, and never touches the user's mice. The traversal
//! engine's own cost is deliberately excluded: it is measured by `mouse_engine`
//! and `alloc_profile`, and mixing the two would hide the plumbing entirely.
//!
//! Two modes are reported side by side:
//!
//! * **owned** — the current pump: every buffer lives in the `Router` and is
//!   reused cycle after cycle;
//! * **per-cycle** — [`legacy`], a frozen copy of the pump body as it was before
//!   this measurement existed, where each cycle built its poll set, collected
//!   each `fetch_events` drain, and allocated the pending/emitted batches as
//!   locals.
//!
//! The two do the same routing work on the same events, so the difference is the
//! buffers' lifetime and nothing else.
//!
//! Read the columns knowing what each is worth:
//!
//! * **allocs / bytes per cycle** are exact and identical on every machine —
//!   this is the number to diff;
//! * **ns per cycle** is latency, and it is machine-dependent (CPU, governor,
//!   allocator, build flags). It is also slightly pessimistic for the per-cycle
//!   mode, which pays the counting allocator's two relaxed atomics on every
//!   allocation it makes;
//! * **cycles/s** is the same figure as throughput, against the 1000 reports/s a
//!   1 kHz mouse produces per device.

mod support;

#[global_allocator]
static ALLOCATOR: support::alloc::Counting = support::alloc::Counting;

#[cfg(not(target_os = "linux"))]
fn main() {
    println!("evdev_pump: the evdev backend is Linux-only, nothing to measure here.");
}

#[cfg(target_os = "linux")]
fn main() {
    linux::main();
}

#[cfg(target_os = "linux")]
mod linux {
    use std::time::Instant;

    use evdev::{
        AbsoluteAxisCode, EventType, InputEvent, KeyCode, RelativeAxisCode, SynchronizationCode,
    };

    use littlebigmouse_hook::geometry::{Point, Rect};
    use littlebigmouse_hook::hook::linux::evdev::{EvdevCursor, Frame, PumpBuffers};

    use crate::support::alloc;

    /// A plausible session: two grabbed mice and one observed keyboard, so the
    /// poll set the cycle rebuilds has the size a real one has.
    const POLL_FDS: [std::os::fd::RawFd; 3] = [11, 12, 13];

    const DESKTOP: (i32, i32) = (3840, 2160);

    // --- event helpers --------------------------------------------------------

    fn rel(axis: RelativeAxisCode, value: i32) -> InputEvent {
        InputEvent::new(EventType::RELATIVE.0, axis.0, value)
    }

    fn key(code: KeyCode, value: i32) -> InputEvent {
        InputEvent::new(EventType::KEY.0, code.0, value)
    }

    fn scan(value: i32) -> InputEvent {
        InputEvent::new(EventType::MISC.0, 4 /* MSC_SCAN */, value)
    }

    fn syn() -> InputEvent {
        InputEvent::new(
            EventType::SYNCHRONIZATION.0,
            SynchronizationCode::SYN_REPORT.0,
            0,
        )
    }

    // --- scenarios ------------------------------------------------------------

    /// One scenario is one or more pump cycles: the outer `Vec` is what a single
    /// `poll` + drain hands the router, so a report split across two reads is a
    /// two-cycle scenario.
    struct Scenario {
        name: &'static str,
        cycles: Vec<Vec<InputEvent>>,
    }

    fn scenarios() -> Vec<Scenario> {
        let motion = || {
            vec![
                rel(RelativeAxisCode::REL_X, 3),
                rel(RelativeAxisCode::REL_Y, -2),
                syn(),
            ]
        };
        vec![
            // The overwhelmingly common report, and the one that arrives 1000
            // times a second per device.
            Scenario {
                name: "motion",
                cycles: vec![motion()],
            },
            // Scrolling: two extra relative axes ride the pointer frame.
            Scenario {
                name: "motion+wheel",
                cycles: vec![vec![
                    rel(RelativeAxisCode::REL_X, 1),
                    rel(RelativeAxisCode::REL_Y, 0),
                    rel(RelativeAxisCode::REL_WHEEL, 1),
                    rel(RelativeAxisCode::REL_WHEEL_HI_RES, 120),
                    syn(),
                ]],
            },
            // A click: the button rides the pointer frame and updates the held
            // mask the drag detection reads.
            Scenario {
                name: "button",
                cycles: vec![vec![key(KeyCode::BTN_LEFT, 1), syn()]],
            },
            // A macro key off a combined receiver node: scancode to the pointer
            // frame, usage to the virtual keyboard's own frame.
            Scenario {
                name: "keyboard",
                cycles: vec![vec![scan(0x70004), key(KeyCode::KEY_A, 1), syn()]],
            },
            // Both at once — the case the two virtual devices exist for.
            Scenario {
                name: "combined",
                cycles: vec![vec![
                    scan(0x70004),
                    key(KeyCode::KEY_A, 1),
                    rel(RelativeAxisCode::REL_X, 4),
                    key(KeyCode::BTN_RIGHT, 1),
                    rel(RelativeAxisCode::REL_Y, -1),
                    syn(),
                ]],
            },
            // A report split across two reads: the first cycle flushes a partial
            // frame, the second completes the motion.
            Scenario {
                name: "partial",
                cycles: vec![
                    vec![rel(RelativeAxisCode::REL_X, 5)],
                    vec![rel(RelativeAxisCode::REL_Y, 7), syn()],
                ],
            },
            // Eight reports in one drain: what a busy device (or a pump that was
            // late) delivers at once. The per-cycle mode amortises its poll set
            // and its drain over the eight; the batches it cannot.
            Scenario {
                name: "burst/8",
                cycles: vec![std::iter::repeat_with(motion).take(8).flatten().collect()],
            },
        ]
    }

    // --- the current pump -----------------------------------------------------

    /// One cycle with the buffers the `Router` owns. Same order of operations as
    /// `Router::pump`: rebuild the poll set, drain, route, flush on every
    /// `SYN_REPORT`, then flush whatever a partial frame left behind.
    fn owned_cycle(bufs: &mut PumpBuffers, env: &mut EvdevCursor, events: &[InputEvent]) {
        bufs.fill_poll_set(POLL_FDS.into_iter());
        bufs.refill_events(events.iter().copied());
        for k in 0..bufs.event_count() {
            let ev = bufs.event(k);
            if bufs.push(ev, env, 0) == Frame::Complete {
                owned_flush(bufs);
            }
        }
        owned_flush(bufs);
    }

    /// `Router::flush_frame` minus the engine and the two uinput writes: take the
    /// motion, compose the pointer frame, then the keyboard frame.
    fn owned_flush(bufs: &mut PumpBuffers) {
        if !bufs.frame_pending() {
            return;
        }
        std::hint::black_box(bufs.take_motion());
        std::hint::black_box(bufs.pointer_frame(0, 0));
        bufs.take_keyboard_frame(|frame| {
            std::hint::black_box(frame);
        });
    }

    // --- the pump as it was ---------------------------------------------------

    /// The cycle as it was before the buffers moved into the `Router`, kept so
    /// the before/after figures stay reproducible instead of living in a commit
    /// message. It is a frozen copy of the pump body of the time: the poll set,
    /// the `fetch_events` drain, the pending pointer/keyboard batches and the
    /// emitted frame were all locals, allocated and dropped once per cycle.
    ///
    /// It deliberately does *not* update the cursor's button/modifier state: two
    /// field writes that allocate nothing, and leaving them out only makes this
    /// column look better than the code it stands for.
    mod legacy {
        use super::*;

        pub fn cycle(events: &[InputEvent]) {
            let mut fds: Vec<libc::pollfd> = POLL_FDS
                .into_iter()
                .map(|fd| libc::pollfd {
                    fd,
                    events: libc::POLLIN,
                    revents: 0,
                })
                .collect();
            std::hint::black_box(&mut fds);

            let mut acc = (0i64, 0i64);
            let mut passthrough: Vec<InputEvent> = Vec::new();
            let mut kbd: Vec<InputEvent> = Vec::new();
            let dead: Vec<usize> = Vec::new();
            std::hint::black_box(&dead);

            // `fetch_events` was collected into a fresh Vec per readable device:
            // one allocation of exactly the drain's size, which is what `to_vec`
            // does here.
            let drained: Vec<InputEvent> = events.to_vec();
            for ev in drained {
                match ev.event_type() {
                    EventType::SYNCHRONIZATION => flush(&mut acc, &mut passthrough, &mut kbd),
                    EventType::RELATIVE if ev.code() == RelativeAxisCode::REL_X.0 => {
                        acc.0 += ev.value() as i64
                    }
                    EventType::RELATIVE if ev.code() == RelativeAxisCode::REL_Y.0 => {
                        acc.1 += ev.value() as i64
                    }
                    EventType::RELATIVE => passthrough.push(ev),
                    EventType::KEY if (0x100..=0x15f).contains(&ev.code()) => passthrough.push(ev),
                    EventType::KEY => kbd.push(ev),
                    EventType::MISC => passthrough.push(ev),
                    _ => {}
                }
            }
            if acc != (0, 0) || !passthrough.is_empty() || !kbd.is_empty() {
                flush(&mut acc, &mut passthrough, &mut kbd);
            }
        }

        fn flush(
            acc: &mut (i64, i64),
            passthrough: &mut Vec<InputEvent>,
            kbd: &mut Vec<InputEvent>,
        ) {
            if *acc == (0, 0) && passthrough.is_empty() && kbd.is_empty() {
                return;
            }
            *acc = (0, 0);
            emit_absolute(passthrough);
            if !kbd.is_empty() {
                std::hint::black_box(&kbd);
                kbd.clear();
            }
        }

        fn emit_absolute(passthrough: &mut Vec<InputEvent>) {
            let mut batch = vec![
                InputEvent::new(EventType::ABSOLUTE.0, AbsoluteAxisCode::ABS_X.0, 0),
                InputEvent::new(EventType::ABSOLUTE.0, AbsoluteAxisCode::ABS_Y.0, 0),
            ];
            batch.append(passthrough);
            std::hint::black_box(&batch);
        }
    }

    // --- measurement ----------------------------------------------------------

    struct Row {
        scenario: &'static str,
        mode: &'static str,
        allocs_per_cycle: f64,
        bytes_per_cycle: f64,
        ns_per_cycle: f64,
    }

    /// Warm up, count allocations over a long run, then time a separate run.
    ///
    /// The two runs are separate on purpose: the counters are exact whatever the
    /// scheduler does, while the timing wants the best of several repetitions.
    /// The minimum of the per-repetition means is the estimator here — a pump
    /// cycle is short, and every disturbance can only make it look slower.
    fn measure(
        scenario: &'static str,
        mode: &'static str,
        cycles_per_iter: u64,
        mut f: impl FnMut(),
    ) -> Row {
        const WARMUP: u64 = 1_000;
        const ITERS: u64 = 200_000;
        const REPS: u32 = 7;

        for _ in 0..WARMUP {
            f();
        }

        alloc::reset();
        for _ in 0..ITERS {
            f();
        }
        let (allocs, bytes) = alloc::counts();

        let mut best = f64::MAX;
        for _ in 0..REPS {
            let start = Instant::now();
            for _ in 0..ITERS {
                f();
            }
            let ns = start.elapsed().as_nanos() as f64 / (ITERS * cycles_per_iter) as f64;
            best = best.min(ns);
        }

        let cycles = (ITERS * cycles_per_iter) as f64;
        Row {
            scenario,
            mode,
            allocs_per_cycle: allocs as f64 / cycles,
            bytes_per_cycle: bytes as f64 / cycles,
            ns_per_cycle: best,
        }
    }

    pub fn main() {
        // Take stdout's one-off buffer allocation before any measurement.
        println!(
            "evdev pump — one cycle = one poll + drain + route + compose.\n\
             owned = buffers owned by the Router, per-cycle = the pump as it was.\n"
        );

        let desktop = Rect::new(0, 0, DESKTOP.0, DESKTOP.1);
        let mut rows = Vec::new();

        for scenario in scenarios() {
            let cycles_per_iter = scenario.cycles.len() as u64;

            let mut bufs = PumpBuffers::new();
            let mut env = EvdevCursor::new(desktop, Point::new(0, 0));
            rows.push(measure(scenario.name, "owned", cycles_per_iter, || {
                for cycle in &scenario.cycles {
                    owned_cycle(&mut bufs, &mut env, cycle);
                }
            }));

            rows.push(measure(scenario.name, "per-cycle", cycles_per_iter, || {
                for cycle in &scenario.cycles {
                    legacy::cycle(cycle);
                }
            }));
        }

        let width = rows.iter().map(|r| r.scenario.len()).max().unwrap_or(0);
        println!(
            "{:<width$}  {:<10}  {:>11}  {:>10}  {:>10}  {:>12}",
            "scenario", "mode", "allocs/cyc", "bytes/cyc", "ns/cyc", "cycles/s"
        );
        println!(
            "{:-<width$}  {:-<10}  {:->11}  {:->10}  {:->10}  {:->12}",
            "", "", "", "", "", ""
        );
        for row in &rows {
            println!(
                "{:<width$}  {:<10}  {:>11.2}  {:>10.1}  {:>10.1}  {:>12.0}",
                row.scenario,
                row.mode,
                row.allocs_per_cycle,
                row.bytes_per_cycle,
                row.ns_per_cycle,
                1e9 / row.ns_per_cycle,
            );
        }
    }
}
