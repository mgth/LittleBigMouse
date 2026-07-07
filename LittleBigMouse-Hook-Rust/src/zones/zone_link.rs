//! Port of `Engine/ZoneLink`.
//!
//! The C++ singly-linked list (`ZoneLink* Next`) becomes a `Vec<ZoneLink>` per
//! side; the `Target` raw pointer becomes an `Option<ZoneId>` resolved from
//! `TargetId` at layout init.

use super::ZoneId;

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
    pub border_resistance: f64,
    pub border_resistance_px: i32,
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
        border_resistance: f64,
        target_id: i32,
    ) -> Self {
        let mut border_resistance = border_resistance;
        let mut border_resistance_px = 0;
        if border_resistance <= 0.0 {
            border_resistance = 0.0;
        } else {
            border_resistance_px =
                ((border_resistance / (to - from)) * (source_to_px - source_from_px) as f64) as i32;
        }

        ZoneLink {
            target: None,
            target_id,
            from,
            to,
            source_from_px,
            source_to_px,
            target_from_px,
            target_to_px,
            border_resistance,
            border_resistance_px,
        }
    }

    /// C++ `ZoneLink::ToTargetPixel`. Computed via `i64` intermediates to avoid
    /// debug-mode overflow panics; on the finite links this is actually called
    /// on, the result matches the C++ `long` arithmetic exactly.
    pub fn to_target_pixel(&self, v: i32) -> i32 {
        let s_len = (self.source_to_px - self.source_from_px) as i64;
        let t_len = (self.target_to_px - self.target_from_px) as i64;
        (((v - self.source_from_px) as i64 * t_len / s_len) as i32) + self.target_from_px
    }
}

/// C++ `ZoneLink::AtPhysical` — first link covering physical position `pos`.
/// Assumes a terminating catch-all link (`to == +inf`), which the layout always
/// provides.
pub fn at_physical(links: &[ZoneLink], pos: f64) -> &ZoneLink {
    let mut i = 0;
    while pos >= links[i].to {
        i += 1;
    }
    &links[i]
}

/// C++ `ZoneLink::AtPixel` — first link covering pixel position `pos`.
pub fn at_pixel(links: &[ZoneLink], pos: i32) -> &ZoneLink {
    let mut i = 0;
    while pos >= links[i].source_to_px {
        i += 1;
    }
    &links[i]
}
