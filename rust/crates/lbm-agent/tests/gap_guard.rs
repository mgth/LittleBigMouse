//! The KWin gap guard: C#'s `KScreenGapGuardTests` (the shift computation, against the
//! same naive reference), then the journal and the restore rules.

use std::collections::{BTreeMap, HashSet};

use lbm_agent::gap_guard::{compute_shifts, plan_apply, plan_restore, GapEntry};
use lbm_layout::linux::LinuxMonitor;

/// C#: `M(name, x, y, w, h)`, an enabled output with that logical geometry.
fn m(name: &str, x: f64, y: f64, w: f64, h: f64) -> LinuxMonitor {
    LinuxMonitor {
        connector_name: name.into(),
        logical_x: x,
        logical_y: y,
        logical_width: w,
        logical_height: h,
        pixel_width: w as i32,
        pixel_height: h as i32,
        scale: 1.0,
        width_mm: 0.0,
        height_mm: 0.0,
        primary: false,
        enabled: true,
        orientation: 0,
        frequency: 0,
        edid: None,
    }
}

/// C#: `ReferenceShifts`, the obvious per-edge algorithm, deliberately unoptimized.
fn reference_shifts(monitors: &[LinuxMonitor]) -> BTreeMap<String, (i32, i32)> {
    let r = |v: f64| v.round_ties_even() as i32;
    let overlaps = |s1: i32, l1: i32, s2: i32, l2: i32| (s1 + l1).min(s2 + l2) - s1.max(s2) > 0;
    let mut x_cuts = HashSet::new();
    let mut y_cuts = HashSet::new();
    for (i, a) in monitors.iter().enumerate() {
        for (j, b) in monitors.iter().enumerate() {
            if i == j {
                continue;
            }
            let (ax, ay, aw, ah) = (
                r(a.logical_x),
                r(a.logical_y),
                r(a.logical_width),
                r(a.logical_height),
            );
            let (bx, by, bw, bh) = (
                r(b.logical_x),
                r(b.logical_y),
                r(b.logical_width),
                r(b.logical_height),
            );
            if ax + aw == bx && overlaps(ay, ah, by, bh) {
                x_cuts.insert(bx);
            }
            if ay + ah == by && overlaps(ax, aw, bx, bw) {
                y_cuts.insert(by);
            }
        }
    }
    monitors
        .iter()
        .filter_map(|m| {
            let dx = x_cuts.iter().filter(|c| **c <= r(m.logical_x)).count() as i32;
            let dy = y_cuts.iter().filter(|c| **c <= r(m.logical_y)).count() as i32;
            (dx != 0 || dy != 0).then(|| (m.connector_name.clone(), (dx, dy)))
        })
        .collect()
}

fn shifts(monitors: &[LinuxMonitor]) -> BTreeMap<String, (i32, i32)> {
    let shifts: BTreeMap<_, _> = compute_shifts(monitors).into_iter().collect();
    assert_eq!(shifts, reference_shifts(monitors), "matches the reference");
    shifts
}

/// C#: `SingleMonitor_NoShift`.
#[test]
fn single_monitor_no_shift() {
    assert!(shifts(&[m("A", 0.0, 0.0, 1920.0, 1080.0)]).is_empty());
}

/// C#: `TwoSideBySide_RightOutputShiftsByOne`.
#[test]
fn two_side_by_side_right_output_shifts_by_one() {
    let s = shifts(&[
        m("A", 0.0, 0.0, 1920.0, 1080.0),
        m("B", 1920.0, 0.0, 1920.0, 1080.0),
    ]);
    assert!(!s.contains_key("A"), "origin: no cut at or before it");
    assert_eq!(s["B"], (1, 0));
}

/// C#: `TwoStacked_BottomOutputShiftsByOne`.
#[test]
fn two_stacked_bottom_output_shifts_by_one() {
    let s = shifts(&[
        m("A", 0.0, 0.0, 1920.0, 1080.0),
        m("B", 0.0, 1080.0, 1920.0, 1080.0),
    ]);
    assert!(!s.contains_key("A"));
    assert_eq!(s["B"], (0, 1));
}

