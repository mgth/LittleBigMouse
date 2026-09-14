//! Writing the desktop topology: `LinuxDisplayController.SetLocations`.
//!
//! The reading side of this crate asks the compositor where the outputs are. This is the
//! other direction — telling it where they should be — and it is the one place in the
//! product where getting the arithmetic wrong is not a cosmetic bug: **a one-pixel error
//! turns an intended edge contact into an overlap or a gap**, which is the exact topology
//! corruption the mouse engine cannot paper over.
//!
//! Everything that decides *what* to ask for is a pure function here, tested; only
//! [`apply`] talks to the compositor. The reason for that split is that the effect cannot
//! be tested without moving somebody's real screens.
//!
//! The order of operations is not arbitrary, and the C# earned each step:
//!
//! 1. **Translate to non-negative.** kscreen refuses negative positions for enabled
//!    outputs, so the Windows-style "primary at (0,0)" anchor cannot be applied verbatim:
//!    the whole topology moves so its top-left corner is the origin, wherever that puts
//!    the primary.
//! 2. **Scales first, in their own pass.** A scale change resizes the output's logical
//!    rectangle and lets the compositor re-shuffle its neighbours to its own taste — a
//!    1px overlap appearing at a shared edge was observed live. Whatever it decides, the
//!    positions pass must be the last word.
//! 3. **Re-read the sizes.** The compositor's own rounding of native/scale is
//!    authoritative and can differ by a pixel from the prediction.
//! 4. **Chain the contacts flush** with those actual sizes ([`snap_to_actual`]), so an
//!    intended edge contact stays a contact.
//! 5. **Positions**, then **verify**: re-query, and re-assert once whatever the
//!    compositor moved while settling.
//! 6. **Audit** every enabled output, including ones this batch never touched — a rarely
//!    used output overlapping a neighbour by 1280px was observed live.

use std::collections::BTreeMap;

use lbm_layout::geo::dotnet;

use super::LinuxMonitor;

/// `RoundI`: `(int)Math.Round(v)`, which is **half to even** and not half away from zero.
/// A pixel is what this whole module is about, so the tie rule is the C#'s.
fn round_i(v: f64) -> i64 {
    dotnet::round(v) as i64
}

/// What the caller wants of one output: where it goes, and optionally at what scale.
///
/// Positions are in the platform's positioning space — logical pixels on Wayland.
#[derive(Clone, Debug, PartialEq)]
pub struct Wanted {
    /// The connector, which is what both tools name an output by.
    pub connector: String,
    pub x: f64,
    pub y: f64,
    pub scale: Option<f64>,
}

/// One output placed: the intended position with the sizes the solver believed in, plus
/// the sizes the compositor really settled on.
#[derive(Clone, Debug, PartialEq)]
pub struct Placed {
    pub name: String,
    pub x: f64,
    pub y: f64,
    pub predicted_width: f64,
    pub predicted_height: f64,
    pub actual_width: f64,
    pub actual_height: f64,
}

/// The whole topology moved so its top-left corner is the origin.
///
/// kscreen refuses a negative position for an enabled output, so this is not a tidying
/// step: without it the request is rejected outright.
pub fn anchored(wanted: &[Wanted]) -> Vec<Wanted> {
    let (Some(dx), Some(dy)) = (
        wanted.iter().map(|w| w.x).reduce(f64::min),
        wanted.iter().map(|w| w.y).reduce(f64::min),
    ) else {
        return Vec::new();
    };
    wanted
        .iter()
        .map(|w| Wanted {
            connector: w.connector.clone(),
            x: w.x - dx,
            y: w.y - dy,
            scale: w.scale,
        })
        .collect()
}

/// The ones worth asking for: a scale to set, or a position that is not where the output
/// already is.
///
/// An empty answer means the caller can stop — **and must not restore the engine's gaps
/// either**: an engine running with its gaps keeps them.
pub fn changed(wanted: &[Wanted], now: &[LinuxMonitor]) -> Vec<Wanted> {
    wanted
        .iter()
        .filter(|w| {
            if w.scale.is_some() {
                return true;
            }
            match now.iter().find(|m| m.connector_name == w.connector) {
                Some(m) => m.logical_x != w.x || m.logical_y != w.y,
                None => true,
            }
        })
        .cloned()
        .collect()
}

