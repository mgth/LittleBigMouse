//! `CompactionSolverTests.cs`: the placement geometry of `ForceCompact`, tested
//! directly on the solver — immutable rects in, translations out.

mod common;

use common::{assert_equal_precision, round_digits};
use lbm_layout::geo::dotnet::{max, min};
use lbm_layout::geo::Rect;
use lbm_layout::solve::compaction::{self, CompactionMonitor, CompactionOptions, CONTACT_EPSILON};
use lbm_layout::solve::distance::RectDistance;
use lbm_layout::solve::IdMap;

/// Suite-wide corridor requirement, mirroring the option's default.
const REQ: f64 = 20.0;

const COMPACTING: CompactionOptions = CompactionOptions::new(false, false, REQ);

/// `M(id, x, y, w, h, primary, bezel: 0)`: zero bezel, so panel and outside bounds
/// coincide.
fn m(id: &str, x: f64, y: f64, w: f64, h: f64, primary: bool) -> CompactionMonitor {
    m_bezel(id, x, y, w, h, primary, 0.0)
}

/// `M(id, x, y, w, h, primary, bezel)`.
fn m_bezel(
    id: &str,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    primary: bool,
    bezel: f64,
) -> CompactionMonitor {
    CompactionMonitor::new(
        id,
        Rect::new(x, y, w, h),
        Rect::new(x - bezel, y - bezel, w + 2.0 * bezel, h + 2.0 * bezel),
        primary,
    )
}

/// Apply the solved translations, so assertions read as final panel positions.
fn place(monitors: &[CompactionMonitor], options: CompactionOptions) -> IdMap<Rect> {
    let offsets = compaction::solve(monitors, options);
    let mut placed = IdMap::new();
    for m in monitors {
        let rect = match offsets.get(&m.id) {
            Some(d) => Rect::new(
                m.bounds.x() + d.x,
                m.bounds.y() + d.y,
                m.bounds.width(),
                m.bounds.height(),
            ),
            None => m.bounds,
        };
        placed.insert(&m.id, rect);
    }
    placed
}

fn overlaps(a: &Rect, b: &Rect) -> bool {
    min(a.right(), b.right()) - max(a.x(), b.x()) > CONTACT_EPSILON
        && min(a.bottom(), b.bottom()) - max(a.y(), b.y()) > CONTACT_EPSILON
}

/// Length of display surface the two share along their contact, mm.
fn corridor(a: &Rect, b: &Rect) -> f64 {
    max(
        min(a.right(), b.right()) - max(a.x(), b.x()),
        min(a.bottom(), b.bottom()) - max(a.y(), b.y()),
    )
}

/// Mirrors `CompactionSolver.AreConnected` on zero-bezel rects.
fn touches(a: &Rect, b: &Rect, req: f64) -> bool {
    let d = a.distance(b);

    if max(d.left, d.right) > CONTACT_EPSILON || max(d.top, d.bottom) > CONTACT_EPSILON {
        return false;
    }

    req <= 0.0 || corridor(a, b) >= req - CONTACT_EPSILON
}

/// True when every monitor is reachable from every other through contacts.
fn is_connected(rects: &[Rect], req: f64) -> bool {
    if rects.is_empty() {
        return true;
    }
    let mut seen = vec![rects[0]];
    let mut todo: Vec<Rect> = rects[1..].to_vec();

    let mut grown = true;
    while grown {
        grown = false;
        for i in (0..todo.len()).rev() {
            if !seen.iter().any(|s| touches(s, &todo[i], req)) {
                continue;
            }
            seen.push(todo[i]);
            todo.remove(i);
            grown = true;
        }
    }
    todo.is_empty()
}

fn values(placed: &IdMap<Rect>) -> Vec<Rect> {
    placed.values().copied().collect()
}

/// Ordinal order of the ids, positions rounded to 6 decimals.
fn signature(placed: &IdMap<Rect>) -> String {
    let mut entries: Vec<(&str, &Rect)> = placed.iter().collect();
    entries.sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));
    entries
        .iter()
        .map(|(k, v)| format!("{k}={},{}", round_digits(v.x(), 6), round_digits(v.y(), 6)))
        .collect::<Vec<_>>()
        .join(" ")
}

fn with_bounds(monitors: &[CompactionMonitor], placed: &IdMap<Rect>) -> Vec<CompactionMonitor> {
    monitors
        .iter()
        .map(|m| CompactionMonitor {
            bounds: placed[m.id.as_str()],
            outside_bounds: placed[m.id.as_str()],
            ..m.clone()
        })
        .collect()
}