/// C#: `AlreadyGapped_NoShift_Idempotent` — a gapped topology fed back plans nothing.
#[test]
fn already_gapped_no_shift_idempotent() {
    assert!(shifts(&[
        m("A", 0.0, 0.0, 1920.0, 1080.0),
        m("B", 1921.0, 0.0, 1920.0, 1080.0)
    ])
    .is_empty());
}

/// C#: `OverlapOnlyAtCorner_DoesNotCount` — the 2×2 corner quirk.
#[test]
fn overlap_only_at_corner_does_not_count() {
    assert!(shifts(&[
        m("A", 0.0, 0.0, 1920.0, 1080.0),
        m("B", 1920.0, 1080.0, 1920.0, 1080.0)
    ])
    .is_empty());
}

/// C#: `ThreeInARow_CumulativeShift`.
#[test]
fn three_in_a_row_cumulative_shift() {
    let s = shifts(&[
        m("A", 0.0, 0.0, 1000.0, 1000.0),
        m("B", 1000.0, 0.0, 1000.0, 1000.0),
        m("C", 2000.0, 0.0, 1000.0, 1000.0),
    ]);
    assert!(!s.contains_key("A"));
    assert_eq!((s["B"], s["C"]), ((1, 0), (2, 0)));
}

/// C#: `ScaledAndRotated_UsesLogicalDimensions`.
#[test]
fn scaled_and_rotated_uses_logical_dimensions() {
    let s = shifts(&[
        m("A", 0.0, 0.0, 1280.0, 720.0),
        m("B", 1280.0, 0.0, 720.0, 1280.0),
    ]);
    assert_eq!(s["B"], (1, 0));
}

/// C#: `TwoByTwoGrid_MatchesReference`.
#[test]
fn two_by_two_grid_matches_reference() {
    let s = shifts(&[
        m("TL", 0.0, 0.0, 1920.0, 1080.0),
        m("TR", 1920.0, 0.0, 1920.0, 1080.0),
        m("BL", 0.0, 1080.0, 1920.0, 1080.0),
        m("BR", 1920.0, 1080.0, 1920.0, 1080.0),
    ]);
    assert!(!s.contains_key("TL"));
    assert_eq!((s["TR"], s["BL"], s["BR"]), ((1, 0), (0, 1), (1, 1)));
}

/// C#: `HorizontalStrip_MatchesReferenceAtScale` (2, 6, 16 and 32 outputs).
#[test]
fn horizontal_strip_matches_reference_at_scale() {
    for count in [2, 6, 16, 32] {
        let monitors: Vec<_> = (0..count)
            .map(|i| {
                m(
                    &format!("DP-{i}"),
                    f64::from(i) * 1920.0,
                    0.0,
                    1920.0,
                    1080.0,
                )
            })
            .collect();
        let s = shifts(&monitors);
        assert!(!s.contains_key("DP-0"));
        for i in 1..count {
            assert_eq!(s[&format!("DP-{i}")], (i, 0));
        }
    }
}

/// C#: `SquareGrid_MatchesReference` (2×2, 4×4, 6×6): cell (r, c) shifts (c, r).
#[test]
fn square_grid_matches_reference() {
    for side in [2, 4, 6] {
        let mut monitors = Vec::new();
        for row in 0..side {
            for col in 0..side {
                monitors.push(m(
                    &format!("R{row}C{col}"),
                    f64::from(col) * 1920.0,
                    f64::from(row) * 1080.0,
                    1920.0,
                    1080.0,
                ));
            }
        }
        let s = shifts(&monitors);
        for row in 0..side {
            for col in 0..side {
                let name = format!("R{row}C{col}");
                assert_eq!(
                    s.get(&name).copied().unwrap_or((0, 0)),
                    (col, row),
                    "{name}"
                );
            }
        }
    }
}

//==================//
// Journal, restore //
//==================//

fn entry(name: &str, original: (i32, i32), applied: (i32, i32)) -> GapEntry {
    GapEntry {
        name: name.into(),
        original_x: original.0,
        original_y: original.1,
        applied_x: applied.0,
        applied_y: applied.1,
    }
}

