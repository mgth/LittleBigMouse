//! Allocation profile for the same scenarios `mouse_engine` times.
//!
//! ```text
//! cargo bench --bench alloc_profile
//! ```
//!
//! Unlike the timings, these figures are *not* machine-dependent: they are exact
//! counts, identical on any host for a given build of the code. That makes them
//! the more useful half of the baseline — a change that starts allocating on the
//! per-event path shows up here as a whole number, with no statistics to argue
//! about.
//!
//! Why a plain binary instead of a criterion harness: criterion's own sampling,
//! reporting and `Vec` bookkeeping allocate heavily, and separating its
//! allocations from the code under test is more machinery than the answer is
//! worth. A counting [`GlobalAlloc`] over a bare loop gives exact numbers in
//! twenty lines. It is registered for this bench binary only, so nothing about
//! the daemon's allocator changes.
//!
//! No assertion, no threshold: this records a baseline, and thresholds on
//! allocation counts would fail on an unrelated `std` change. Read the numbers,
//! compare them to the README table, investigate a difference.

mod support;

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicU64, Ordering};

use littlebigmouse_hook::geometry::{Point, Rect, Segment};
use littlebigmouse_hook::zones::travel::travel;
use littlebigmouse_hook::zones::ZonesLayout;
use support::{
    complex_intersection, crossing, grid_panels, interior, layout_xml, no_target, row_panels, Algo,
    Scenario,
};

// --- counting allocator ------------------------------------------------------

static ALLOCS: AtomicU64 = AtomicU64::new(0);
static BYTES: AtomicU64 = AtomicU64::new(0);

struct Counting;

/// Delegates everything to the system allocator, counting on the way through.
/// `realloc` and `alloc_zeroed` are forwarded rather than left to the trait's
/// default implementations so the allocator behaves exactly like the real one —
/// the defaults would turn every `realloc` into an alloc/copy/free.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        BYTES.fetch_add(layout.size() as u64, Ordering::Relaxed);
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        BYTES.fetch_add(layout.size() as u64, Ordering::Relaxed);
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        BYTES.fetch_add(
            new_size.saturating_sub(layout.size()) as u64,
            Ordering::Relaxed,
        );
        unsafe { System.realloc(ptr, layout, new_size) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

// --- measurement -------------------------------------------------------------

struct Row {
    label: String,
    /// Unit of work the counts are divided by — one mouse event, or one call.
    unit: &'static str,
    allocs_per_unit: f64,
    bytes_per_unit: f64,
}

/// Run `f` `iters` times and attribute the allocations to `units_per_iter`
/// units of work. The warm-up lap matters: lazily initialised statics (the
/// travel cache's hash state, stdout's buffer) allocate exactly once and would
/// otherwise be charged to the first measured iteration.
fn measure(
    label: impl Into<String>,
    unit: &'static str,
    iters: u64,
    units_per_iter: u64,
    mut f: impl FnMut(),
) -> Row {
    for _ in 0..16 {
        f();
    }

    ALLOCS.store(0, Ordering::Relaxed);
    BYTES.store(0, Ordering::Relaxed);
    for _ in 0..iters {
        f();
    }
    let allocs = ALLOCS.load(Ordering::Relaxed);
    let bytes = BYTES.load(Ordering::Relaxed);

    let units = (iters * units_per_iter) as f64;
    Row {
        label: label.into(),
        unit,
        allocs_per_unit: allocs as f64 / units,
        bytes_per_unit: bytes as f64 / units,
    }
}

/// A verified scenario, measured per mouse event.
fn measure_scenario(label: impl Into<String>, mut scenario: Scenario, iters: u64) -> Row {
    scenario.verify();
    let units = scenario.path.len() as u64;
    measure(label, "event", iters, units, || scenario.run())
}

const ZONE_COUNTS: [usize; 3] = [2, 6, 16];
const ITERS: u64 = 2_000;

fn main() {
    // Take stdout's one-off buffer allocation before any measurement.
    println!("Allocation profile — exact counts, identical on every machine.\n");

    let mut rows = Vec::new();

    for algo in [Algo::Strait, Algo::Cross] {
        rows.push(measure_scenario(
            format!("interior/{algo:?}"),
            interior(algo, 2),
            ITERS,
        ));
    }
    for algo in [Algo::Strait, Algo::Cross] {
        for zones in ZONE_COUNTS {
            rows.push(measure_scenario(
                format!("crossing/{algo:?}/{zones}"),
                crossing(algo, zones),
                ITERS,
            ));
        }
    }
    for algo in [Algo::Strait, Algo::Cross] {
        for zones in ZONE_COUNTS {
            rows.push(measure_scenario(
                format!("no_target/{algo:?}/{zones}"),
                no_target(algo, zones),
                ITERS,
            ));
        }
    }
    rows.push(measure_scenario(
        "intersection/cross_4x4_diagonal",
        complex_intersection(),
        ITERS,
    ));

    // The geometry primitive the Cross scan calls once per zone.
    let rect: Rect<f64> = Rect::new(0.0, 0.0, 480.0, 270.0);
    let diagonal = Segment::new(Point::new(-100.0, -50.0), Point::new(600.0, 400.0)).line();
    let vertical = Segment::new(Point::new(240.0, -50.0), Point::new(240.0, 400.0)).line();
    for (name, line) in [
        ("rect_intersect_line/diagonal", diagonal),
        ("rect_intersect_line/vertical", vertical),
    ] {
        rows.push(measure(name, "call", ITERS, 1, || {
            std::hint::black_box(rect.intersect(std::hint::black_box(&line)));
        }));
    }

    // travel_pixels, cached and uncached.
    for zones in ZONE_COUNTS {
        let panels = row_panels(zones);
        let xml = layout_xml(&panels, Algo::Strait, 200.0, 0.0);
        let mut layout = ZonesLayout::from_xml(&xml).expect("generated layout must parse");
        let (first, last) = (layout.zones[0], layout.zones[zones - 1]);
        layout.travel_pixels(first, last);
        rows.push(measure(
            format!("travel_pixels/cached/{zones}"),
            "call",
            ITERS,
            1,
            || {
                std::hint::black_box(layout.travel_pixels(first, last));
            },
        ));
    }
    for (name, panels) in [
        ("travel_pixels/compute/row_16", row_panels(16)),
        ("travel_pixels/compute/grid_4x4", grid_panels(4, 4)),
    ] {
        let bounds: Vec<Rect<i32>> = panels.iter().map(|p| p.pixels).collect();
        let (source, target) = (bounds[0], bounds[bounds.len() - 1]);
        rows.push(measure(name, "call", ITERS, 1, || {
            std::hint::black_box(travel(source, target, &bounds));
        }));
    }

    // Layout load, for scale: this is what a reconfiguration costs.
    for zones in ZONE_COUNTS {
        let xml = layout_xml(&row_panels(zones), Algo::Strait, 200.0, 0.0);
        rows.push(measure(
            format!("load_layout/{zones}"),
            "call",
            200,
            1,
            || {
                std::hint::black_box(ZonesLayout::from_xml(&xml));
            },
        ));
    }

    let width = rows.iter().map(|r| r.label.len()).max().unwrap_or(0);
    println!(
        "{:<width$}  {:>12}  {:>12}",
        "scenario", "allocs/unit", "bytes/unit"
    );
    println!("{:-<width$}  {:->12}  {:->12}", "", "", "");
    for row in &rows {
        println!(
            "{:<width$}  {:>12.2}  {:>12.1}   per {}",
            row.label, row.allocs_per_unit, row.bytes_per_unit, row.unit
        );
    }
}
