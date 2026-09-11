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
//!
//! LINQ orders doubles its own way too, and the ports of `OrderBy`, `Min` and
//! `Max` go through [`compare`], [`enumerable_min`] and [`enumerable_max`].

use std::cmp::Ordering;

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

/// `double.CompareTo(double)`, which is what `Comparer<double>.Default` hands to
/// LINQ's `OrderBy`: NaN is smaller than every other value, negative infinity
/// included, and equal to itself; -0 equals +0. A total order, so it can drive
/// `slice::sort_by`, which is stable like `OrderBy`.
pub fn compare(a: f64, b: f64) -> Ordering {
    if a < b {
        return Ordering::Less;
    }
    if a > b {
        return Ordering::Greater;
    }
    if a == b {
        return Ordering::Equal;
    }
    // At least one of them is NaN.
    match (a.is_nan(), b.is_nan()) {
        (true, true) => Ordering::Equal,
        (true, false) => Ordering::Less,
        _ => Ordering::Greater,
    }
}

/// `Enumerable.Min` over doubles, with or without a selector: the first NaN met
/// is the answer, otherwise the first of the smallest values (compared with `<`,
/// so of -0 and +0 whichever comes first). `None` where LINQ throws on an empty
/// sequence.
pub fn enumerable_min(values: impl IntoIterator<Item = f64>) -> Option<f64> {
    let mut values = values.into_iter();
    let mut value = values.next()?;
    if value.is_nan() {
        return Some(value);
    }
    for x in values {
        if x < value {
            value = x;
        } else if x.is_nan() {
            return Some(x);
        }
    }
    Some(value)
}

/// `Enumerable.Max` over doubles, with or without a selector: NaN only when every
/// value is NaN, otherwise the first of the largest values (compared with `>`).
/// `None` where LINQ throws on an empty sequence.
pub fn enumerable_max(values: impl IntoIterator<Item = f64>) -> Option<f64> {
    let mut values = values.into_iter();
    let mut value = values.next()?;
    while value.is_nan() {
        match values.next() {
            Some(x) => value = x,
            None => return Some(value),
        }
    }
    for x in values {
        if x > value {
            value = x;
        }
    }
    Some(value)
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
    fn compare_puts_nan_first_and_zeros_together() {
        let mut v = [1.0, f64::NAN, f64::NEG_INFINITY, -0.0, 0.0, f64::NAN];
        let tagged: Vec<(f64, usize)> = v.iter().copied().zip(0..).collect();
        let mut sorted = tagged.clone();
        sorted.sort_by(|a, b| compare(a.0, b.0));
        let order: Vec<usize> = sorted.iter().map(|t| t.1).collect();
        // Stable: the two NaNs and the two zeros keep their input order.
        assert_eq!(order, [1, 5, 2, 3, 4, 0]);
        v.sort_by(|a, b| compare(*a, *b));
        assert!(v[0].is_nan() && v[1].is_nan());
    }

    #[test]
    fn enumerable_min_and_max_follow_linq() {
        // Measured on .NET 10: the first extreme wins, Min stops at a NaN, Max skips them.
        assert!(enumerable_min([0.0, -0.0]).unwrap().is_sign_positive());
        assert!(enumerable_min([-0.0, 0.0]).unwrap().is_sign_negative());
        assert!(enumerable_min([1.0, f64::NAN, 0.0]).unwrap().is_nan());
        assert!(enumerable_max([-0.0, 0.0]).unwrap().is_sign_negative());
        assert_eq!(enumerable_max([f64::NAN, 1.0, f64::NAN]), Some(1.0));
        assert!(enumerable_max([f64::NAN, f64::NAN]).unwrap().is_nan());
        assert_eq!(enumerable_min(std::iter::empty()), None);
        assert_eq!(enumerable_max(std::iter::empty()), None);
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