/// A scale differing by less than this is the same scale: `1.0 / 240`, the C#'s own
/// threshold for "the user did not change it".
const SAME_SCALE: f64 = 1.0 / 240.0;

/// What each output will look like once the scales are applied — before the compositor
/// has had its say.
///
/// The positions the solver produced are only consistent with **these** sizes, which is
/// why they are carried rather than re-derived later.
pub fn predict(wanted: &[Wanted], before: &[LinuxMonitor]) -> Vec<Placed> {
    wanted
        .iter()
        .map(|w| {
            let b = before
                .iter()
                .find(|m| m.enabled && m.connector_name == w.connector);
            let (mut width, mut height) = match b {
                Some(m) => (m.logical_width, m.logical_height),
                None => (0.0, 0.0),
            };
            if let (Some(m), Some(scale)) = (b, w.scale) {
                if (scale - m.scale).abs() >= SAME_SCALE {
                    width = dotnet::round(m.pixel_width as f64 / scale);
                    height = dotnet::round(m.pixel_height as f64 / scale);
                }
            }
            Placed {
                name: w.connector.clone(),
                x: w.x,
                y: w.y,
                predicted_width: width,
                predicted_height: height,
                actual_width: width,
                actual_height: height,
            }
        })
        .collect()
}

/// The sizes the compositor actually settled on, folded into the placement.
pub fn with_actual_sizes(placed: &[Placed], live: &[LinuxMonitor]) -> Vec<Placed> {
    placed
        .iter()
        .map(|p| {
            match live
                .iter()
                .find(|m| m.enabled && m.connector_name == p.name)
            {
                Some(m) => Placed {
                    actual_width: m.logical_width,
                    actual_height: m.logical_height,
                    ..p.clone()
                },
                None => p.clone(),
            }
        })
        .collect()
}

/// Two outputs are within a pixel and a half of touching: the C#'s `contactTolerance`.
const CONTACT: f64 = 1.5;

/// Rebuild the intended edge contacts with the **actual** output sizes.
///
/// Wherever the solver meant two outputs to touch — edge to edge in its own predicted
/// space — chain them flush using the sizes the compositor really applied. An output with
/// no contact on an axis keeps its intended coordinate.
///
/// A found contact **replaces** the intended coordinate, which closes gaps as well as
/// overlaps; the maximum only arbitrates between several neighbours.
pub fn snap_to_actual(placed: &[Placed]) -> BTreeMap<String, (f64, f64)> {
    // Do the spans overlap on the perpendicular axis, in intended space?
    fn spans(a_lo: f64, a_len: f64, b_lo: f64, b_len: f64) -> bool {
        f64::min(a_lo + a_len, b_lo + b_len) - f64::max(a_lo, b_lo) > 0.5
    }

    let mut result: BTreeMap<String, (f64, f64)> = placed
        .iter()
        .map(|p| (p.name.clone(), (p.x, p.y)))
        .collect();

    let mut by_x: Vec<&Placed> = placed.iter().collect();
    by_x.sort_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)));
    for item in by_x {
        let mut chained: Option<f64> = None;
        for prior in placed {
            if prior.name == item.name
                || !spans(
                    prior.y,
                    prior.predicted_height,
                    item.y,
                    item.predicted_height,
                )
                || (prior.x + prior.predicted_width - item.x).abs() > CONTACT
            {
                continue;
            }
            let x = result[&prior.name].0 + prior.actual_width;
            chained = Some(chained.map_or(x, |c: f64| c.max(x)));
        }
        if let Some(x) = chained {
            result.get_mut(&item.name).expect("placed").0 = x;
        }
    }

    let mut by_y: Vec<&Placed> = placed.iter().collect();
    by_y.sort_by(|a, b| a.y.total_cmp(&b.y).then(a.x.total_cmp(&b.x)));
    for item in by_y {
        let mut chained: Option<f64> = None;
        for prior in placed {
            if prior.name == item.name
                || !spans(prior.x, prior.predicted_width, item.x, item.predicted_width)
                || (prior.y + prior.predicted_height - item.y).abs() > CONTACT
            {
                continue;
            }
            let y = result[&prior.name].1 + prior.actual_height;
            chained = Some(chained.map_or(y, |c: f64| c.max(y)));
        }
        if let Some(y) = chained {
            result.get_mut(&item.name).expect("placed").1 = y;
        }
    }

    result
}

