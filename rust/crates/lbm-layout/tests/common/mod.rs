//! What the ported xUnit tests lean on from .NET and xUnit themselves, and the
//! construction steps the C# model tests share.
#![allow(dead_code)]

use lbm_layout::model::{Layout, LayoutOptions, Monitor, MonitorModel, PhysicalSource};

/// `new ILayoutOptions.Design()`: `LbmOptions`' defaults, but for the members the
/// design-time class initializes otherwise.
pub fn design_options() -> LayoutOptions {
    LayoutOptions {
        enabled: true,
        auto_update: true,
        elevated: true,
        excluded_list: vec!["/game/".to_owned(), "/another/game/".to_owned()],
        ..LayoutOptions::default()
    }
}

/// `new PhysicalMonitorModel(pnp)` with `PhysicalSize.Width` and `Height` set,
/// then the four borders when given (left, top, right, bottom, like the C#
/// helpers write them; the model's own default is 20 mm). The Rust layout keeps
/// its models, so a PnP code already there is reused as is.
pub fn model(
    layout: &mut Layout,
    pnp: &str,
    width: f64,
    height: f64,
    borders: Option<f64>,
) -> MonitorModel {
    layout
        .get_or_add_model(pnp, |code| {
            let mut m = MonitorModel::new(code);
            m.physical_size.set_width(width);
            m.physical_size.set_height(height);
            if let Some(b) = borders {
                m.physical_size.set_left_border(b);
                m.physical_size.set_top_border(b);
                m.physical_size.set_right_border(b);
                m.physical_size.set_bottom_border(b);
            }
            m
        })
        .clone()
}

/// The tail of every C# test helper: `monitor.ActiveSource = physicalSource;
/// monitor.Sources.Add(physicalSource); layout.AddOrUpdatePhysicalMonitor(monitor);`
/// then, when `register`, `layout.AddOrUpdatePhysicalSource(physicalSource)`.
pub fn add_with_source(
    layout: &mut Layout,
    mut monitor: Monitor,
    source: PhysicalSource,
    register: bool,
) {
    let id = source.source.id.clone();
    monitor.active_source = Some(id.clone());
    monitor.sources.push(id);
    layout.attach_source(source.clone());
    layout.add_or_update_monitor(monitor);
    if register {
        layout.add_or_update_source(source);
    }
}

/// What a `WhenAnyValue` subscription collects: the value when subscribing, then
/// each distinct change. Call it after every edit.
pub fn observe<T: PartialEq>(log: &mut Vec<T>, value: T) {
    if log.last() != Some(&value) {
        log.push(value);
    }
}

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
