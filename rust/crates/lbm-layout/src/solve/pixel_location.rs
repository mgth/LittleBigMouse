//! `PixelLocationSolver.cs`: derive integer system pixel positions from the
//! physical (mm) layout, the inverse of [`super::system_location`]. Physical
//! adjacency becomes exact pixel edge contact, and on the perpendicular axis the
//! shared-span invariant of [`super::edge_projection`] is solved the other way
//! round.
//!
//! Reads the millimetre rects of each [`MonitorSnapshot`] and only the size of its
//! pixel rect: the pixel positions are the answer.

use std::collections::VecDeque;

use super::distance::{RectDistance, ThicknessDistance};
use super::edge_projection;
use super::layout_geometry::{self, Axis, EdgeContact, LayoutGeometryRect, MonitorSnapshot};
use super::IdMap;
use crate::geo::dotnet::{compare, enumerable_max, enumerable_min, max, round};
use crate::geo::{Point, Rect};

/// `PixelLocationSolver.DefaultToleranceMm`: physical gaps up to this count as
/// adjacent, bezel measurements being hand-entered.
pub const DEFAULT_TOLERANCE_MM: f64 = 5.0;

/// `PixelLocationSolver.Axes`.
const AXES: [Axis; 2] = [Axis::Horizontal, Axis::Vertical];

/// `PixelLocationSolver.Solve`: pixel origins keyed by monitor id, in the order of
/// `monitors`, relative to the primary (or to the first monitor when none is
/// primary), which sits at (0, 0). Every monitor is in the result. The C# default
/// for `tolerance_mm` is [`DEFAULT_TOLERANCE_MM`].
///
/// A spanning tree over physical adjacency first (first placement wins), then the
/// detached islands, nearest first, snapped into contact; then a pass pushing each
/// monitor, in placement order, out of the ones placed before it.
///
/// Ids must be unique: C# builds the result with `ToDictionary`, which throws on a
/// duplicate; here that is a debug assertion.
pub fn solve(monitors: &[MonitorSnapshot], tolerance_mm: f64) -> IdMap<Point> {
    let mut placed: IdMap<Rect> = IdMap::new();
    if monitors.is_empty() {
        return IdMap::new();
    }

    let primary = monitors.iter().find(|m| m.primary).unwrap_or(&monitors[0]);
    let mut order: Vec<&MonitorSnapshot> = Vec::new();

    place(&mut placed, &mut order, primary, Point::new(0.0, 0.0));

    // Spanning tree over physical adjacency: first placement wins, grid cycles are
    // reconciled by the overlap pass below.
    let mut todo: VecDeque<&MonitorSnapshot> = VecDeque::new();
    todo.push_back(primary);
    while let Some(a) = todo.pop_front() {
        for b in monitors {
            if placed.contains_key(&b.id) {
                continue;
            }
            let Some(pos) = try_place_adjacent(a, placed[a.id.as_str()], b, tolerance_mm) else {
                continue;
            };
            place(&mut placed, &mut order, b, pos);
            todo.push_back(b);
        }
    }

    // Physically detached islands, nearest first: every ordering key is computed
    // against the tree above before any island is placed (`OrderBy(...).ToList()`),
    // and the sort is stable. LINQ never calls the key selector of a lone element,
    // so a single island gets no key: that is what keeps C# from reaching the
    // throwing `new Rect` of `FromPixels` when a placed rect is empty.
    let pitch_primary = pitch(primary);
    let mut islands: Vec<&MonitorSnapshot> = monitors
        .iter()
        .filter(|m| !placed.contains_key(&m.id))
        .collect();
    if islands.len() > 1 {
        let mut keyed: Vec<(f64, &MonitorSnapshot)> = islands
            .iter()
            .map(|&m| {
                let from_pixels = from_pixels(m);
                let group: Vec<Rect> = placed.values().map(from_pixels).collect();
                let key = m
                    .mm_outside_bounds
                    .distance_to_touch_all(&group, false)
                    .distance_hv();
                (key, m)
            })
            .collect();
        keyed.sort_by(|a, b| compare(a.0, b.0));
        islands = keyed.into_iter().map(|(_, m)| m).collect();
    }

    for b in islands {
        let estimate = Rect::from_location_size(
            Point::new(
                round((b.mm_bounds.x() - primary.mm_bounds.x()) / pitch_primary.x),
                round((b.mm_bounds.y() - primary.mm_bounds.y()) / pitch_primary.y),
            ),
            b.pixel_size(),
        );
        let others: Vec<Rect> = placed.values().copied().collect();
        let snapped = snap_to_touch(estimate, &others);
        place(&mut placed, &mut order, b, snapped.location());
    }

    // Rounding and inconsistent cycle constraints can leave small overlaps: push
    // each monitor (placement order, never the primary) out of the ones before it.
    for i in 1..order.len() {
        let others: Vec<Rect> = order[..i].iter().map(|m| placed[m.id.as_str()]).collect();
        let resolved = resolve_overlap(placed[order[i].id.as_str()], &others);
        placed.insert(&order[i].id, resolved);
    }

    // Re-anchor on the primary.
    let origin = placed[primary.id.as_str()].location();
    let mut result = IdMap::new();
    for m in monitors {
        if !placed.contains_key(&m.id) {
            continue;
        }
        debug_assert!(
            !result.contains_key(&m.id),
            "duplicate monitor id {:?}",
            m.id
        );
        let rect = placed[m.id.as_str()];
        result.insert(&m.id, Point::new(rect.x() - origin.x, rect.y() - origin.y));
    }
    result
}