// ---------------------------------------------------------------- overlaps

#[test]
fn overlap_is_resolved_and_primary_never_moves() {
    let monitors = [
        m("A", 0.0, 0.0, 700.0, 400.0, true),
        m("B", 300.0, 100.0, 700.0, 400.0, false),
    ];

    let placed = place(&monitors, COMPACTING);

    assert_eq!(Rect::new(0.0, 0.0, 700.0, 400.0), placed["A"]);
    assert!(!overlaps(&placed["A"], &placed["B"]));
    assert!(touches(&placed["A"], &placed["B"], REQ));
}

#[test]
fn overlap_shortest_free_push_wins() {
    // B sits 100mm into A's right edge: the shortest way out is 100mm to the right,
    // not 600mm back to the left.
    let monitors = [
        m("A", 0.0, 0.0, 700.0, 400.0, true),
        m("B", 600.0, 0.0, 700.0, 400.0, false),
    ];

    let placed = place(&monitors, COMPACTING);

    assert_equal_precision(700.0, placed["B"].x(), 6);
    assert_equal_precision(0.0, placed["B"].y(), 6);
}

#[test]
fn exactly_stacked_monitors_are_separated() {
    // Fully coincident rects: degenerate input must still terminate and separate.
    let monitors = [
        m("A", 0.0, 0.0, 700.0, 400.0, true),
        m("B", 0.0, 0.0, 700.0, 400.0, false),
        m("C", 0.0, 0.0, 700.0, 400.0, false),
    ];

    let placed = place(&monitors, COMPACTING);

    assert_eq!(Rect::new(0.0, 0.0, 700.0, 400.0), placed["A"]);
    for (x, y) in [("A", "B"), ("A", "C"), ("B", "C")] {
        assert!(!overlaps(&placed[x], &placed[y]), "{x}/{y} still overlap");
    }
}

#[test]
fn allow_overlaps_leaves_overlaps_alone() {
    let monitors = [
        m("A", 0.0, 0.0, 700.0, 400.0, true),
        m("B", 300.0, 100.0, 700.0, 400.0, false),
    ];

    let placed = place(&monitors, CompactionOptions::new(true, false, REQ));

    // Already one cluster (they touch), so the compaction phase has nothing to pull.
    assert_eq!(Rect::new(300.0, 100.0, 700.0, 400.0), placed["B"]);
}

// ---------------------------------------------------------------- clusters

#[test]
fn disjoint_clusters_are_pulled_into_contact() {
    let monitors = [
        m("A", 0.0, 0.0, 700.0, 400.0, true),
        m("B", 2000.0, 0.0, 700.0, 400.0, false),
    ];

    let placed = place(&monitors, COMPACTING);

    assert_equal_precision(700.0, placed["B"].x(), 6);
    assert_equal_precision(0.0, placed["B"].y(), 6);
}

#[test]
fn cluster_travels_as_one_rigid_block() {
    // {B, C} touch each other but are far from the primary: they must arrive
    // together, keeping their 200mm relative offset.
    let monitors = [
        m("A", 0.0, 0.0, 700.0, 400.0, true),
        m("B", 1500.0, -300.0, 1650.0, 920.0, false),
        m("C", 1700.0, -700.0, 700.0, 400.0, false),
    ];

    let placed = place(&monitors, COMPACTING);

    let db = placed["B"].x() - 1500.0;
    let dc = placed["C"].x() - 1700.0;
    assert_equal_precision(db, dc, 6);
    assert_equal_precision(placed["B"].y() - -300.0, placed["C"].y() - -700.0, 6);
    assert_equal_precision(700.0, placed["B"].x(), 6);
}

#[test]
fn three_disjoint_islands_all_end_up_connected() {
    let monitors = [
        m("A", 0.0, 0.0, 700.0, 400.0, true),
        m("B", 5000.0, 0.0, 700.0, 400.0, false),
        m("C", -5000.0, 3000.0, 700.0, 400.0, false),
        m("D", 9000.0, -4000.0, 700.0, 400.0, false),
    ];

    let placed = place(&monitors, COMPACTING);

    assert!(
        is_connected(&values(&placed), REQ),
        "layout still disjoint: {}",
        signature(&placed)
    );
    assert_eq!(Rect::new(0.0, 0.0, 700.0, 400.0), placed["A"]);
}

