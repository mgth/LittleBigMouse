//! `DimensionReactiveContractTests.cs`: the ReactiveUI behaviour behind the
//! dimension contracts — a derived value follows its sources through every view
//! and every node of a chain.
//!
//! In Rust a derived value is computed from its sources when it is read, so what
//! a C# subscription collects is what reading after each edit gives:
//! `common::observe` keeps the value read at subscription time, then each
//! distinct change, as `WhenAnyValue` does. Which view a value is read through,
//! and which property-change names fire, have no Rust counterpart beyond that.
//!
//! The C# `PhysicalSize()` helper also sets the size's `X = 1, Y = 2`. The model
//! size's location has no Rust setter (production never writes it), and no
//! assertion here reads it: the chain below takes its location from `located`.

mod common;

use common::{add_with_source, assert_equal_precision, design_options, observe};
use lbm_layout::geo::{Point, Rect, Thickness};
use lbm_layout::model::{
    DisplaySize, DisplaySource, Layout, MmSize, Monitor, MonitorModel, PhysicalSource, Ratio,
    PER_MODEL, PER_MONITOR,
};

/// `PhysicalSize()`: 300 x 200 mm, borders 3, 4, 5, 6 (left, top, right, bottom).
fn physical_size() -> MmSize {
    let mut size = MmSize::default();
    size.set_width(300.0);
    size.set_height(200.0);
    size.set_left_border(3.0);
    size.set_top_border(4.0);
    size.set_right_border(5.0);
    size.set_bottom_border(6.0);
    size
}

/// C#: `DimensionReactiveContractTests.MutablePropertyIsObservedThroughConcreteBaseMutableAndReadOnlyViews`.
#[test]
fn mutable_property_is_observed_through_concrete_base_mutable_and_read_only_views() {
    // The concrete, base and mutable views are all the one `MmSize` in Rust; the
    // read-only `IDisplaySize` view is the `DisplaySize` it hands out.
    let mut concrete = physical_size();

    let mut concrete_values = Vec::new();
    let mut read_only_values = Vec::new();
    observe(&mut concrete_values, concrete.width());
    observe(&mut read_only_values, concrete.as_display_size().width);

    concrete.set_width(640.0);
    observe(&mut concrete_values, concrete.width());
    observe(&mut read_only_values, concrete.as_display_size().width);

    let expected = vec![300.0, 640.0];
    assert_eq!(expected, concrete_values);
    assert_eq!(expected, read_only_values);
}

/// C#: `DimensionReactiveContractTests.GetterOnlyPixelBordersDoNotPreventCalculatedPropertiesFromUpdating`.
#[test]
fn getter_only_pixel_borders_do_not_prevent_calculated_properties_from_updating() {
    // `new DisplaySizeInPixels(rect)`: a `DisplaySize` without borders.
    let mut pixels = DisplaySize::from_rect(Rect::new(10.0, 20.0, 1920.0, 1080.0));

    let mut borders = Vec::new();
    let mut outside_widths = Vec::new();
    let mut outside_bounds = Vec::new();
    let mut watch = |p: &DisplaySize| {
        observe(&mut borders, p.left_border);
        observe(&mut outside_widths, p.outside_width());
        observe(&mut outside_bounds, p.outside_bounds());
    };
    watch(&pixels);

    pixels.width = 2560.0;
    watch(&pixels);
    pixels.x = 30.0;
    watch(&pixels);

    assert_eq!(vec![0.0], borders);
    assert_eq!(vec![1920.0, 2560.0], outside_widths);
    assert_eq!(
        Rect::new(30.0, 20.0, 2560.0, 1080.0),
        *outside_bounds.last().unwrap()
    );
    assert!(outside_bounds.len() >= 3);
}

