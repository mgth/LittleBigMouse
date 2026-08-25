//! Timing baseline for the mouse traversal engine.
//!
//! Everything here runs against the fake cursor of `support::BenchCursor`: no
//! hook is installed, no device is opened, the real pointer never moves. What is
//! measured is `MouseEngine::on_mouse_move` and the zone/geometry code it calls
//! — the part of the daemon that runs on every single mouse report, up to 1000
//! times per second per device.
//!
//! ```text
//! cargo bench --bench mouse_engine              # full run (a few minutes)
//! cargo bench --bench mouse_engine -- --quick   # short mode, good enough to compare
//! cargo bench --bench mouse_engine -- --test    # just run each scenario once
//! cargo bench --bench mouse_engine -- crossing  # filter by name
//! ```
//!
//! Absolute numbers are meaningless across machines — they depend on the CPU,
//! its clock governor, the allocator and the build flags. Compare runs made on
//! the same machine, ideally back to back; criterion's own
//! `target/criterion/<id>/` baselines do this for you. See the README for the
//! recorded reference environment.
//!
//! Every scenario is a *closed loop* of events (replaying it returns the engine
//! to its starting state) and is checked by [`Scenario::verify`] before being
//! timed, so a benchmark cannot silently decay into measuring an early return.

mod support;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use littlebigmouse_hook::geometry::{Point, Rect, Segment};
use littlebigmouse_hook::zones::travel::travel;
use littlebigmouse_hook::zones::ZonesLayout;
use support::{
    complex_intersection, crossing, desktop_bounds, grid_panels, interior, layout_xml, no_target,
    row_panels, Algo, Scenario,
};

/// Zone counts to sweep: a laptop plus one screen, a normal multi-monitor desk,
/// and a wall large enough to make an O(zones) scan visible.
const ZONE_COUNTS: [usize; 3] = [2, 6, 16];

/// Time a verified scenario, reporting per-event throughput.
fn bench_scenario(
    group: &mut criterion::BenchmarkGroup<'_, criterion::measurement::WallTime>,
    id: BenchmarkId,
    mut scenario: Scenario,
) {
    scenario.verify();
    group.throughput(Throughput::Elements(scenario.path.len() as u64));
    group.bench_function(id, |b| b.iter(|| scenario.run()));
}

/// The common case: the pointer moves inside the monitor it is already on. The
/// engine must decide "nothing to do" as cheaply as possible.
fn bench_interior(c: &mut Criterion) {
    let mut group = c.benchmark_group("interior");
    for algo in [Algo::Strait, Algo::Cross] {
        bench_scenario(
            &mut group,
            BenchmarkId::from_parameter(format!("{algo:?}")),
            interior(algo, 2),
        );
    }
    group.finish();
}

/// A border crossing, swept over the zone count. The interesting result is the
/// *shape*: Strait resolves a link on one side and is flat in the zone count,
/// while Cross scans every zone for the nearest exit and is not.
fn bench_crossing(c: &mut Criterion) {
    let mut group = c.benchmark_group("crossing");
    for algo in [Algo::Strait, Algo::Cross] {
        for zones in ZONE_COUNTS {
            bench_scenario(
                &mut group,
                BenchmarkId::new(format!("{algo:?}"), zones),
                crossing(algo, zones),
            );
        }
    }
    group.finish();
}

/// Pushing at the outer rim of the desktop. No zone can match, so the search
/// cannot exit early — the worst case of the scan, and the one a user produces
/// continuously just by shoving the pointer into a corner.
fn bench_no_target(c: &mut Criterion) {
    let mut group = c.benchmark_group("no_target");
    for algo in [Algo::Strait, Algo::Cross] {
        for zones in ZONE_COUNTS {
            bench_scenario(
                &mut group,
                BenchmarkId::new(format!("{algo:?}"), zones),
                no_target(algo, zones),
            );
        }
    }
    group.finish();
}

/// A long diagonal trip across a 4x4 wall: many zone rectangles actually
/// intersect the trip line, so the nearest-exit comparison runs on all of them.
fn bench_intersection(c: &mut Criterion) {
    let mut group = c.benchmark_group("intersection");
    bench_scenario(
        &mut group,
        BenchmarkId::from_parameter("cross_4x4_diagonal"),
        complex_intersection(),
    );
    group.finish();
}