#[test]
fn nearest_cluster_is_pulled_first() {
    // The far TV comes first in the input; the near neighbour must still take the
    // primary's right edge, with the TV landing beyond it (issue #450).
    let monitors = [
        m("C_tv", 1454.0, -294.0, 1650.0, 920.0, false),
        m("A_primary", 0.0, 0.0, 700.0, 400.0, true),
        m("B_near", 754.0, 16.0, 700.0, 400.0, false),
    ];

    let placed = place(&monitors, COMPACTING);

    assert_equal_precision(700.0, placed["B_near"].x(), 6);
    assert_equal_precision(16.0, placed["B_near"].y(), 6);
    assert_equal_precision(1400.0, placed["C_tv"].x(), 6);
    assert_equal_precision(-294.0, placed["C_tv"].y(), 6);
}

// ------------------------------------------------------------- compaction

#[test]
fn already_compact_layout_is_left_untouched() {
    let monitors = [
        m("A", 0.0, 0.0, 700.0, 400.0, true),
        m("B", 700.0, 0.0, 700.0, 400.0, false),
        m("C", 1400.0, 0.0, 700.0, 400.0, false),
    ];

    assert!(compaction::solve(&monitors, COMPACTING).is_empty());
}

#[test]
fn compaction_is_idempotent() {
    let monitors = [
        m("A", 0.0, 0.0, 700.0, 400.0, true),
        m("B", 1200.0, 250.0, 600.0, 340.0, false),
        m("C", -900.0, -700.0, 800.0, 350.0, false),
        m("D", 2500.0, 1500.0, 1650.0, 920.0, false),
    ];

    let placed = place(&monitors, COMPACTING);
    let again = with_bounds(&monitors, &placed);

    assert!(compaction::solve(&again, COMPACTING).is_empty());
}

#[test]
fn allow_discontinuity_resolves_overlaps_but_keeps_gaps() {
    let monitors = [
        m("A", 0.0, 0.0, 700.0, 400.0, true),
        m("B", 300.0, 100.0, 700.0, 400.0, false), // overlapping
        m("C", 5000.0, 0.0, 700.0, 400.0, false),  // far away
    ];

    let placed = place(&monitors, CompactionOptions::new(false, true, REQ));

    assert!(!overlaps(&placed["A"], &placed["B"]));
    assert_eq!(Rect::new(5000.0, 0.0, 700.0, 400.0), placed["C"]);
}

// ------------------------------------------------------------ degenerate

#[test]
fn no_primary_produces_no_offsets() {
    let monitors = [
        m("A", 0.0, 0.0, 700.0, 400.0, false),
        m("B", 5000.0, 0.0, 700.0, 400.0, false),
    ];

    assert!(compaction::solve(&monitors, COMPACTING).is_empty());
}

#[test]
fn single_monitor_produces_no_offsets() {
    assert!(compaction::solve(&[m("A", 90.0, 90.0, 700.0, 400.0, true)], COMPACTING).is_empty());
}

#[test]
fn primary_is_never_translated() {
    let monitors = [
        m("A", 123.0, -456.0, 700.0, 400.0, true),
        m("B", 4000.0, 4000.0, 700.0, 400.0, false),
        m("C", 200.0, -300.0, 700.0, 400.0, false),
    ];

    assert!(!compaction::solve(&monitors, COMPACTING).contains_key("A"));
}

// ----------------------------------------------- determinism / permutation

/// Every ordering of the same monitors must compact identically.
#[test]
fn input_order_permutation_does_not_change_result() {
    let monitors = [
        m("A", 0.0, 0.0, 700.0, 400.0, true),
        m("B", 900.0, 30.0, 600.0, 340.0, false),
        m("C", -1200.0, -500.0, 1650.0, 920.0, false),
        m("D", 850.0, 900.0, 800.0, 350.0, false),
        m("E", 2400.0, -1400.0, 340.0, 600.0, false),
    ];

    let expected = signature(&place(&monitors, COMPACTING));
    let mut count = 0;

    for order in permutations(monitors.len()) {
        let permuted: Vec<CompactionMonitor> = order.iter().map(|&i| monitors[i].clone()).collect();
        assert_eq!(expected, signature(&place(&permuted, COMPACTING)));
        count += 1;
    }

    assert_eq!(120, count);
}