/// `Solve`'s local `Place`: the rounded position at the monitor's pixel size.
fn place<'a>(
    placed: &mut IdMap<Rect>,
    order: &mut Vec<&'a MonitorSnapshot>,
    m: &'a MonitorSnapshot,
    pos: Point,
) {
    placed.insert(
        &m.id,
        Rect::from_location_size(Point::new(round(pos.x), round(pos.y)), m.pixel_size()),
    );
    order.push(m);
}

/// `PixelLocationSolver.Pitch`: mm per pixel on both axes.
fn pitch(m: &MonitorSnapshot) -> Point {
    Point::new(
        m.profile(Axis::Horizontal).pitch(),
        m.profile(Axis::Vertical).pitch(),
    )
}

/// `PixelLocationSolver.FromPixels`: scale a placed pixel rect back to mm through
/// the island's own pitch; only the relative order of the distances matters.
fn from_pixels(m: &MonitorSnapshot) -> impl Fn(&Rect) -> Rect {
    let pitch = pitch(m);
    move |r| {
        Rect::new(
            r.x() * pitch.x,
            r.y() * pitch.y,
            r.width() * pitch.x,
            r.height() * pitch.y,
        )
    }
}

/// `PixelLocationSolver.TryPlaceAdjacent`: where `b` goes if it is docked against
/// `a` (placed at `a_px`): the bezels meet within tolerance on one axis and overlap
/// on the other. Horizontal is tried first.
fn try_place_adjacent(
    a: &MonitorSnapshot,
    a_px: Rect,
    b: &MonitorSnapshot,
    tolerance: f64,
) -> Option<Point> {
    for axis in AXES {
        // A shared edge with no overlap across it is a corner meeting.
        if layout_geometry::overlap_on(
            &a.mm_outside_bounds,
            &b.mm_outside_bounds,
            axis.perpendicular(),
        ) <= 0.0
        {
            continue;
        }

        let contact = layout_geometry::contact_between_rects(
            &a.mm_outside_bounds,
            &b.mm_outside_bounds,
            axis,
            tolerance,
        );
        if contact == EdgeContact::None {
            continue;
        }

        let anchor_px = a_px.on(axis);
        let lo = if contact == EdgeContact::After {
            anchor_px.hi()
        } else {
            anchor_px.lo - b.pixel_bounds.on(axis).size
        };

        return Some(origin(
            axis,
            lo,
            perpendicular_offset(a, a_px, b, axis.perpendicular()),
        ));
    }

    None
}

