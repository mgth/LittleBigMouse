//! `CompactionSolver.cs`: the pure geometry behind `MonitorsLayout.ForceCompact`.
//! Resolve overlaps, group touching monitors into clusters, pull each cluster
//! against the primary's, and settle until nothing overlaps and everything forms a
//! single block.
//!
//! Contact is bezel against bezel (outside bounds touching), corner to corner
//! included. On top of that, [`CompactionOptions::minimal_edge_overlap`] demands a
//! crossing corridor: at least that many mm of display surface (panels, bezels
//! excluded) shared along the contact. Zero keeps the bare contact rule.

use super::distance::RectDistance;
use super::{ordinal, IdMap};
use crate::geo::dotnet::{compare, max, min};
use crate::geo::{Point, Rect, Vector};

/// `CompactionMonitor`: one monitor reduced to what compaction depends on, its id,
/// its mm panel bounds (`DepthProjection.Bounds`) and its mm outside bounds (panel
/// plus bezels, `DepthProjection.OutsideBounds`).
#[derive(Clone, Debug, PartialEq)]
pub struct CompactionMonitor {
    pub id: String,
    pub bounds: Rect,
    pub outside_bounds: Rect,
    pub primary: bool,
}

impl CompactionMonitor {
    /// The positional constructor `CompactionMonitor(Id, Bounds, OutsideBounds, Primary)`.
    pub fn new(id: impl Into<String>, bounds: Rect, outside_bounds: Rect, primary: bool) -> Self {
        Self {
            id: id.into(),
            bounds,
            outside_bounds,
            primary,
        }
    }
}

/// `CompactionOptions`: the layout options compaction obeys. `Default` is the C#
/// `default(CompactionOptions)`, and `minimal_edge_overlap` defaults to 0 in the C#
/// constructor too.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CompactionOptions {
    pub allow_overlaps: bool,
    pub allow_discontinuity: bool,
    /// The corridor requirement, in mm of panel shared along a contact.
    pub minimal_edge_overlap: f64,
}

impl CompactionOptions {
    /// The positional constructor
    /// `CompactionOptions(AllowOverlaps, AllowDiscontinuity, MinimalEdgeOverlap)`.
    pub const fn new(
        allow_overlaps: bool,
        allow_discontinuity: bool,
        minimal_edge_overlap: f64,
    ) -> Self {
        Self {
            allow_overlaps,
            allow_discontinuity,
            minimal_edge_overlap,
        }
    }
}

/// `CompactionSolver.ContactEpsilon`: contacts are computed values, so this much
/// rounding noise is allowed when deciding whether two monitors touch (or overlap,
/// when overlaps are permitted).
pub const CONTACT_EPSILON: f64 = 0.5;

/// `CompactionSolver.SettlePasses`: bound on push-apart / pull-together rounds
/// after the first pass.
const SETTLE_PASSES: i32 = 8;

/// `CompactionSolver.Body`: the working copy of one monitor; both rects move
/// together.
#[derive(Clone, Copy, Debug)]
struct Body {
    outside: Rect,
    panel: Rect,
}

impl Body {
    fn translate(&self, dx: f64, dy: f64) -> Body {
        Body {
            outside: Rect::from_location_size(
                Point::new(self.outside.x() + dx, self.outside.y() + dy),
                self.outside.size(),
            ),
            panel: Rect::from_location_size(
                Point::new(self.panel.x() + dx, self.panel.y() + dy),
                self.panel.size(),
            ),
        }
    }
}

/// `CompactionSolver.Solve`: the translations to apply, keyed by monitor id, in
/// ordinal id order. Monitors that do not move are absent, the primary always:
/// it anchors the layout where it is. Nothing moves without a primary or with
/// fewer than two monitors.
///
/// The monitors are sorted by id (ordinal, stable) before anything else, so the
/// outcome does not depend on the caller's order.
pub fn solve(monitors: &[CompactionMonitor], options: CompactionOptions) -> IdMap<Vector> {
    let mut result = IdMap::new();
    if monitors.len() < 2 {
        return result;
    }

    let mut ordered: Vec<&CompactionMonitor> = monitors.iter().collect();
    ordered.sort_by(|a, b| ordinal(&a.id, &b.id));

    // Nothing to pull against until the primary is known.
    let Some(primary) = index_of_primary(&ordered) else {
        return result;
    };

    let req = options.minimal_edge_overlap;

    let mut bodies: Vec<Body> = ordered
        .iter()
        .map(|m| Body {
            outside: m.outside_bounds,
            panel: m.bounds,
        })
        .collect();

    if !options.allow_overlaps {
        resolve_overlaps(&mut bodies, primary);
    }

    if !options.allow_discontinuity {
        pull_clusters_together(&mut bodies, primary, req);

        // A cluster pulled against the monitors anchored so far can land on one
        // still queued; push apart and pull back together until settled.
        if !options.allow_overlaps {
            let mut pass = 0;
            while pass < SETTLE_PASSES && !is_settled(&bodies, req) {
                resolve_overlaps(&mut bodies, primary);
                pull_clusters_together(&mut bodies, primary, req);
                pass += 1;
            }
        }
    }

    for (i, m) in ordered.iter().enumerate() {
        let offset = Vector::new(
            bodies[i].outside.x() - m.outside_bounds.x(),
            bodies[i].outside.y() - m.outside_bounds.y(),
        );

        if offset.x != 0.0 || offset.y != 0.0 {
            result.insert(&m.id, offset);
        }
    }

    result
}