/// The `kscreen-doctor` arguments that set the scales, which go in their own pass first.
pub fn scale_arguments(wanted: &[Wanted]) -> Vec<String> {
    wanted
        .iter()
        .filter_map(|w| {
            // Written the way .NET writes a double with the invariant culture, which is
            // the spelling the C# sends and the one kscreen-doctor parses.
            w.scale
                .map(|s| format!("output.{}.scale.{}", w.connector, dotnet::format_double(s)))
        })
        .collect()
}

/// The `kscreen-doctor` arguments that set the positions.
pub fn position_arguments(
    placed: &[Placed],
    snapped: &BTreeMap<String, (f64, f64)>,
) -> Vec<String> {
    placed
        .iter()
        .map(|p| {
            let (x, y) = snapped.get(&p.name).copied().unwrap_or((p.x, p.y));
            format!("output.{}.position.{},{}", p.name, round_i(x), round_i(y))
        })
        .collect()
}

/// The `xrandr` arguments, for a session that is not KDE.
pub fn xrandr_arguments(wanted: &[Wanted]) -> Vec<String> {
    wanted
        .iter()
        .flat_map(|w| {
            [
                "--output".to_owned(),
                w.connector.clone(),
                "--pos".to_owned(),
                format!("{}x{}", round_i(w.x), round_i(w.y)),
            ]
        })
        .collect()
}

/// Which outputs the compositor put somewhere other than where they were asked to go.
///
/// Compared at whole pixels, as the request was made — a drift below that is not
/// something the request could have expressed.
pub fn drifted(
    expected: &BTreeMap<String, (f64, f64)>,
    live: &[LinuxMonitor],
) -> Vec<(String, (i64, i64))> {
    expected
        .iter()
        .filter_map(|(name, (x, y))| {
            let m = live
                .iter()
                .find(|m| m.enabled && m.connector_name == *name)?;
            let (want_x, want_y) = (round_i(*x), round_i(*y));
            (round_i(m.logical_x) != want_x || round_i(m.logical_y) != want_y)
                .then(|| (name.clone(), (want_x, want_y)))
        })
        .collect()
}

/// Every pair of enabled outputs that ends up overlapping, described.
///
/// Over **every** enabled output, not only the ones this batch touched: an output nobody
/// moved keeps whatever stale position the compositor had for it, and that is how a
/// rarely used screen ends up 1280px on top of its neighbour.
pub fn overlaps(live: &[LinuxMonitor]) -> Vec<String> {
    let enabled: Vec<&LinuxMonitor> = live.iter().filter(|m| m.enabled).collect();
    let mut found = Vec::new();
    for (i, a) in enabled.iter().enumerate() {
        for b in enabled.iter().skip(i + 1) {
            let w = f64::min(a.logical_x + a.logical_width, b.logical_x + b.logical_width)
                - f64::max(a.logical_x, b.logical_x);
            let h = f64::min(
                a.logical_y + a.logical_height,
                b.logical_y + b.logical_height,
            ) - f64::max(a.logical_y, b.logical_y);
            if w > 0.5 && h > 0.5 {
                found.push(format!(
                    "{} ({},{} {}x{}) / {} ({},{} {}x{})",
                    a.connector_name,
                    a.logical_x,
                    a.logical_y,
                    a.logical_width,
                    a.logical_height,
                    b.connector_name,
                    b.logical_x,
                    b.logical_y,
                    b.logical_width,
                    b.logical_height
                ));
            }
        }
    }
    found
}

