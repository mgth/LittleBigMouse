//! The Wayland scale quantisation of
//! `MonitorsLocationsToSystemExtensions.ComputePixelLocationsFromPhysical`
//! (`adjustScale: true`), lifted out of the bridge as a pure function.
//!
//! Per-output scales are recomputed so a logical pixel covers the same physical
//! size everywhere (the primary's current logical pitch), quantised to 1/120, the
//! fractional-scale protocol unit KWin rounds to. The bridge then hands the pixel
//! size this returns to [`super::pixel_location::solve`] in place of the monitor's
//! current one.

use crate::geo::dotnet::round;
use crate::geo::Size;

/// The loop body of `ComputePixelLocationsFromPhysical` for one monitor, when
/// `adjustScale` is set: `Some((scale, pixel_size))` when the monitor should move to
/// a new scale, with the logical pixel size it will have then; `None` when the
/// quantised scale is the one the compositor already runs (less than half a
/// protocol step away), in which case the bridge keeps `InPixel.Bounds.Size` and
/// reports no scale.
///
/// - `target_pitch`: the primary's logical pitch, which the bridge computes once as
///   `primary.DepthProjection.Bounds.Width / primary.ActiveSource.Source.InPixel.Width`
///   (so the primary itself comes out unchanged).
/// - `effective_dpi_x`: `source.EffectiveDpi.X`; the current scale is that over 96.
/// - `in_pixel_width`, `in_pixel_height`: `source.InPixel.Width` and `Height`, the
///   current logical size.
/// - `mm_width`: `monitor.DepthProjection.Bounds.Width`.
///
/// The wanted scale is clamped to 0.5..=3 (`Math.Clamp`, which keeps a NaN).
pub fn adjusted_scale(
    target_pitch: f64,
    effective_dpi_x: f64,
    in_pixel_width: f64,
    in_pixel_height: f64,
    mm_width: f64,
) -> Option<(f64, Size)> {
    let scale = effective_dpi_x / 96.0;
    let native = Size::new(in_pixel_width * scale, in_pixel_height * scale);
    let physical_pitch = mm_width / native.width();

    let mut wanted = round(target_pitch / physical_pitch * 120.0) / 120.0;
    wanted = clamp(wanted, 0.5, 3.0);

    // 1/240 = half a protocol step: below that the quantised scale is the one the
    // compositor already runs, don't emit a no-op change.
    if (wanted - scale).abs() >= 1.0 / 240.0 {
        let pixel_size = Size::new(
            round(native.width() / wanted),
            round(native.height() / wanted),
        );
        return Some((wanted, pixel_size));
    }

    None
}

/// `Math.Clamp(double, double, double)` for a valid range: NaN passes through.
fn clamp(value: f64, min: f64, max: f64) -> f64 {
    if value < min {
        min
    } else if value > max {
        max
    } else {
        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `three-screens-mixed-scale` in `domain-oracle/`: a 698 mm primary at
    /// 2560 logical px (scale 1.5), a 527 mm 4K at 1969 px (1.95) and a 697 mm 4K
    /// at 3072 px (1.25). The recorded adjustment moves the first to 1.98333… at
    /// 1936x1089 and the last to 1.5 at 2560x1440; the primary stays put.
    #[test]
    fn matches_the_mixed_scale_oracle() {
        let target_pitch = 698.0 / 2560.0;

        assert_eq!(
            adjusted_scale(target_pitch, 144.0, 2560.0, 1440.0, 698.0),
            None
        );

        let (scale, size) = adjusted_scale(target_pitch, 187.2, 1969.0, 1108.0, 527.0).unwrap();
        assert_eq!(scale, 1.9833333333333334);
        assert_eq!(size, Size::new(1936.0, 1089.0));

        let (scale, size) = adjusted_scale(target_pitch, 120.0, 3072.0, 1728.0, 697.0).unwrap();
        assert_eq!(scale, 1.5);
        assert_eq!(size, Size::new(2560.0, 1440.0));
    }

    #[test]
    fn wanted_scale_is_clamped() {
        // A pitch ten times finer than the target asks for scale 10: clamped to 3.
        let (scale, _) = adjusted_scale(1.0, 96.0, 1000.0, 500.0, 100.0).unwrap();
        assert_eq!(scale, 3.0);
        // And ten times coarser for 0.1: clamped to 0.5.
        let (scale, _) = adjusted_scale(0.1, 96.0, 1000.0, 500.0, 1000.0).unwrap();
        assert_eq!(scale, 0.5);
    }

    #[test]
    fn less_than_half_a_step_is_no_change() {
        // Current scale 1.0; the target asks for 1 + 1/300, which quantises to 1.
        let target_pitch = 1.0 + 1.0 / 300.0;
        assert_eq!(
            adjusted_scale(target_pitch, 96.0, 1000.0, 500.0, 1000.0),
            None
        );
    }
}