fn index_of_primary(monitors: &[&CompactionMonitor]) -> Option<usize> {
    monitors.iter().position(|m| m.primary)
}

/// `CompactionSolver.PullClustersTogether`: pull each connected cluster, as a rigid
/// group, toward the primary's cluster, nearest first.
fn pull_clusters_together(bodies: &mut [Body], primary: usize, req: f64) {
    let mut clusters = build_clusters(bodies, req);

    // The primary anchors everything: its cluster never moves. The others keep
    // their order (`clusters.Where(c => c != anchored)`).
    let anchored_at = clusters
        .iter()
        .position(|c| c.contains(&primary))
        .expect("every monitor is in a cluster");
    let mut anchored = clusters.remove(anchored_at);
    let mut todo = clusters;

    while !todo.is_empty() {
        // `todo.OrderBy(PullCost).First()`: the first of the cheapest.
        let mut next = 0;
        let mut next_cost = pull_cost(bodies, &todo[0], &anchored, req);
        for (i, c) in todo.iter().enumerate().skip(1) {
            let cost = pull_cost(bodies, c, &anchored, req);
            if compare(cost, next_cost).is_lt() {
                next = i;
                next_cost = cost;
            }
        }
        let cluster = todo.remove(next);

        let (dx, dy) = cluster_pull(bodies, &cluster, &anchored, req);
        if dx != 0.0 || dy != 0.0 {
            for &i in &cluster {
                bodies[i] = bodies[i].translate(dx, dy);
            }
        }

        anchored.extend_from_slice(&cluster);
    }
}

/// `CompactionSolver.SlideInto`: smallest displacement of the span
/// `lo..lo + size` so that it overlaps the anchor span by at least `required`
/// (clamped to what the two sizes allow). Zero when it already does.
fn slide_into(lo: f64, size: f64, anchor_lo: f64, anchor_size: f64, required: f64) -> f64 {
    let r = min(required, min(size, anchor_size));

    let min_ = anchor_lo + r - (lo + size);
    let max_ = anchor_lo + anchor_size - r - lo;

    max(min_, min(0.0, max_))
}

/// `CompactionSolver.BestDock`: the cheapest way to park `m` against `a`, as
/// (cost, dx, dy): one of the four sides, bezels flush, plus the minimal
/// perpendicular slide that yields the corridor.
fn best_dock(m: &Body, a: &Body, req: f64) -> (f64, f64, f64) {
    let slide_y = if req > 0.0 {
        slide_into(
            m.panel.y(),
            m.panel.height(),
            a.panel.y(),
            a.panel.height(),
            req,
        )
    } else {
        slide_into(
            m.outside.y(),
            m.outside.height(),
            a.outside.y(),
            a.outside.height(),
            0.0,
        )
    };

    let slide_x = if req > 0.0 {
        slide_into(
            m.panel.x(),
            m.panel.width(),
            a.panel.x(),
            a.panel.width(),
            req,
        )
    } else {
        slide_into(
            m.outside.x(),
            m.outside.width(),
            a.outside.x(),
            a.outside.width(),
            0.0,
        )
    };

    // Right of, left of, below, above the anchor.
    let options = [
        (a.outside.right() - m.outside.x(), slide_y),
        (a.outside.x() - m.outside.right(), slide_y),
        (slide_x, a.outside.bottom() - m.outside.y()),
        (slide_x, a.outside.y() - m.outside.bottom()),
    ];

    let mut best = (f64::INFINITY, 0.0, 0.0);

    for (dx, dy) in options {
        // Strictly closer only, so the first option wins ties.
        let cost = Vector::new(dx, dy).length();
        if cost >= best.0 {
            continue;
        }

        best = (cost, dx, dy);
    }

    best
}

/// `CompactionSolver.ClusterPull`: the translation bringing `cluster` into contact
/// with the anchored group, the cheapest docking over every (cluster monitor,
/// anchored monitor) pair.
fn cluster_pull(bodies: &[Body], cluster: &[usize], anchored: &[usize], req: f64) -> (f64, f64) {
    let mut best = (f64::INFINITY, 0.0, 0.0);

    for &m in cluster {
        for &a in anchored {
            let dock = best_dock(&bodies[m], &bodies[a], req);
            if dock.0 >= best.0 {
                continue;
            }

            best = dock;
        }
    }

    (best.1, best.2)
}