/// What one apply did, for a caller that wants to say so.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Applied {
    /// Nothing needed doing — and the caller must **not** restore the engine's gaps
    /// either: an engine running with its gaps keeps them.
    pub unchanged: bool,
    /// Outputs the compositor kept putting somewhere else, after one re-assertion.
    pub stubborn: Vec<String>,
    /// Pairs of enabled outputs still overlapping afterwards, described.
    pub overlapping: Vec<String>,
}

/// How long a `kscreen-doctor` that applies is given. Longer than the read path's
/// patience: this one waits on the compositor reconfiguring outputs, not on a query.
const APPLYING: std::time::Duration = std::time::Duration::from_secs(10);

/// Runs `kscreen-doctor`, which **exits 0 even when the compositor rejects the config**:
/// the only failure signal is the word "failed" on its output.
fn kscreen(arguments: &[String]) -> Result<(), String> {
    let args: Vec<&str> = arguments.iter().map(String::as_str).collect();
    let Some(output) = super::probe::run_with_stderr("kscreen-doctor", &args, APPLYING) else {
        return Err(format!("kscreen-doctor {} did not answer", args.join(" ")));
    };
    if output.to_lowercase().contains("failed") {
        return Err(format!(
            "kscreen-doctor {} failed: {}",
            args.join(" "),
            output.trim()
        ));
    }
    Ok(())
}