/// 200 generated layouts — touching, gapped, overlapping, disjoint — as
/// (case, monitors, primary index).
fn generated_layouts() -> Vec<(usize, Vec<CompactionMonitor>, usize)> {
    let mut rng = Lcg::new(0xC0FFEE);
    let panels: [(f64, f64); 6] = [
        (700.0, 400.0),
        (600.0, 340.0),
        (1650.0, 920.0),
        (520.0, 330.0),
        (800.0, 350.0),
        (340.0, 600.0),
    ];

    let mut layouts = Vec::new();
    for c in 0..200 {
        let count = 2 + rng.next(5) as usize;
        let mut monitors = Vec::new();
        let primary = rng.next(count as i32) as usize;

        for j in 0..count {
            let (w, h) = panels[rng.next(panels.len() as i32) as usize];
            let x = (rng.next(9) - 4) * 500 + (rng.next(21) - 10) * 20;
            let y = (rng.next(7) - 3) * 450 + (rng.next(21) - 10) * 20;
            monitors.push(m(&format!("M{j}"), x as f64, y as f64, w, h, j == primary));
        }

        layouts.push((c, monitors, primary));
    }
    layouts
}

/// The result does not depend on input order, and the primary never moves.
#[test]
fn generated_layouts_are_order_invariant_and_keep_primary_anchored() {
    for (_c, monitors, primary) in generated_layouts() {
        let placed = place(&monitors, COMPACTING);
        let reference = signature(&placed);

        // A few rotations plus the reverse catch encounter-order leaks.
        for r in 1..monitors.len() {
            let rotated: Vec<CompactionMonitor> = monitors[r..]
                .iter()
                .chain(&monitors[..r])
                .cloned()
                .collect();
            assert_eq!(reference, signature(&place(&rotated, COMPACTING)));
        }
        let reversed: Vec<CompactionMonitor> = monitors.iter().rev().cloned().collect();
        assert_eq!(reference, signature(&place(&reversed, COMPACTING)));

        assert_eq!(
            monitors[primary].outside_bounds,
            placed[format!("M{primary}").as_str()]
        );
    }
}

/// `[Theory]` over `req`: nothing overlaps, everything forms one block connected at
/// that requirement, and compacting the result changes nothing.
fn generated_layouts_end_up_overlap_free_and_connected(req: f64) {
    let options = CompactionOptions {
        minimal_edge_overlap: req,
        ..COMPACTING
    };
    let mut total = 0;

    for (c, monitors, _) in generated_layouts() {
        let placed = place(&monitors, options);
        let rects = values(&placed);
        total += 1;

        for i in 0..rects.len() {
            for j in i + 1..rects.len() {
                assert!(
                    !overlaps(&rects[i], &rects[j]),
                    "case {c}: overlap left in {}",
                    signature(&placed)
                );
            }
        }

        assert!(
            is_connected(&rects, req),
            "case {c}: still disjoint: {}",
            signature(&placed)
        );

        // Settling must reach a fixed point: compacting the result changes nothing.
        let again = with_bounds(&monitors, &placed);
        assert!(compaction::solve(&again, options).is_empty());
    }

    assert_eq!(200, total);
}

#[test]
fn generated_layouts_end_up_overlap_free_and_connected_0() {
    generated_layouts_end_up_overlap_free_and_connected(0.0);
}

#[test]
fn generated_layouts_end_up_overlap_free_and_connected_20() {
    generated_layouts_end_up_overlap_free_and_connected(20.0);
}

#[test]
fn generated_layouts_end_up_overlap_free_and_connected_40() {
    generated_layouts_end_up_overlap_free_and_connected(40.0);
}

/// A cluster pulled against the monitors anchored so far can land on one still
/// queued; the settle passes repair it. Generated corpus, case 40, reduced.
#[test]
fn pulled_cluster_landing_on_a_queued_one_is_repaired() {
    let monitors = [
        m("M0", 1100.0, -880.0, 520.0, 330.0, true),
        m("M1", -520.0, 200.0, 700.0, 400.0, false),
        m("M2", 120.0, 960.0, 600.0, 340.0, false),
        m("M3", 900.0, 1060.0, 800.0, 350.0, false),
        m("M4", -2060.0, -310.0, 700.0, 400.0, false),
    ];

    let placed = place(&monitors, COMPACTING);

    assert!(
        !overlaps(&placed["M1"], &placed["M2"]),
        "M1/M2 still overlap: {}",
        signature(&placed)
    );
    assert!(
        is_connected(&values(&placed), REQ),
        "{}",
        signature(&placed)
    );
}

/// When no single-axis translation can reach the anchored group, the cluster must
/// still land on a real edge. Generated corpus, case 1.
#[test]
fn diagonal_pull_lands_in_contact_not_between_two_monitors() {
    let monitors = [
        m("M0", 380.0, -1040.0, 340.0, 600.0, false),
        m("M1", -620.0, -720.0, 340.0, 600.0, true),
        m("M2", 800.0, 610.0, 520.0, 330.0, false),
    ];

    let placed = place(&monitors, COMPACTING);

    assert!(
        is_connected(&values(&placed), REQ),
        "M2 landed touching nothing: {}",
        signature(&placed)
    );
    assert_eq!(Rect::new(-620.0, -720.0, 340.0, 600.0), placed["M1"]);
}