#[test]
fn applying_journals_the_original_positions_and_keeps_those_already_journaled() {
    let side_by_side = [
        m("A", 0.0, 0.0, 1920.0, 1080.0),
        m("B", 1920.0, 0.0, 1920.0, 1080.0),
    ];
    let (journal, args) = plan_apply(&side_by_side, &[]).unwrap();
    assert_eq!(journal, [entry("B", (1920, 0), (1921, 0))]);
    assert_eq!(args, ["output.B.position.1921,0"]);

    // A third output plugged while gapped: B's original stays the pre-gap one.
    let plugged = [
        m("A", 0.0, 0.0, 1920.0, 1080.0),
        m("B", 1921.0, 0.0, 1920.0, 1080.0),
        m("C", 3841.0, 0.0, 1920.0, 1080.0),
    ];
    let (journal, args) = plan_apply(&plugged, &journal).unwrap();
    assert_eq!(
        journal,
        [
            entry("B", (1920, 0), (1921, 0)),
            entry("C", (3841, 0), (3842, 0))
        ]
    );
    assert_eq!(args, ["output.C.position.3842,0"]);

    // Nothing to open: no plan.
    assert!(plan_apply(&[m("A", 0.0, 0.0, 1920.0, 1080.0)], &[]).is_none());
}

#[test]
fn restoring_leaves_alone_what_the_user_moved_or_unplugged() {
    let journal = [
        entry("B", (1920, 0), (1921, 0)),
        entry("C", (3840, 0), (3842, 0)),
        entry("D", (0, 1080), (0, 1081)),
        entry("E", (5760, 0), (5763, 0)),
    ];
    let live = [
        m("A", 0.0, 0.0, 1920.0, 1080.0),
        m("B", 1921.0, 0.0, 1920.0, 1080.0), // as applied: put back
        m("C", 4000.0, 0.0, 1920.0, 1080.0), // moved by the user: left there
        m("D", 0.0, 1080.0, 1920.0, 1080.0), // already back
                                             // E unplugged
    ];
    assert_eq!(plan_restore(&journal, &live), ["output.B.position.1920,0"]);
}

#[test]
fn the_journal_has_the_shape_the_csharp_writes() {
    // What System.Text.Json writes for List<GapEntry>, indented.
    let csharp = r#"[
  {
    "Name": "DP-3",
    "OriginalX": 3072,
    "OriginalY": 0,
    "AppliedX": 3073,
    "AppliedY": 0
  }
]"#;
    let parsed: Vec<GapEntry> = serde_json::from_str(csharp).unwrap();
    assert_eq!(parsed, [entry("DP-3", (3072, 0), (3073, 0))]);
    assert_eq!(serde_json::to_string_pretty(&parsed).unwrap(), csharp);
}

//==========================================================================//
// A dry run must not touch the journal                                     //
//==========================================================================//

/// `restore` deletes the journal when it succeeds. A dry run that called it with a
/// runner that merely recorded would report success and take the journal with it —
/// leaving a gapped topology with nothing left able to put it back.
///
/// The journal is written here by hand rather than through `apply`, whose prologue only
/// runs on a Wayland Plasma session without evdev; what is under test is the epilogue's
/// side effect, not when the prologue fires.
#[test]
fn asking_what_a_restore_would_do_leaves_the_journal_where_it_is() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("kscreen-restore.json");
    let journal = [entry("B", (1920, 0), (1921, 0))];
    std::fs::write(&file, serde_json::to_string(&journal).unwrap()).unwrap();

    let guard = lbm_agent::gap_guard::GapGuard::for_session(file.clone());
    let live = [
        m("A", 0.0, 0.0, 1920.0, 1080.0),
        m("B", 1921.0, 0.0, 1920.0, 1080.0),
    ];

    assert_eq!(
        guard.would_restore(&live),
        ["output.B.position.1920,0"],
        "it still says what a restore would run"
    );
    assert!(
        file.exists(),
        "the dry run deleted the journal: a gapped topology would be unrestorable"
    );

    // And the real thing does consume it, which is what makes the difference matter.
    assert!(guard.restore(&live, |_| true));
    assert!(!file.exists(), "a restore that ran keeps no journal");
}
