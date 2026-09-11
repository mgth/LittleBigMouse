//! `SystemLocationSolver.cs`: derive the physical (mm) layout from the pixel
//! configuration the system reports. Walks out from the primary over pixel-space
//! edge adjacency and turns each contact into a millimetre position. The inverse
//! of [`super::pixel_location`].
//!
//! It does not close the gaps it leaves: monitors with no pixel adjacency to
//! anything placed keep whatever their alignment hints gave them, and pulling them
//! into the block is [`super::compaction`]'s job, which the caller runs after.

use std::collections::{HashSet, VecDeque};

use super::edge_projection;
use super::layout_geometry::{self, Axis, EdgeContact, LayoutGeometryRect, MonitorSnapshot};
use super::IdMap;
use crate::geo::Point;

/// `SystemLocationSolver.PixelTolerance`: pixel edges are integers the system
/// reports verbatim, so two monitors are adjacent when their edges are equal.
pub const PIXEL_TOLERANCE: f64 = 0.0;

/// `SystemLocationSolver.Solve`: millimetre panel origins, keyed by monitor id in
/// the order of `monitors`, for the monitors that actually move.
///
/// `monitors` is every monitor of the layout, any of which can anchor. `to_place`
/// holds the ids allowed to move, `None` for all of them; the primary anchors the
/// walk whether it is in there or not. Nothing moves without a primary.
///
/// Ids must be unique: C# builds its position table with `ToDictionary`, which
/// throws on a duplicate; here that is a debug assertion.
pub fn solve(monitors: &[MonitorSnapshot], to_place: Option<&HashSet<String>>) -> IdMap<Point> {
    let mut result = IdMap::new();

    let Some(primary) = monitors.iter().find(|m| m.primary) else {
        return result;
    };

    // Where every monitor currently sits; a monitor no rule fires on keeps it.
    let mut positions = IdMap::new();
    for m in monitors {
        debug_assert!(
            !positions.contains_key(&m.id),
            "duplicate monitor id {:?}",
            m.id
        );
        positions.insert(&m.id, m.mm_location());
    }

    let mut unplaced: Vec<&MonitorSnapshot> = monitors
        .iter()
        .filter(|m| to_place.is_none_or(|ids| ids.contains(&m.id)))
        .collect();

    if unplaced.is_empty() {
        return result;
    }

    // Start with the primary display and walk outwards.
    let mut todo: VecDeque<&MonitorSnapshot> = VecDeque::new();
    todo.push_back(primary);

    loop {
        for monitor in &todo {
            unplaced.retain(|m| m.id != monitor.id);
        }

        let Some(placed) = todo.pop_front() else {
            break;
        };
        let placed_at = positions[placed.id.as_str()];

        for target in unplaced.clone() {
            if target.id == placed.id {
                continue;
            }

            let (position, adjacent) =
                against(placed, placed_at, target, positions[target.id.as_str()]);

            // Hints fire whether or not the monitor is claimed.
            positions.insert(&target.id, position);

            if !adjacent {
                continue;
            }

            unplaced.retain(|m| m.id != target.id);

            todo.push_back(target);
        }
    }

    for monitor in monitors {
        let position = positions[monitor.id.as_str()];
        if position != monitor.mm_location() {
            result.insert(&monitor.id, position);
        }
    }

    result
}

/// `SystemLocationSolver.Against`: everything one already-placed monitor says about
/// where another goes, and whether that claims it. Contact rule, then alignment
/// hints, then edge projection, each overwriting what the previous one set.
fn against(
    placed: &MonitorSnapshot,
    placed_at: Point,
    target: &MonitorSnapshot,
    current: Point,
) -> (Point, bool) {
    // The anchor at its solved position; of the target only sizes and bezels are read.
    let anchor = placed.moved_to(placed_at);

    let mut position = current;
    let horizontal = claim(&anchor, target, Axis::Horizontal, &mut position);
    let vertical = claim(&anchor, target, Axis::Vertical, &mut position);

    // The offset along the contact edge, on the axis the contact is not on. Last,
    // so it overrides the alignment equalities of `claim`.
    if horizontal {
        project(&anchor, target, Axis::Vertical, &mut position);
    }
    if vertical {
        project(&anchor, target, Axis::Horizontal, &mut position);
    }

    (position, horizontal || vertical)
}

/// `SystemLocationSolver.Claim`: the contact rule and the two alignment hints on
/// one axis. True only for a crossable contact (the two straddle the shared edge),
/// which is what claims the monitor; a corner contact still suggests a position.
fn claim(
    anchor: &MonitorSnapshot,
    target: &MonitorSnapshot,
    axis: Axis,
    position: &mut Point,
) -> bool {
    // Contact: flush against the anchor's bezel edge, plus the target's own bezel.
    let contact = layout_geometry::contact_between_rects(
        &anchor.pixel_bounds,
        &target.pixel_bounds,
        axis,
        PIXEL_TOLERANCE,
    );

    let crossable = layout_geometry::overlap_on(
        &anchor.pixel_bounds,
        &target.pixel_bounds,
        axis.perpendicular(),
    ) > 0.0;

    let outside = anchor.mm_outside_bounds.on(axis);

    *position = match contact {
        EdgeContact::After => with(*position, axis, outside.hi() + target.low_border(axis)),
        EdgeContact::Before => with(
            *position,
            axis,
            outside.lo - target.mm_outside_bounds.on(axis).size + target.low_border(axis),
        ),
        EdgeContact::None => *position,
    };

    // Alignment hints, panel edge to panel edge; they override the contact rule.
    let anchor_pixel = anchor.pixel_bounds.on(axis);
    let target_pixel = target.pixel_bounds.on(axis);
    let anchor_panel = anchor.mm_bounds.on(axis);

    if target_pixel.lo == anchor_pixel.lo {
        *position = with(*position, axis, anchor_panel.lo);
    }
    if target_pixel.hi() == anchor_pixel.hi() {
        *position = with(
            *position,
            axis,
            anchor_panel.hi() - target.mm_bounds.on(axis).size,
        );
    }

    contact != EdgeContact::None && crossable
}

/// `SystemLocationSolver.Project`: solve the shared-midpoint invariant along `axis`,
/// unless either monitor reports no pixels on it.
fn project(anchor: &MonitorSnapshot, target: &MonitorSnapshot, axis: Axis, position: &mut Point) {
    let anchor_profile = anchor.profile(axis);
    let target_profile = target.profile(axis);

    if !anchor_profile.has_pixels() || !target_profile.has_pixels() {
        return;
    }

    *position = with(
        *position,
        axis,
        edge_projection::millimetre_origin(anchor_profile, target_profile),
    );
}

/// `SystemLocationSolver.With`: `point` with its coordinate on `axis` replaced.
fn with(point: Point, axis: Axis, value: f64) -> Point {
    if axis == Axis::Horizontal {
        Point::new(value, point.y)
    } else {
        Point::new(point.x, value)
    }
}
