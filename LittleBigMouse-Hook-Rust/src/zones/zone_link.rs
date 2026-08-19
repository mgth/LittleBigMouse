//! Port of `Engine/ZoneLink`.
//!
//! The C++ singly-linked list (`ZoneLink* Next`) becomes a `Vec<ZoneLink>` per
//! side; the `Target` raw pointer becomes an `Option<ZoneId>` resolved from
//! `TargetId` at layout init.
//!
//! Border resistance is a *pair* indexed by [`MODE_MOVE`] / [`MODE_DRAG`]: the
//! engine indexes it with a bool instead of branching on the mode, so telling
//! plain moves apart from drags costs nothing on the hot path (#389). An
//! infinite resistance means "blocked": `INFINITY - distance` never drops to
//! zero, and its pixel counterpart saturates on `i64::MAX`, so a wall is just
//! the limit case of a resistance — no flag, no extra test.

use super::ZoneId;

/// Index of the plain-move resistance in [`ZoneLink::border_resistance`].
pub const MODE_MOVE: usize = 0;
/// Index of the drag resistance (at least one mouse button held).
pub const MODE_DRAG: usize = 1;

#[derive(Debug, Clone)]
pub struct ZoneLink {
    /// Resolved from `target_id` during `ZonesLayout::init`; `None` for the
    /// catch-all links whose `target_id` is `-1`.
    pub target: Option<ZoneId>,
    pub target_id: i32,
    pub from: f64,
    pub to: f64,
    pub source_from_px: i32,
    pub source_to_px: i32,
    pub target_from_px: i32,
    pub target_to_px: i32,
    /// `[move, drag]`, in mm — the Cross path.
    pub border_resistance: [f64; 2],
    /// `[move, drag]`, in pixels — the Strait path. `i64` (not `i32`) so a
    /// blocked border can saturate on `i64::MAX` and still be drained by the
    /// engine's plain subtraction without ever reaching zero.
    pub border_resistance_px: [i64; 2],
}

/// Physical resistance -> pixel resistance along one link.
fn to_px(resistance: f64, from: f64, to: f64, source_from_px: i32, source_to_px: i32) -> i64 {
    if resistance <= 0.0 {
        return 0;
    }
    // A blocked link must not go through the ratio below: catch-all links span
    // `f64::MIN..f64::MAX`, so `to - from` is already `INFINITY` and the division
    // would yield `NaN` — which casts to 0 in Rust, silently unblocking the border.
    if !resistance.is_finite() {
        return i64::MAX;
    }
    // i64 like to_target_pixel: catch-all links carry i32::MIN/MAX sentinel
    // bounds, so the i32 subtraction overflows once a resistance is set.
    ((resistance / (to - from)) * (source_to_px as i64 - source_from_px as i64) as f64) as i64
}

impl ZoneLink {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        from: f64,
        to: f64,
        source_from_px: i32,
        source_to_px: i32,
        target_from_px: i32,
        target_to_px: i32,
        move_resistance: f64,
        drag_resistance: f64,
        target_id: i32,
    ) -> Self {
        let normalize = |r: f64| if r <= 0.0 { 0.0 } else { r };

        ZoneLink {
            target: None,
            target_id,
            from,
            to,
            source_from_px,
            source_to_px,
            target_from_px,
            target_to_px,
            border_resistance: [normalize(move_resistance), normalize(drag_resistance)],
            border_resistance_px: [
                to_px(move_resistance, from, to, source_from_px, source_to_px),
                to_px(drag_resistance, from, to, source_from_px, source_to_px),
            ],
        }
    }

    /// C++ `ZoneLink::ToTargetPixel`. All arithmetic in `i64`: the catch-all links carry
    /// `i32::MIN`/`i32::MAX` sentinel bounds, so even the *subtractions* (`target_to - target_from`,
    /// `v - source_from`) overflow `i32` and panic in a debug build — casting the i32 *result* to
    /// i64, as before, was too late. Operands are widened to i64 BEFORE subtracting.
    pub fn to_target_pixel(&self, v: i32) -> i32 {
        let s_len = self.source_to_px as i64 - self.source_from_px as i64;
        let t_len = self.target_to_px as i64 - self.target_from_px as i64;
        if s_len == 0 {
            return self.target_from_px;
        }
        ((v as i64 - self.source_from_px as i64) * t_len / s_len + self.target_from_px as i64)
            as i32
    }
}

/// C++ `ZoneLink::AtPhysical` — index of the first link covering physical
/// position `pos`. Returns the index (rather than a reference) so the caller can
/// identify the link for border-resistance tracking.
///
/// Well-formed layouts terminate each side with a catch-all link (`to == +inf`),
/// so the walk always stops. Unlike the C++ (which would walk off the list — a
/// null deref on a link-less side), this clamps to the last link and returns
/// `None` for an empty list, so a malformed layout can't crash the daemon.
pub fn at_physical_index(links: &[ZoneLink], pos: f64) -> Option<usize> {
    if links.is_empty() {
        return None;
    }
    let mut i = 0;
    while i + 1 < links.len() && pos >= links[i].to {
        i += 1;
    }
    Some(i)
}

/// C++ `ZoneLink::AtPixel` — index of the first link covering pixel position `pos`.
pub fn at_pixel_index(links: &[ZoneLink], pos: i32) -> Option<usize> {
    if links.is_empty() {
        return None;
    }
    let mut i = 0;
    while i + 1 < links.len() && pos >= links[i].source_to_px {
        i += 1;
    }
    Some(i)
}