/// C#: `DimensionReactiveContractTests.SourceAndRatioChangesPropagateAcrossTheFullDecoratorChain`.
#[test]
fn source_and_ratio_changes_propagate_across_the_full_decorator_chain() {
    // `new DisplayLocate(new DisplayScale(new DisplayRotate(source, 1), ratio),
    // new Point(50, 60))`, read after every edit of the source, the ratio and the
    // located node's own position.
    let mut source = physical_size();
    let mut ratio = Ratio::new(2.0, 3.0);
    let mut location = Point::new(50.0, 60.0);
    let located = |source: &MmSize, ratio: Ratio, location: Point| {
        source
            .as_display_size()
            .rotate(1)
            .scale(ratio)
            .located(location)
    };

    let mut widths = Vec::new();
    let mut heights = Vec::new();
    let mut bounds = Vec::new();
    let mut outside_bounds = Vec::new();
    let mut watch = |view: DisplaySize| {
        observe(&mut widths, view.width);
        observe(&mut heights, view.height);
        observe(&mut bounds, view.bounds());
        observe(&mut outside_bounds, view.outside_bounds());
    };
    watch(located(&source, ratio, location));

    source.set_height(250.0);
    watch(located(&source, ratio, location));
    source.set_width(350.0);
    watch(located(&source, ratio, location));
    ratio.x = 4.0;
    watch(located(&source, ratio, location));
    ratio.y = 5.0;
    watch(located(&source, ratio, location));
    location.x = 75.0;
    watch(located(&source, ratio, location));
    location.y = 85.0;
    watch(located(&source, ratio, location));

    let read_only_view = located(&source, ratio, location);
    assert_equal_precision(1000.0, read_only_view.width, 10);
    assert_equal_precision(1750.0, read_only_view.height, 10);
    assert_eq!(
        Rect::new(75.0, 85.0, 1000.0, 1750.0),
        read_only_view.bounds()
    );
    assert_eq!(
        Rect::new(59.0, 60.0, 1040.0, 1790.0),
        read_only_view.outside_bounds()
    );
    assert_eq!(read_only_view.width, *widths.last().unwrap());
    assert_eq!(read_only_view.height, *heights.last().unwrap());
    assert_eq!(read_only_view.bounds(), *bounds.last().unwrap());
    assert_eq!(
        read_only_view.outside_bounds(),
        *outside_bounds.last().unwrap()
    );
    // `changedProperties` holds Width, Height, Bounds and OutsideBounds: each of
    // them changed along the way.
    assert!(widths.len() > 1);
    assert!(heights.len() > 1);
    assert!(bounds.len() > 1);
    assert!(outside_bounds.len() > 1);
}

/// C#: `DimensionReactiveContractTests.ReadOnlyCalculatedRatiosRemainReactiveThroughTheirContracts`.
#[test]
fn read_only_calculated_ratios_remain_reactive_through_their_contracts() {
    // `DisplayRatioRatio(first, second)` and `DisplayInverseRatio(first)`, read
    // after every edit. The concrete, base and contract views of the product are
    // the one value in Rust.
    let mut first = Ratio::new(2.0, 3.0);
    let mut second = Ratio::new(5.0, 7.0);

    let mut product_values = Vec::new();
    let mut inverse_values = Vec::new();
    let mut watch = |first: Ratio, second: Ratio| {
        observe(&mut product_values, first.multiply(second).x);
        observe(&mut inverse_values, first.inverse().x);
    };
    watch(first, second);

    first.x = 3.0;
    watch(first, second);
    second.x = 11.0;
    watch(first, second);
    // `first.Set(4, 6)`: X, then Y.
    first.x = 4.0;
    first.y = 6.0;
    watch(first, second);

    assert_eq!(vec![10.0, 15.0, 33.0, 44.0], product_values);
    assert_eq!(vec![0.5, 1.0 / 3.0, 0.25], inverse_values);
}

