//! The .NET `System.Math` functions whose semantics differ from Rust's `f64`
//! methods of the same name. Every port of C# arithmetic goes through these.
//!
//! - `Math.Min`/`Math.Max` follow IEEE 754:2019 `minimum`/`maximum`: a NaN
//!   operand propagates, and -0 is less than +0. Rust's `f64::min`/`max` return
//!   the non-NaN operand instead.
//! - `Math.Round(x)` rounds half to even (`MidpointRounding.ToEven`). Rust's
//!   `f64::round` rounds half away from zero; that is `Math.Round(x,
//!   MidpointRounding.AwayFromZero)`.
//!
//! The rest matches without help: `+ - * /` and `Math.Sqrt` are IEEE-exact on
//! both sides, and since .NET 9 a double-to-int cast saturates and maps NaN to 0,
//! exactly like Rust's `as`.

/// `Math.Min(double, double)`.
pub fn min(a: f64, b: f64) -> f64 {
    if a != b {
        if !a.is_nan() {
            return if a < b { a } else { b };
        }
        return a;
    }
    if a.is_sign_negative() {
        a
    } else {
        b
    }
}

/// `Math.Max(double, double)`.
pub fn max(a: f64, b: f64) -> f64 {
    if a != b {
        if !a.is_nan() {
            return if b < a { a } else { b };
        }
        return a;
    }
    if b.is_sign_negative() {
        a
    } else {
        b
    }
}

/// `Math.Round(double)`: half to even.
pub fn round(x: f64) -> f64 {
    x.round_ties_even()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nan_propagates_like_dotnet() {
        assert!(min(f64::NAN, 1.0).is_nan());
        assert!(min(1.0, f64::NAN).is_nan());
        assert!(max(f64::NAN, 1.0).is_nan());
        assert!(max(1.0, f64::NAN).is_nan());
        // ...where Rust's own methods drop it.
        assert_eq!(1.0f64.max(f64::NAN), 1.0);
    }

    #[test]
    fn negative_zero_is_the_smaller_zero() {
        assert!(min(0.0, -0.0).is_sign_negative());
        assert!(min(-0.0, 0.0).is_sign_negative());
        assert!(max(0.0, -0.0).is_sign_positive());
        assert!(max(-0.0, 0.0).is_sign_positive());
    }

    #[test]
    fn round_is_bankers() {
        assert_eq!(round(0.5), 0.0);
        assert_eq!(round(1.5), 2.0);
        assert_eq!(round(2.5), 2.0);
        assert_eq!(round(-2.5), -2.0);
        assert_eq!(round(2.6), 3.0);
    }
}