/// The geometry primitive underneath the scan: `Rect::intersect(&Line)`, called
/// once per zone per Cross event. It returns a freshly allocated `Vec`, which is
/// why it also shows up in the allocation profile.
fn bench_rect_intersect(c: &mut Criterion) {
    let rect: Rect<f64> = Rect::new(0.0, 0.0, 480.0, 270.0);
    // A diagonal that genuinely cuts two edges, and a vertical one (the
    // `f64::MAX` slope sentinel), which takes a different branch.
    let diagonal = Segment::new(Point::new(-100.0, -50.0), Point::new(600.0, 400.0)).line();
    let vertical = Segment::new(Point::new(240.0, -50.0), Point::new(240.0, 400.0)).line();

    let mut group = c.benchmark_group("rect_intersect_line");
    group.bench_function("diagonal", |b| {
        b.iter(|| std::hint::black_box(rect.intersect(std::hint::black_box(&diagonal))))
    });
    group.bench_function("vertical", |b| {
        b.iter(|| std::hint::black_box(rect.intersect(std::hint::black_box(&vertical))))
    });
    group.finish();
}

/// `travel_pixels` — the clip-rect path the cursor is walked along so it cannot
/// escape the desktop between two monitors.
///
/// Two very different costs share the name: the cached lookup the engine
/// actually pays per crossing (a hash lookup plus a `Vec` clone), and the
/// backtracking search behind the cache, benchmarked directly through
/// `zones::travel::travel` because `ZonesLayout` has no way to drop its cache.
fn bench_travel(c: &mut Criterion) {
    let mut group = c.benchmark_group("travel_pixels");

    for zones in ZONE_COUNTS {
        let panels = row_panels(zones);
        let xml = layout_xml(&panels, Algo::Strait, 200.0, 0.0);
        let mut layout = ZonesLayout::from_xml(&xml).expect("generated layout must parse");
        let (first, last) = (layout.zones[0], layout.zones[zones - 1]);
        // Warm the cache so the loop measures the lookup, not the search.
        let warm = layout.travel_pixels(first, last);
        assert!(
            !warm.is_empty(),
            "adjacent monitors must have a travel path"
        );

        group.bench_function(BenchmarkId::new("cached", zones), |b| {
            b.iter(|| std::hint::black_box(layout.travel_pixels(first, last)))
        });
    }

    // Uncached search. A row is the easy shape (the two monitors are directly
    // reachable, so it returns at once); the grid forces the recursion.
    for (name, panels) in [
        ("compute/row_16", row_panels(16)),
        ("compute/grid_4x4", grid_panels(4, 4)),
    ] {
        let bounds: Vec<Rect<i32>> = panels.iter().map(|p| p.pixels).collect();
        let source = bounds[0];
        let target = bounds[bounds.len() - 1];
        assert!(
            !travel(source, target, &bounds).is_empty(),
            "{name}: the fixture must have a travel path to measure"
        );
        group.bench_function(name, |b| {
            b.iter(|| {
                std::hint::black_box(travel(
                    std::hint::black_box(source),
                    std::hint::black_box(target),
                    std::hint::black_box(&bounds),
                ))
            })
        });
    }

    group.finish();
}

/// Layout load. Not a hot path — it runs once per reconfiguration — but a 16
/// monitor layout is a ~40 kB document and the daemon parses it while the
/// pointer is live, so it is worth a number.
fn bench_load(c: &mut Criterion) {
    let mut group = c.benchmark_group("load_layout");
    for zones in ZONE_COUNTS {
        let panels = row_panels(zones);
        let xml = layout_xml(&panels, Algo::Strait, 200.0, 0.0);
        group.throughput(Throughput::Bytes(xml.len() as u64));
        group.bench_function(BenchmarkId::from_parameter(zones), |b| {
            b.iter(|| std::hint::black_box(ZonesLayout::from_xml(std::hint::black_box(&xml))))
        });
    }
    group.finish();
}

/// Not timed: prints the shape of the fixtures so a run is self-describing and
/// a surprising number can be traced back to what was actually measured.
fn describe(_c: &mut Criterion) {
    for zones in ZONE_COUNTS {
        let panels = row_panels(zones);
        let xml = layout_xml(&panels, Algo::Strait, 200.0, 0.0);
        let desktop = desktop_bounds(&panels);
        eprintln!(
            "fixture row_{zones}: desktop {}x{} px, layout xml {} bytes",
            desktop.width(),
            desktop.height(),
            xml.len(),
        );
    }
}

criterion_group!(
    benches,
    describe,
    bench_interior,
    bench_crossing,
    bench_no_target,
    bench_intersection,
    bench_rect_intersect,
    bench_travel,
    bench_load,
);
criterion_main!(benches);
