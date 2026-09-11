//! What the ported xUnit tests lean on from .NET and xUnit themselves.
#![allow(dead_code)]

/// `Math.Round(double, int digits)` (half to even): scale by the power of ten,
/// round, scale back, and only below 1e16, where .NET leaves larger values alone.
pub fn round_digits(value: f64, digits: usize) -> f64 {
    const POWERS: [f64; 16] = [
        1e0, 1e1, 1e2, 1e3, 1e4, 1e5, 1e6, 1e7, 1e8, 1e9, 1e10, 1e11, 1e12, 1e13, 1e14, 1e15,
    ];
    if value.abs() < 1e16 {
        let power10 = POWERS[digits];
        (value * power10).round_ties_even() / power10
    } else {
        value
    }
}

/// xUnit's `Assert.Equal(double expected, double actual, int precision)`: both
/// rounded to `precision` decimals with `Math.Round`, then compared with
/// `double.Equals` (NaN equals NaN).
#[track_caller]
pub fn assert_equal_precision(expected: f64, actual: f64, precision: usize) {
    let e = round_digits(expected, precision);
    let a = round_digits(actual, precision);
    assert!(
        e == a || e.is_nan() && a.is_nan(),
        "Assert.Equal() failure: expected {expected} ({e} at {precision} digits), actual {actual} ({a})"
    );
}
