using DynamicData;
using HLab.Geo;
using LittleBigMouse.DisplayLayout.Dimensions;
using LittleBigMouse.DisplayLayout.Monitors;
using ReactiveUI;

namespace LittleBigMouse.DisplayLayout.Tests;

/// <summary>
/// Guards the ReactiveUI behaviour behind the dimension contracts. In particular, these
/// tests exercise getter-only views and the properties hidden by the mutable interfaces:
/// observing a property must not depend on that property's compile-time setter.
/// </summary>
public class DimensionReactiveContractTests
{
    sealed class ReactiveLayoutOptions : ILayoutOptions.Design
    {
        public void SetBorderValues(string value)
        {
            BorderValues = value;
            OnPropertyChanged(nameof(BorderValues));
        }
    }

    static DisplaySizeInMm PhysicalSize() => new()
    {
        X = 1,
        Y = 2,
        Width = 300,
        Height = 200,
        LeftBorder = 3,
        TopBorder = 4,
        RightBorder = 5,
        BottomBorder = 6,
    };

    [Fact]
    public void MutablePropertyIsObservedThroughConcreteBaseMutableAndReadOnlyViews()
    {
        var concrete = PhysicalSize();
        DisplaySize baseView = concrete;
        IMutableDisplaySize mutableView = concrete;
        IDisplaySize readOnlyView = concrete;

        var concreteValues = new List<double>();
        var baseValues = new List<double>();
        var mutableValues = new List<double>();
        var readOnlyValues = new List<double>();

        using var concreteSubscription = concrete.WhenAnyValue(e => e.Width).Subscribe(concreteValues.Add);
        using var baseSubscription = baseView.WhenAnyValue(e => e.Width).Subscribe(baseValues.Add);
        using var mutableSubscription = mutableView.WhenAnyValue(e => e.Width).Subscribe(mutableValues.Add);
        using var readOnlySubscription = readOnlyView.WhenAnyValue(e => e.Width).Subscribe(readOnlyValues.Add);

        mutableView.Width = 640;

        var expected = new[] { 300d, 640d };
        Assert.Equal(expected, concreteValues);
        Assert.Equal(expected, baseValues);
        Assert.Equal(expected, mutableValues);
        Assert.Equal(expected, readOnlyValues);
    }

    [Fact]
    public void GetterOnlyPixelBordersDoNotPreventCalculatedPropertiesFromUpdating()
    {
        var pixels = new DisplaySizeInPixels(new Rect(10, 20, 1920, 1080));
        IDisplaySize readOnlyView = pixels;

        var borders = new List<double>();
        var outsideWidths = new List<double>();
        var outsideBounds = new List<Rect>();

        using var borderSubscription = readOnlyView.WhenAnyValue(e => e.LeftBorder).Subscribe(borders.Add);
        using var widthSubscription = readOnlyView.WhenAnyValue(e => e.OutsideWidth).Subscribe(outsideWidths.Add);
        using var boundsSubscription = readOnlyView.WhenAnyValue(e => e.OutsideBounds).Subscribe(outsideBounds.Add);

        pixels.Width = 2560;
        pixels.X = 30;

        Assert.Equal(new[] { 0d }, borders);
        Assert.Equal(new[] { 1920d, 2560d }, outsideWidths);
        Assert.Equal(new Rect(30, 20, 2560, 1080), outsideBounds[^1]);
        Assert.True(outsideBounds.Count >= 3);
    }

    [Fact]
    public void SourceAndRatioChangesPropagateAcrossTheFullDecoratorChain()
    {
        var source = PhysicalSize();
        var ratio = new DisplayRatioValue(2, 3);
        var rotated = new DisplayRotate(source, 1);
        var scaled = new DisplayScale(rotated, ratio);
        var located = new DisplayLocate(scaled, new Point(50, 60));
        IDisplaySize readOnlyView = located;

        var widths = new List<double>();
        var heights = new List<double>();
        var bounds = new List<Rect>();
        var outsideBounds = new List<Rect>();
        var changedProperties = new HashSet<string?>();

        using var widthSubscription = readOnlyView.WhenAnyValue(e => e.Width).Subscribe(widths.Add);
        using var heightSubscription = readOnlyView.WhenAnyValue(e => e.Height).Subscribe(heights.Add);
        using var boundsSubscription = readOnlyView.WhenAnyValue(e => e.Bounds).Subscribe(bounds.Add);
        using var outsideSubscription = readOnlyView.WhenAnyValue(e => e.OutsideBounds).Subscribe(outsideBounds.Add);
        located.PropertyChanged += (_, e) => changedProperties.Add(e.PropertyName);

        source.Height = 250;
        source.Width = 350;
        ratio.X = 4;
        ratio.Y = 5;
        located.X = 75;
        located.Y = 85;

        Assert.Equal(1000, readOnlyView.Width, 10);
        Assert.Equal(1750, readOnlyView.Height, 10);
        Assert.Equal(new Rect(75, 85, 1000, 1750), readOnlyView.Bounds);
        Assert.Equal(new Rect(59, 60, 1040, 1790), readOnlyView.OutsideBounds);
        Assert.Equal(readOnlyView.Width, widths[^1]);
        Assert.Equal(readOnlyView.Height, heights[^1]);
        Assert.Equal(readOnlyView.Bounds, bounds[^1]);
        Assert.Equal(readOnlyView.OutsideBounds, outsideBounds[^1]);
        Assert.Contains(nameof(IDisplaySize.Width), changedProperties);
        Assert.Contains(nameof(IDisplaySize.Height), changedProperties);
        Assert.Contains(nameof(IDisplaySize.Bounds), changedProperties);
        Assert.Contains(nameof(IDisplaySize.OutsideBounds), changedProperties);
    }