/// `PixelLocationSolver.PerpendicularOffset`: the rounded pixel coordinate of `b`
/// on `axis` that keeps the shared physical midpoint on the same pixel.
fn perpendicular_offset(a: &MonitorSnapshot, a_px: Rect, b: &MonitorSnapshot, axis: Axis) -> f64 {
    round(edge_projection::pixel_origin(
        a.profile(axis).at_pixel(a_px.on(axis).lo),
        b.profile(axis),
    ))
}

/// `PixelLocationSolver.Origin`.
fn origin(axis: Axis, along: f64, across: f64) -> Point {
    if axis == Axis::Horizontal {
        Point::new(along, across)
    } else {
        Point::new(across, along)
    }
}

/// `PixelLocationSolver.SnapToTouch`: slide into the group band when no single
/// translation can touch, then translate by the smallest positive distance.
/// `others` is never empty: the primary is placed first.
fn snap_to_touch(mut rect: Rect, others: &[Rect]) -> Rect {
    let mut distance = rect.distance_to_touch_all(others, true);

    if distance.is_positive_infinity() {
        const PLACED: &str = "the primary is always placed";
        let left = enumerable_min(others.iter().map(|r| r.x())).expect(PLACED);
        let top = enumerable_min(others.iter().map(|r| r.y())).expect(PLACED);
        let right = enumerable_max(others.iter().map(|r| r.right())).expect(PLACED);
        let bottom = enumerable_max(others.iter().map(|r| r.bottom())).expect(PLACED);

        let to_left = left - rect.right();
        let to_top = top - rect.bottom();
        let to_right = rect.x() - right;
        let to_bottom = rect.y() - bottom;

        // Slide along the axis with the smaller gap so the rect straddles the
        // group band, then one translation on the other axis touches.
        if max(to_left, to_right) <= max(to_top, to_bottom) {
            rect.set_x(if to_left >= to_right {
                left - round(rect.width() / 2.0)
            } else {
                right - round(rect.width() / 2.0)
            });
        } else {
            rect.set_y(if to_top >= to_bottom {
                top - round(rect.height() / 2.0)
            } else {
                bottom - round(rect.height() / 2.0)
            });
        }

        distance = rect.distance_to_touch_all(others, true);
    }

    let min = distance.min_positive();
    if min > 0.0 && !min.is_infinite() {
        if distance.left > 0.0 && distance.left <= min {
            rect.set_x(rect.x() - distance.left);
        } else if distance.top > 0.0 && distance.top <= min {
            rect.set_y(rect.y() - distance.top);
        } else if distance.right > 0.0 && distance.right <= min {
            rect.set_x(rect.x() + distance.right);
        } else if distance.bottom > 0.0 && distance.bottom <= min {
            rect.set_y(rect.y() + distance.bottom);
        }
    }

    resolve_overlap(rect, others)
}

/// `PixelLocationSolver.ResolveOverlap`: up to eight single-axis pushes out of the
/// first rect of `others` this one overlaps, each by the smallest amount.
fn resolve_overlap(mut rect: Rect, others: &[Rect]) -> Rect {
    for _ in 0..8 {
        // `FirstOrDefault` on a list of structs: no conflict reads as
        // `default(Rect)`, (0, 0, 0, 0), which the zero-size test catches.
        let conflict = others
            .iter()
            .copied()
            .find(|r| layout_geometry::overlap(r, &rect))
            .unwrap_or_default();
        if conflict.is_empty() || conflict.width() == 0.0 && conflict.height() == 0.0 {
            break;
        }

        let moves = [
            (conflict.x() - rect.right(), true),
            (conflict.right() - rect.x(), true),
            (conflict.y() - rect.bottom(), false),
            (conflict.bottom() - rect.y(), false),
        ];
        // `OrderBy(|amount|).First()`: the first of the smallest.
        let mut best = moves[0];
        for m in &moves[1..] {
            if compare(m.0.abs(), best.0.abs()).is_lt() {
                best = *m;
            }
        }
        if best.1 {
            rect.set_x(rect.x() + best.0);
        } else {
            rect.set_y(rect.y() + best.0);
        }
    }
    rect
}