/// `CompactionSolver.PullCost`: how far this cluster has to travel to dock, the
/// "nearest first" ordering key.
fn pull_cost(bodies: &[Body], cluster: &[usize], anchored: &[usize], req: f64) -> f64 {
    let mut best = f64::INFINITY;

    for &m in cluster {
        for &a in anchored {
            best = min(best, best_dock(&bodies[m], &bodies[a], req).0);
        }
    }

    best
}

/// `CompactionSolver.ResolveOverlaps`: push overlapping monitors apart, the
/// primary never moving. Out of the four ways out of an overlap, the shortest push
/// that lands on no other monitor, else the shortest push; iterate until stable.
fn resolve_overlaps(bodies: &mut [Body], primary: usize) {
    for _pass in 0..bodies.len() + 4 {
        let mut moved = false;
        for i in 0..bodies.len() {
            if i == primary {
                continue;
            }

            let Some(overlapped) = first_overlapping(bodies, &bodies[i].outside, i) else {
                continue;
            };

            let d = bodies[i].outside.distance(&bodies[overlapped].outside);

            // Right, left, below, above, shortest first; the sort is stable, so
            // equal-length pushes keep this order.
            let mut candidates = [
                (-d.left, 0.0),
                (d.right, 0.0),
                (0.0, -d.top),
                (0.0, d.bottom),
            ];
            candidates.sort_by(|a, b| compare(a.0.abs() + a.1.abs(), b.0.abs() + b.1.abs()));

            let free = candidates
                .iter()
                .copied()
                .find(|c| {
                    first_overlapping(bodies, &translate(&bodies[i].outside, c.0, c.1), i).is_none()
                })
                .unwrap_or(candidates[0]);

            bodies[i] = bodies[i].translate(free.0, free.1);
            moved = true;
        }
        if !moved {
            return;
        }
    }
}

/// `CompactionSolver.FirstOverlapping`: index of the first monitor `rect` overlaps,
/// ignoring `self_`.
fn first_overlapping(bodies: &[Body], rect: &Rect, self_: usize) -> Option<usize> {
    (0..bodies.len()).find(|&i| i != self_ && overlap(rect, &bodies[i].outside))
}

fn translate(rect: &Rect, dx: f64, dy: f64) -> Rect {
    Rect::from_location_size(Point::new(rect.x() + dx, rect.y() + dy), rect.size())
}

/// `CompactionSolver.Overlap`: more than [`CONTACT_EPSILON`] of penetration on both
/// axes. Not `LayoutGeometry.Overlap`, which has no epsilon.
fn overlap(a: &Rect, b: &Rect) -> bool {
    let d = a.distance(b);
    max(d.left, d.right) < -CONTACT_EPSILON && max(d.top, d.bottom) < -CONTACT_EPSILON
}

/// `CompactionSolver.AreConnected`: bezels in contact (a corner counts) and, when a
/// corridor is required, at least that many mm of panel shared on one axis.
fn are_connected(a: &Body, b: &Body, req: f64) -> bool {
    let d = a.outside.distance(&b.outside);

    if max(d.left, d.right) > CONTACT_EPSILON || max(d.top, d.bottom) > CONTACT_EPSILON {
        return false;
    }

    if req <= 0.0 {
        return true;
    }

    let corridor_x = min(a.panel.right(), b.panel.right()) - max(a.panel.x(), b.panel.x());
    let corridor_y = min(a.panel.bottom(), b.panel.bottom()) - max(a.panel.y(), b.panel.y());

    max(corridor_x, corridor_y) >= req - CONTACT_EPSILON
}

/// `CompactionSolver.IsSettled`: no overlap left, and one single block.
fn is_settled(bodies: &[Body], req: f64) -> bool {
    for i in 0..bodies.len() {
        if first_overlapping(bodies, &bodies[i].outside, i).is_some() {
            return false;
        }
    }

    build_clusters(bodies, req).len() == 1
}

/// `CompactionSolver.BuildClusters`: connected components over [`are_connected`],
/// as index lists. Each cluster grows by scanning the remaining monitors from the
/// last to the first.
fn build_clusters(bodies: &[Body], req: f64) -> Vec<Vec<usize>> {
    let mut clusters = Vec::new();
    let mut remaining: Vec<usize> = (0..bodies.len()).collect();

    while !remaining.is_empty() {
        let mut cluster = vec![remaining.remove(0)];

        let mut grown = true;
        while grown {
            grown = false;
            for i in (0..remaining.len()).rev() {
                if !cluster
                    .iter()
                    .any(|&m| are_connected(&bodies[m], &bodies[remaining[i]], req))
                {
                    continue;
                }
                cluster.push(remaining[i]);
                remaining.remove(i);
                grown = true;
            }
        }
        clusters.push(cluster);
    }
    clusters
}