    [Fact]
    public void ReadOnlyCalculatedRatiosRemainReactiveThroughTheirContracts()
    {
        var first = new DisplayRatioValue(2, 3);
        var second = new DisplayRatioValue(5, 7);
        var product = new DisplayRatioRatio(first, second);
        var inverse = new DisplayInverseRatio(first);
        DisplayRatio baseProduct = product;
        IDisplayRatio productContract = product;
        IDisplayRatio inverseContract = inverse;

        var concreteValues = new List<double>();
        var baseValues = new List<double>();
        var contractValues = new List<double>();
        var inverseValues = new List<double>();

        using var concreteSubscription = product.WhenAnyValue(e => e.X).Subscribe(concreteValues.Add);
        using var baseSubscription = baseProduct.WhenAnyValue(e => e.X).Subscribe(baseValues.Add);
        using var contractSubscription = productContract.WhenAnyValue(e => e.X).Subscribe(contractValues.Add);
        using var inverseSubscription = inverseContract.WhenAnyValue(e => e.X).Subscribe(inverseValues.Add);

        first.X = 3;
        second.X = 11;
        first.Set(4, 6);

        Assert.Equal(new[] { 10d, 15d, 33d, 44d }, concreteValues);
        Assert.Equal(concreteValues, baseValues);
        Assert.Equal(concreteValues, contractValues);
        Assert.Equal(new[] { 0.5, 1.0 / 3.0, 0.25 }, inverseValues);
    }

    [Fact]
    public void MonitorGeometryRewiresWhenTheEffectiveDimensionInstanceChanges()
    {
        var options = new ReactiveLayoutOptions();
        var layout = new MonitorsLayout(options);
        var model = new PhysicalMonitorModel("MODEL");
        model.PhysicalSize.Width = 600;
        model.PhysicalSize.Height = 340;
        model.PhysicalSize.LeftBorder = 10;
        model.PhysicalSize.TopBorder = 11;
        model.PhysicalSize.RightBorder = 12;
        model.PhysicalSize.BottomBorder = 13;

        var monitor = new PhysicalMonitor("MONITOR", layout, model);
        var source = new DisplaySource("SOURCE") { AttachedToDesktop = true };
        source.InPixel.Set(new Rect(0, 0, 1920, 1080));
        var physicalSource = new PhysicalSource("DEVICE", monitor, source);
        monitor.ActiveSource = physicalSource;
        monitor.Sources.Add(physicalSource);

        var effectiveSizes = new List<IMutableDisplaySize>();
        var outsideWidths = new List<double>();
        using var sizeSubscription = monitor.WhenAnyValue(e => e.EffectivePhysicalSize).Subscribe(effectiveSizes.Add);
        using var geometrySubscription = monitor.WhenAnyValue(e => e.DepthProjection.OutsideWidth).Subscribe(outsideWidths.Add);

        Assert.Same(model.PhysicalSize, monitor.EffectivePhysicalSize);
        Assert.Equal(622, outsideWidths[^1]);

        options.SetBorderValues("PerMonitor");
        var perMonitorSize = monitor.EffectivePhysicalSize;
        Assert.NotSame(model.PhysicalSize, perMonitorSize);
        monitor.Borders.Left = 40;
        Assert.Equal(652, outsideWidths[^1]);

        var activeBranchEmissionCount = outsideWidths.Count;
        model.PhysicalSize.LeftBorder = 15;
        Assert.Equal(activeBranchEmissionCount, outsideWidths.Count);

        options.SetBorderValues("PerModel");
        Assert.Same(model.PhysicalSize, monitor.EffectivePhysicalSize);
        Assert.Equal(627, outsideWidths[^1]);

        activeBranchEmissionCount = outsideWidths.Count;
        monitor.Borders.Right = 50;
        Assert.Equal(activeBranchEmissionCount, outsideWidths.Count);

        options.SetBorderValues("PerMonitor");
        Assert.Same(perMonitorSize, monitor.EffectivePhysicalSize);
        Assert.Equal(690, outsideWidths[^1]);
        Assert.Equal(4, effectiveSizes.Count);
    }
}
