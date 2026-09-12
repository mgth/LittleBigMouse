//! `EdgeProjection` (in `LayoutGeometry.cs`): the one invariant the two placement
//! directions share, and the two ways to solve it.
//!
//! When two monitors meet along an edge, their position on the perpendicular axis
//! is fixed by requiring that the physical midpoint of the span their panels share
//! maps to the same coordinate on both. "Place from system" knows both pixel
//! positions and solves for the millimetre one ([`millimetre_origin`]); "apply to
//! system" knows both millimetre positions and solves for the pixel one
//! ([`pixel_origin`]).

use super::AxisProfile;

/// `EdgeProjection.SettlePasses`: the millimetre-space solve is iterated this many
/// times at most.
const SETTLE_PASSES: i32 = 4;

/// `EdgeProjection.Settled`: a pass moving the estimate less than this ends the
/// iteration.
const SETTLED: f64 = 1e-9;

/// `EdgeProjection.PixelOrigin`: the pixel origin `target` must take for the
/// invariant to hold against `anchor`, whose own pixel origin is known. Closed
/// form, unrounded.
pub fn pixel_origin(anchor: AxisProfile, target: AxisProfile) -> f64 {
    let mid = anchor.mm.shared_midpoint(target.mm);
    anchor.to_pixel(mid) - (mid - target.mm.lo) / target.pitch()
}

/// `EdgeProjection.MillimetreOrigin`: the millimetre origin `target` must take,
/// both pixel origins being known. Seeded from the pixel-space midpoint, then
/// settled onto the millimetre-space one in at most four passes.
pub fn millimetre_origin(anchor: AxisProfile, target: AxisProfile) -> f64 {
    let mid_pixel = anchor.pixel.shared_midpoint(target.pixel);
    let mut mm = anchor.to_mm(mid_pixel) - (mid_pixel - target.pixel.lo) * target.pitch();

    for _ in 0..SETTLE_PASSES {
        let mid = anchor.mm.shared_midpoint(target.at_mm(mm).mm);
        let next = mid - (anchor.to_pixel(mid) - target.pixel.lo) * target.pitch();

        if (next - mm).abs() < SETTLED {
            break;
        }
        mm = next;
    }

    mm
}