/// Applies a whole topology: the C#'s `SetLocations`, in the order its comments earned.
///
/// **`before_apply` is where the engine's gaps are closed**, and it is a parameter rather
/// than a call because this crate cannot reach the gap guard: the guard lives in
/// `lbm-agent` (decision D7) and a frontend that linked the agent could start a hook. A
/// caller with no gaps to close passes a closure that does nothing — visibly, as a
/// decision, rather than by forgetting. It is not called when nothing needs changing:
/// an engine running with its gaps keeps them.
///
/// **Not exercised anywhere.** Running it moves real screens, so there is no test and no
/// trial run behind it; everything it decides is in the pure functions above, which are.
/// The orchestration itself — the order of the passes, the re-read, the retry — is
/// transcribed from the C# and has not been watched working.
pub fn apply(
    backend: super::Backend,
    wanted: &[Wanted],
    before_apply: impl FnOnce(&[LinuxMonitor]),
) -> Result<Applied, String> {
    let anchored = anchored(wanted);
    if anchored.is_empty() {
        return Ok(Applied {
            unchanged: true,
            ..Default::default()
        });
    }
    let now = backend.query().map_err(|e| e.to_string())?;
    let moved = changed(&anchored, &now);
    if moved.is_empty() {
        return Ok(Applied {
            unchanged: true,
            ..Default::default()
        });
    }

    before_apply(&now);

    if backend != super::Backend::KScreen {
        let args = xrandr_arguments(&moved);
        let borrowed: Vec<&str> = args.iter().map(String::as_str).collect();
        return match super::probe::run_with_stderr("xrandr", &borrowed, APPLYING) {
            Some(_) => Ok(Applied::default()),
            None => Err("xrandr did not answer".to_owned()),
        };
    }

    let mut placed = predict(&moved, &now);

    // Scales first and alone: a scale change resizes the logical rectangle and lets the
    // compositor re-shuffle neighbours to its own taste. The positions pass below has to
    // be the last word.
    let scales = scale_arguments(&moved);
    if !scales.is_empty() {
        kscreen(&scales)?;
        // Its rounding of native/scale is authoritative and can differ by a pixel from
        // the prediction — enough to turn an intended contact into an overlap.
        let actual = backend.query().map_err(|e| e.to_string())?;
        placed = with_actual_sizes(&placed, &actual);
    }

    let snapped = snap_to_actual(&placed);
    kscreen(&position_arguments(&placed, &snapped))?;

    // Trust but verify: re-assert once whatever the compositor moved while settling.
    let expected: BTreeMap<String, (f64, f64)> = placed
        .iter()
        .map(|p| {
            let at = snapped.get(&p.name).copied().unwrap_or((p.x, p.y));
            (p.name.clone(), at)
        })
        .collect();
    let mut stubborn = Vec::new();
    for attempt in 0..2 {
        let live = backend.query().map_err(|e| e.to_string())?;
        let drift = drifted(&expected, &live);
        if drift.is_empty() {
            break;
        }
        if attempt == 1 {
            stubborn = drift.into_iter().map(|(name, _)| name).collect();
            eprintln!(
                "[lbm-display] the compositor kept overriding positions: {}",
                stubborn.join(", ")
            );
            break;
        }
        let again: Vec<String> = drift
            .iter()
            .map(|(name, (x, y))| format!("output.{name}.position.{x},{y}"))
            .collect();
        kscreen(&again)?;
    }

    let live = backend.query().map_err(|e| e.to_string())?;
    let overlapping = overlaps(&live);
    for pair in &overlapping {
        eprintln!("[lbm-display] outputs overlap after apply: {pair}");
    }
    Ok(Applied {
        unchanged: false,
        stubborn,
        overlapping,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn monitor(name: &str, x: f64, y: f64, w: f64, h: f64) -> LinuxMonitor {
        LinuxMonitor {
            connector_name: name.to_owned(),
            logical_x: x,
            logical_y: y,
            logical_width: w,
            logical_height: h,
            pixel_width: w as i32,
            pixel_height: h as i32,
            scale: 1.0,
            width_mm: 600.0,
            height_mm: 340.0,
            primary: false,
            enabled: true,
            orientation: 0,
            frequency: 60,
            edid: None,
        }
    }

    fn want(name: &str, x: f64, y: f64) -> Wanted {
        Wanted {
            connector: name.to_owned(),
            x,
            y,
            scale: None,
        }
    }

    /// kscreen refuses a negative position, so the anchor is not a tidying step: without
    /// it the whole request is rejected.
    #[test]
    fn the_topology_is_moved_until_nothing_is_negative() {
        let wanted = vec![want("DP-1", -1920.0, -100.0), want("DP-2", 0.0, 0.0)];
        let moved = anchored(&wanted);

        assert_eq!((moved[0].x, moved[0].y), (0.0, 0.0));
        assert_eq!((moved[1].x, moved[1].y), (1920.0, 100.0));
        assert!(moved.iter().all(|w| w.x >= 0.0 && w.y >= 0.0));
    }

    /// And the shape is preserved: this moves the desktop, it does not rearrange it.
    #[test]
    fn anchoring_keeps_every_distance() {
        let wanted = vec![
            want("a", -500.0, 250.0),
            want("b", 1420.0, 250.0),
            want("c", -500.0, 1330.0),
        ];
        let moved = anchored(&wanted);
        for (before, after) in wanted.iter().zip(&moved) {
            for (other_before, other_after) in wanted.iter().zip(&moved) {
                assert_eq!(before.x - other_before.x, after.x - other_after.x);
                assert_eq!(before.y - other_before.y, after.y - other_after.y);
            }
        }
    }

    #[test]
    fn an_output_already_where_it_should_be_is_not_asked_to_move() {
        let now = vec![monitor("DP-1", 0.0, 0.0, 1920.0, 1080.0)];
        assert!(changed(&[want("DP-1", 0.0, 0.0)], &now).is_empty());
        assert_eq!(changed(&[want("DP-1", 10.0, 0.0)], &now).len(), 1);
    }

    /// A scale is always asked for, even at the same position: it is the other half of
    /// what this request can change.
    #[test]
    fn a_scale_is_asked_for_even_where_nothing_moves() {
        let now = vec![monitor("DP-1", 0.0, 0.0, 1920.0, 1080.0)];
        let wanted = Wanted {
            scale: Some(1.25),
            ..want("DP-1", 0.0, 0.0)
        };
        assert_eq!(changed(&[wanted], &now).len(), 1);
    }

    /// The predicted size follows the requested scale, by the compositor's own
    /// arithmetic — native over scale, rounded.
    #[test]
    fn a_new_scale_predicts_the_logical_size_it_will_give() {
        let mut before = monitor("DP-1", 0.0, 0.0, 3840.0, 2160.0);
        before.pixel_width = 3840;
        before.pixel_height = 2160;
        before.scale = 1.0;

        let wanted = vec![Wanted {
            scale: Some(2.0),
            ..want("DP-1", 0.0, 0.0)
        }];
        let placed = predict(&wanted, &[before.clone()]);
        assert_eq!(placed[0].predicted_width, 1920.0);
        assert_eq!(placed[0].predicted_height, 1080.0);

        // A scale that is the same within a 240th is not a change: the size stays as it
        // is rather than being recomputed and rounded a second time.
        let same = vec![Wanted {
            scale: Some(1.0 + 1.0 / 480.0),
            ..want("DP-1", 0.0, 0.0)
        }];
        let placed = predict(&same, &[before]);
        assert_eq!(placed[0].predicted_width, 3840.0);
    }

    /// **The point of the whole snap.** The solver meant two screens to touch at 1920;
    /// the compositor made the first 1921 wide. Keeping the intended 1920 would leave a
    /// one-pixel overlap — the corruption the engine cannot paper over.
    #[test]
    fn a_contact_the_compositor_resized_is_chained_flush_again() {
        let placed = vec![
            Placed {
                name: "left".to_owned(),
                x: 0.0,
                y: 0.0,
                predicted_width: 1920.0,
                predicted_height: 1080.0,
                actual_width: 1921.0,
                actual_height: 1080.0,
            },
            Placed {
                name: "right".to_owned(),
                x: 1920.0,
                y: 0.0,
                predicted_width: 1920.0,
                predicted_height: 1080.0,
                actual_width: 1920.0,
                actual_height: 1080.0,
            },
        ];
        let snapped = snap_to_actual(&placed);
        assert_eq!(snapped["left"], (0.0, 0.0));
        assert_eq!(
            snapped["right"],
            (1921.0, 0.0),
            "the neighbour was left overlapping by the pixel the compositor added"
        );
    }

    /// It closes gaps as well, because a contact *replaces* the intended coordinate.
    #[test]
    fn a_contact_the_compositor_shrank_is_closed_again() {
        let placed = vec![
            Placed {
                name: "left".to_owned(),
                x: 0.0,
                y: 0.0,
                predicted_width: 1920.0,
                predicted_height: 1080.0,
                actual_width: 1918.0,
                actual_height: 1080.0,
            },
            Placed {
                name: "right".to_owned(),
                x: 1920.0,
                y: 0.0,
                predicted_width: 1920.0,
                predicted_height: 1080.0,
                actual_width: 1920.0,
                actual_height: 1080.0,
            },
        ];
        assert_eq!(snap_to_actual(&placed)["right"], (1918.0, 0.0));
    }

    /// An output with no contact on an axis keeps what it was given: the snap chains
    /// contacts, it does not pull everything together.
    #[test]
    fn a_screen_that_touches_nothing_keeps_its_place() {
        let placed = vec![
            Placed {
                name: "here".to_owned(),
                x: 0.0,
                y: 0.0,
                predicted_width: 1920.0,
                predicted_height: 1080.0,
                actual_width: 1920.0,
                actual_height: 1080.0,
            },
            Placed {
                name: "far".to_owned(),
                x: 5000.0,
                y: 2000.0,
                predicted_width: 1920.0,
                predicted_height: 1080.0,
                actual_width: 1920.0,
                actual_height: 1080.0,
            },
        ];
        assert_eq!(snap_to_actual(&placed)["far"], (5000.0, 2000.0));
    }

    /// Screens that merely pass each other on the perpendicular axis are not in contact:
    /// a corner touching a corner is not an edge.
    #[test]
    fn two_screens_that_only_share_a_corner_are_not_chained() {
        let placed = vec![
            Placed {
                name: "left".to_owned(),
                x: 0.0,
                y: 0.0,
                predicted_width: 1920.0,
                predicted_height: 1080.0,
                actual_width: 1900.0,
                actual_height: 1080.0,
            },
            Placed {
                name: "corner".to_owned(),
                x: 1920.0,
                y: 1080.0,
                predicted_width: 1920.0,
                predicted_height: 1080.0,
                actual_width: 1920.0,
                actual_height: 1080.0,
            },
        ];
        assert_eq!(
            snap_to_actual(&placed)["corner"],
            (1920.0, 1080.0),
            "a corner contact was chained as if it were an edge"
        );
    }

    #[test]
    fn the_arguments_are_the_ones_the_tools_read() {
        let placed = vec![Placed {
            name: "DP-1".to_owned(),
            x: 100.4,
            y: 200.6,
            predicted_width: 1920.0,
            predicted_height: 1080.0,
            actual_width: 1920.0,
            actual_height: 1080.0,
        }];
        let snapped = snap_to_actual(&placed);
        assert_eq!(
            position_arguments(&placed, &snapped),
            vec!["output.DP-1.position.100,201"]
        );

        let wanted = vec![Wanted {
            scale: Some(1.25),
            ..want("DP-1", 0.0, 0.0)
        }];
        assert_eq!(scale_arguments(&wanted), vec!["output.DP-1.scale.1.25"]);

        assert_eq!(
            xrandr_arguments(&[want("DP-1", 1920.0, 0.0)]),
            vec!["--output", "DP-1", "--pos", "1920x0"]
        );
    }

    /// `RoundI` is `(int)Math.Round(v)`, which is **half to even**. A pixel is what this
    /// module is about, so the tie rule is the C#'s and not Rust's default.
    #[test]
    fn a_half_pixel_is_rounded_the_way_dotnet_rounds_it() {
        assert_eq!(round_i(0.5), 0, "half away from zero would give 1");
        assert_eq!(round_i(1.5), 2);
        assert_eq!(round_i(2.5), 2, "half away from zero would give 3");
        assert_eq!(round_i(-0.5), 0);
        assert_eq!(round_i(-1.5), -2);
    }

    #[test]
    fn a_compositor_that_moved_something_is_noticed_and_only_that_something() {
        let expected: BTreeMap<String, (f64, f64)> = [
            ("a".to_owned(), (0.0, 0.0)),
            ("b".to_owned(), (1920.0, 0.0)),
        ]
        .into_iter()
        .collect();
        let live = vec![
            monitor("a", 0.0, 0.0, 1920.0, 1080.0),
            monitor("b", 1921.0, 0.0, 1920.0, 1080.0),
        ];
        assert_eq!(drifted(&expected, &live), vec![("b".to_owned(), (1920, 0))]);

        let settled = vec![
            monitor("a", 0.0, 0.0, 1920.0, 1080.0),
            monitor("b", 1920.0, 0.0, 1920.0, 1080.0),
        ];
        assert!(drifted(&expected, &settled).is_empty());
    }

    /// The audit covers outputs this batch never touched, which is where the 1280px
    /// overlap came from.
    #[test]
    fn the_audit_sees_an_untouched_output_sitting_on_its_neighbour() {
        let live = vec![
            monitor("asked", 0.0, 0.0, 1920.0, 1080.0),
            monitor("untouched", 640.0, 0.0, 1920.0, 1080.0),
        ];
        let found = overlaps(&live);
        assert_eq!(found.len(), 1);
        assert!(found[0].contains("asked") && found[0].contains("untouched"));

        let apart = vec![
            monitor("a", 0.0, 0.0, 1920.0, 1080.0),
            monitor("b", 1920.0, 0.0, 1920.0, 1080.0),
        ];
        assert!(
            overlaps(&apart).is_empty(),
            "two screens sharing an edge are not overlapping"
        );
    }

    /// A disabled output is not on the desktop, so it cannot overlap anything.
    #[test]
    fn a_disabled_output_is_not_part_of_the_desktop() {
        let mut off = monitor("off", 0.0, 0.0, 1920.0, 1080.0);
        off.enabled = false;
        let live = vec![monitor("on", 0.0, 0.0, 1920.0, 1080.0), off];
        assert!(overlaps(&live).is_empty());
    }
}