/// C#: `DimensionReactiveContractTests.MonitorGeometryRewiresWhenTheEffectiveDimensionInstanceChanges`.
#[test]
fn monitor_geometry_rewires_when_the_effective_dimension_instance_changes() {
    let mut layout = Layout::new(design_options());
    let model = layout
        .get_or_add_model("MODEL", |code| {
            let mut model = MonitorModel::new(code);
            let size = &mut model.physical_size;
            size.set_width(600.0);
            size.set_height(340.0);
            size.set_left_border(10.0);
            size.set_top_border(11.0);
            size.set_right_border(12.0);
            size.set_bottom_border(13.0);
            model
        })
        .clone();

    let monitor = Monitor::new("MONITOR", &model);
    let mut source = DisplaySource::new("SOURCE");
    source.attached_to_desktop = true;
    source.in_pixel = DisplaySize::from_rect(Rect::new(0.0, 0.0, 1920.0, 1080.0));
    let physical_source = PhysicalSource::new("DEVICE", "MONITOR", source);
    // C# never hands the monitor to the layout. A Rust monitor's geometry is
    // computed by the layout holding it, its source attached, not registered.
    add_with_source(&mut layout, monitor, physical_source, false);

    let monitor = |layout: &Layout| layout.monitor("MONITOR").unwrap().clone();
    let effective = |layout: &Layout| layout.effective_physical_size(&monitor(layout));
    let model_size = |layout: &Layout| {
        layout
            .model("MODEL")
            .unwrap()
            .physical_size
            .as_display_size()
    };
    let set_border_values = |layout: &mut Layout, value: &str| {
        layout.edit_options(|o| o.border_values = value.to_owned());
    };
    let set_borders = |layout: &mut Layout, edit: &dyn Fn(&mut Thickness)| {
        let mut borders = monitor(layout).borders();
        edit(&mut borders);
        layout.set_monitor_borders("MONITOR", borders);
    };

    // `monitor.WhenAnyValue(e => e.DepthProjection.OutsideWidth)`.
    let mut outside_widths = Vec::new();
    let watch = |layout: &Layout, outside_widths: &mut Vec<f64>| {
        let projection = layout.depth_projection(&monitor(layout)).unwrap();
        observe(outside_widths, projection.outside_width());
    };
    watch(&layout, &mut outside_widths);

    // `Assert.Same(model.PhysicalSize, monitor.EffectivePhysicalSize)`.
    assert_eq!(model_size(&layout), effective(&layout));
    assert_eq!(622.0, *outside_widths.last().unwrap());

    set_border_values(&mut layout, PER_MONITOR);
    watch(&layout, &mut outside_widths);
    // `Assert.NotSame(model.PhysicalSize, perMonitorSize)` is an identity: the
    // per-monitor size is the model's dimensions under the monitor's borders,
    // which still mirror the model's and read alike.
    assert_eq!(
        model_size(&layout).with_borders(monitor(&layout).borders()),
        effective(&layout)
    );
    set_borders(&mut layout, &|b| b.left = 40.0);
    watch(&layout, &mut outside_widths);
    assert_eq!(652.0, *outside_widths.last().unwrap());

    let active_branch_emission_count = outside_widths.len();
    layout.edit_model("MODEL", |size, _| {
        size.set_left_border(15.0);
    });
    watch(&layout, &mut outside_widths);
    assert_eq!(active_branch_emission_count, outside_widths.len());

    set_border_values(&mut layout, PER_MODEL);
    watch(&layout, &mut outside_widths);
    assert_eq!(model_size(&layout), effective(&layout));
    assert_eq!(627.0, *outside_widths.last().unwrap());

    let active_branch_emission_count = outside_widths.len();
    set_borders(&mut layout, &|b| b.right = 50.0);
    watch(&layout, &mut outside_widths);
    assert_eq!(active_branch_emission_count, outside_widths.len());

    set_border_values(&mut layout, PER_MONITOR);
    watch(&layout, &mut outside_widths);
    // `Assert.Same(perMonitorSize, monitor.EffectivePhysicalSize)`: the same
    // borders holder, now 40 and 50 on the sides.
    assert_eq!(
        model_size(&layout).with_borders(monitor(&layout).borders()),
        effective(&layout)
    );
    assert_eq!(690.0, *outside_widths.last().unwrap());
    // C# ends on `Assert.Equal(4, effectiveSizes.Count)`: the instances its
    // stream published (model, override, model, override). A Rust size is a value
    // computed on demand; there is no instance to count.
}
