//! `DimensionContractTests.cs`: what each dimension type lets a caller change.
//!
//! In C# a size is a node of a reactive chain, and writing a derived node writes
//! its source back through the inverse transform. In Rust a size is a value and
//! each C# node a method returning a new one (`DisplaySize::rotate`, `scale`,
//! `located`, `with_borders`, `scale_dip`); the model's state is changed only
//! through the layout's edits. The tests about values are ported; the ones about
//! write-back through a node are not, the feature being absent:
//! - `PhysicalDimensionImplementationsExposeWorkingFullMutationContracts` writes
//!   all eight members through `DisplayBorderOverride`, `DisplayLocate`,
//!   `DisplayRotate`, `DisplayScale`, `DisplayScaleWithLocation` and
//!   `DisplayTranslate` and reads them back. Production writes back through a
//!   chain in two places, both unported: the size editor
//!   (`SizeViewModel` writes `Model.PhysicalRotated.*`) and the
//!   `PhysicalSource.RealPitch` setter.
//! - `PixelScaleImplementationsKeepOnlyTheSupportedMutationContract`:
//!   `DisplaySizeWpf` has no Rust counterpart, and no caller in production.
//! - `DipScaleExposesMutableBoundsButNotFixedPixelBorders` writes the pixel rect
//!   back through `DisplayScaleDip`; production only reads `InDip`, and so does
//!   the Rust `Layout::in_dip`.

mod common;

use common::{assert_equal_precision, design_options, model, observe};
use lbm_layout::geo::{Point, Rect, Size, Thickness};
use lbm_layout::model::{DisplaySize, DisplaySource, Layout, Monitor, PhysicalSource, Ratio};

/// C#: `DimensionContractTests.PixelDimensionsExposeMutableBoundsAndReadOnlyZeroBorders`.
#[test]
fn pixel_dimensions_expose_mutable_bounds_and_read_only_zero_borders() {
    // `new DisplaySizeInPixels(rect)` is a source's `InPixel`, and `Set(rect)` the
    // layout's `set_source_in_pixel`. C# also pins at the type level that the
    // borders cannot be written (`pixels is not IMutableDisplaySize`); the Rust
    // pixel size is a plain `DisplaySize`, and the one edit the model offers on it
    // writes the rectangle, never the borders.
    let mut layout = Layout::new(design_options());
    let mut source = DisplaySource::new("SOURCE");
    source.in_pixel = DisplaySize::from_rect(Rect::from_location_size(
        Point::new(10.0, 20.0),
        Size::new(1920.0, 1080.0),
    ));
    layout.attach_source(PhysicalSource::new("DEVICE", "MONITOR", source));
    let pixels = |layout: &Layout| layout.source("SOURCE").unwrap().source.in_pixel;

    let mut observed_widths = Vec::new();
    observe(&mut observed_widths, pixels(&layout).width);

    layout.set_source_in_pixel(
        "SOURCE",
        Rect::from_location_size(Point::new(30.0, 40.0), Size::new(2560.0, 1440.0)),
    );
    observe(&mut observed_widths, pixels(&layout).width);

    let pixels = pixels(&layout);
    assert_eq!(
        Rect::from_location_size(Point::new(30.0, 40.0), Size::new(2560.0, 1440.0)),
        pixels.bounds()
    );
    assert_eq!(0.0, pixels.left_border);
    assert_eq!(0.0, pixels.top_border);
    assert_eq!(0.0, pixels.right_border);
    assert_eq!(0.0, pixels.bottom_border);
    assert_eq!(pixels.bounds(), pixels.outside_bounds());
    assert_eq!(vec![1920.0, 2560.0], observed_widths);
}

/// C#: `DimensionContractTests.RatioImplementationsAdvertiseOnlySupportedMutability`.
#[test]
fn ratio_implementations_advertise_only_supported_mutability() {
    // C# also pins that the inverse and the product are not
    // `IMutableDisplayRatio`: in Rust they are values `inverse` and `multiply`
    // return, derived from the ratio and never written back into it.
    let mut value = Ratio::new(2.0, 3.0);
    let other = Ratio::new(5.0, 7.0);

    value.x = 11.0;
    value.y = 13.0;
    let inverse = value.inverse();
    let product = value.multiply(other);

    assert_eq!(11.0, value.x);
    assert_eq!(13.0, value.y);
    assert_equal_precision(1.0 / 11.0, inverse.x, 10);
    assert_equal_precision(1.0 / 13.0, inverse.y, 10);
    assert_eq!(55.0, product.x);
    assert_eq!(91.0, product.y);
}

/// C#: `DimensionContractTests.PerMonitorBordersRejectNegativeValues`.
#[test]
fn per_monitor_borders_reject_negative_values() {
    // `DisplayBorders` is a monitor's own borders, written with
    // `set_monitor_borders`.
    let mut layout = Layout::new(design_options());
    let model = model(&mut layout, "MODEL", 600.0, 340.0, None);
    layout.add_or_update_monitor(Monitor::new("MONITOR", &model));

    layout.set_monitor_borders("MONITOR", Thickness::new(-1.0, -2.0, -3.0, -4.0));

    let borders = layout.monitor("MONITOR").unwrap().borders();
    assert_eq!(0.0, borders.left);
    assert_eq!(0.0, borders.top);
    assert_eq!(0.0, borders.right);
    assert_eq!(0.0, borders.bottom);
}
