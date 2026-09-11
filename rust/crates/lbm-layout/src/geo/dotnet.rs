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

/// `double.ToString(CultureInfo.InvariantCulture)` since .NET Core 3.0: the
/// shortest string that reads back as the same double, in fixed notation
/// unless the decimal point would sit more than `max(digits, 15)` places right
/// of the first digit or more than three left of it; then `d.dddE+XX`, with
/// at least two exponent digits. `NaN`, `Infinity`, `-Infinity`, and `-0` for
/// negative zero.
pub fn format_double(x: f64) -> String {
    if x.is_nan() {
        return "NaN".to_owned();
    }
    if x.is_infinite() {
        return if x > 0.0 { "Infinity" } else { "-Infinity" }.to_owned();
    }
    if x == 0.0 {
        return if x.is_sign_negative() { "-0" } else { "0" }.to_owned();
    }
    // Rust's `{:e}` prints the same shortest round-trip digits: "d.ddde±x".
    let sci = format!("{:e}", x.abs());
    let (mantissa, exponent) = sci.split_once('e').expect("{:e} has an exponent");
    let digits: String = mantissa.chars().filter(|c| *c != '.').collect();
    let exponent: i32 = exponent.parse().expect("{:e} exponent is an integer");
    let point = exponent + 1;
    let max_digits = (digits.len() as i32).max(15);
    let mut out = String::new();
    if x < 0.0 {
        out.push('-');
    }
    if point > max_digits || point < -3 {
        out.push_str(&digits[..1]);
        if digits.len() > 1 {
            out.push('.');
            out.push_str(&digits[1..]);
        }
        out.push('E');
        out.push(if exponent < 0 { '-' } else { '+' });
        out.push_str(&format!("{:02}", exponent.abs()));
    } else if point <= 0 {
        out.push_str("0.");
        out.push_str(&"0".repeat((-point) as usize));
        out.push_str(&digits);
    } else if point as usize >= digits.len() {
        out.push_str(&digits);
        out.push_str(&"0".repeat(point as usize - digits.len()));
    } else {
        out.push_str(&digits[..point as usize]);
        out.push('.');
        out.push_str(&digits[point as usize..]);
    }
    out
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
    fn doubles_format_like_dotnet() {
        let cases = [
            (f64::MIN, "-1.7976931348623157E+308"),
            (f64::MAX, "1.7976931348623157E+308"),
            (0.0, "0"),
            (-0.0, "-0"),
            (-60.0, "-60"),
            (20.0, "20"),
            (12.5, "12.5"),
            (0.1, "0.1"),
            (0.0001, "0.0001"),
            (0.00001, "1E-05"),
            (1e14, "100000000000000"),
            (1e15, "1E+15"),
            (123456789012345.6, "123456789012345.6"),
            (12345678901234567.0, "12345678901234568"),
            (1.2345678901234568e17, "1.2345678901234568E+17"),
            (0.27447916666666666, "0.27447916666666666"),
            (f64::NAN, "NaN"),
            (f64::INFINITY, "Infinity"),
            (f64::NEG_INFINITY, "-Infinity"),
        ];
        for (x, expected) in cases {
            assert_eq!(format_double(x), expected, "{x:e}");
        }
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