/// With a corridor demanded, a corner-only contact slides by the minimum; with no
/// requirement the same layout is left untouched.
#[test]
fn corner_only_contact_slides_minimally_or_is_accepted_at_zero() {
    let monitors = [
        m("A", 0.0, 0.0, 700.0, 400.0, true),
        m("B", 700.0, 400.0, 600.0, 340.0, false), // touches A at the single point (700,400)
    ];

    // req = 20: slide up by exactly the missing 20mm, nothing more.
    let placed = place(&monitors, COMPACTING);
    assert_eq!(Rect::new(700.0, 380.0, 600.0, 340.0), placed["B"]);
    assert_equal_precision(REQ, corridor(&placed["A"], &placed["B"]), 6);

    // req = 0: the corner is a valid contact, B stays where the user put it.
    let loose = place(
        &monitors,
        CompactionOptions {
            minimal_edge_overlap: 0.0,
            ..COMPACTING
        },
    );
    assert_eq!(Rect::new(700.0, 400.0, 600.0, 340.0), loose["B"]);
}

/// Bezels cannot carry the cursor: outside rects overlapping while the panels share
/// nothing must still be slid into a real corridor.
#[test]
fn bezel_only_contact_is_not_a_corridor() {
    // 20mm bezels: outside rects overlap 30mm vertically, panels are 10mm apart.
    let monitors = [
        m_bezel("A", 0.0, 0.0, 700.0, 400.0, true, 20.0),
        m_bezel("B", 740.0, 410.0, 600.0, 340.0, false, 20.0),
    ];

    let placed = place(&monitors, COMPACTING);

    // Slid up 30mm: 20mm of panels shared, measured on the display surface.
    assert_equal_precision(REQ, corridor(&placed["A"], &placed["B"]), 6);
    assert_eq!(Rect::new(740.0, 380.0, 600.0, 340.0), placed["B"]);

    let loose = place(
        &monitors,
        CompactionOptions {
            minimal_edge_overlap: 0.0,
            ..COMPACTING
        },
    );
    assert_eq!(Rect::new(740.0, 410.0, 600.0, 340.0), loose["B"]);
}

/// The slide is the minimal displacement into the valid band, never a recentring.
#[test]
fn slide_is_minimal_not_a_centring() {
    let monitors = [
        m("A", 0.0, 0.0, 700.0, 400.0, true),
        m("B", 1200.0, 600.0, 600.0, 340.0, false),
    ];

    let placed = place(&monitors, COMPACTING);

    assert_eq!(Rect::new(700.0, 380.0, 600.0, 340.0), placed["B"]);
    assert_equal_precision(REQ, corridor(&placed["A"], &placed["B"]), 6);
}

#[test]
fn repeated_solves_return_the_same_result() {
    let monitors = [
        m("A", 0.0, 0.0, 700.0, 400.0, true),
        m("B", 1300.0, 40.0, 600.0, 340.0, false),
        m("C", 1310.0, 50.0, 800.0, 350.0, false),
    ];

    let first = signature(&place(&monitors, COMPACTING));
    for _ in 0..5 {
        assert_eq!(first, signature(&place(&monitors, COMPACTING)));
    }
}

// ------------------------------------------------------------------ tools

/// Deterministic LCG: `System.Random`'s sequence is not contractually stable.
struct Lcg {
    s: u64,
}

impl Lcg {
    fn new(seed: u64) -> Self {
        Self { s: seed }
    }

    fn next(&mut self, max_exclusive: i32) -> i32 {
        self.s = self
            .s
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((self.s >> 16) % max_exclusive as u64) as i32
    }
}

/// Every permutation of `0..n`, in lexicographic order.
fn permutations(n: usize) -> Vec<Vec<usize>> {
    let mut all = Vec::new();
    let mut idx: Vec<usize> = (0..n).collect();
    loop {
        all.push(idx.clone());

        let mut i = n as isize - 2;
        while i >= 0 && idx[i as usize] >= idx[i as usize + 1] {
            i -= 1;
        }
        if i < 0 {
            return all;
        }
        let i = i as usize;

        let mut j = n - 1;
        while idx[j] <= idx[i] {
            j -= 1;
        }
        idx.swap(i, j);
        idx[i + 1..].reverse();
    }
}
