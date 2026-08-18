using DynamicData;
using HLab.Geo;
using LittleBigMouse.DisplayLayout.Dimensions;
using LittleBigMouse.DisplayLayout.Monitors;
using ReactiveUI;

namespace LittleBigMouse.DisplayLayout.Tests;

public class DimensionContractTests
{
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
    public void PhysicalDimensionImplementationsExposeWorkingFullMutationContracts()
    {
        var ratio = new DisplayRatioValue(2, 4);
        var borderSource = new DisplayBorders { Left = 3, Top = 4, Right = 5, Bottom = 6 };
        var dimensions = new (string Name, IMutableDisplaySize Value)[]
        {
            (nameof(DisplaySizeInMm), PhysicalSize()),
            (nameof(DisplayBorderOverride), new DisplayBorderOverride(PhysicalSize(), borderSource)),
            (nameof(DisplayLocate), new DisplayLocate(PhysicalSize())),
            (nameof(DisplayRotate), new DisplayRotate(PhysicalSize(), 1)),
            (nameof(DisplayScale), new DisplayScale(PhysicalSize(), ratio)),
            (nameof(DisplayScaleWithLocation), new DisplayScaleWithLocation(PhysicalSize(), ratio)),
            (nameof(DisplayTranslate), new DisplayTranslate(PhysicalSize(), new Vector(7, 11))),
        };

        foreach (var (name, value) in dimensions)
        {
            var observedWidths = new List<double>();
            IDisplaySize readOnlyView = value;
            using var subscription = readOnlyView.WhenAnyValue(e => e.Width).Subscribe(observedWidths.Add);

            value.X = 17;
            value.Y = 19;
            value.Width = 230;
            value.Height = 170;
            value.LeftBorder = 7;
            value.TopBorder = 8;
            value.RightBorder = 9;
            value.BottomBorder = 10;

            Assert.Equal(17, value.X, 10);
            Assert.Equal(19, value.Y, 10);
            Assert.Equal(230, value.Width, 10);
            Assert.Equal(170, value.Height, 10);
            Assert.Equal(7, value.LeftBorder, 10);
            Assert.Equal(8, value.TopBorder, 10);
            Assert.Equal(9, value.RightBorder, 10);
            Assert.Equal(10, value.BottomBorder, 10);
            Assert.Equal(230, observedWidths[^1], 10);
            Assert.True(observedWidths.Count >= 2, $"{name} did not notify its read-only Width contract");
            Assert.True(value.OutsideWidth > value.Width, name);
            Assert.True(value.OutsideHeight > value.Height, name);
        }
    }

    [Fact]
    public void PixelDimensionsExposeMutableBoundsAndReadOnlyZeroBorders()
    {
        IMutableDisplayBounds pixels = new DisplaySizeInPixels(
            new Rect(new Point(10, 20), new Size(1920, 1080)));

        Assert.False(pixels is IMutableDisplaySize);

        var observedWidths = new List<double>();
        IDisplaySize readOnlyView = pixels;
        using var subscription = readOnlyView.WhenAnyValue(e => e.Width).Subscribe(observedWidths.Add);

        pixels.Set(new Rect(new Point(30, 40), new Size(2560, 1440)));

        Assert.Equal(new Rect(new Point(30, 40), new Size(2560, 1440)), pixels.Bounds);
        Assert.Equal(0, pixels.LeftBorder);
        Assert.Equal(0, pixels.TopBorder);
        Assert.Equal(0, pixels.RightBorder);
        Assert.Equal(0, pixels.BottomBorder);
        Assert.Equal(pixels.Bounds, pixels.OutsideBounds);
        Assert.Equal(new[] { 1920d, 2560d }, observedWidths);
    }

    [Fact]
    public void PixelScaleImplementationsKeepOnlyTheSupportedMutationContract()
    {
        var pixels = new DisplaySizeInPixels(
            new Rect(new Point(10, 20), new Size(1920, 1080)));
        var ratio = new DisplayRatioValue(0.5, 0.25);

        IMutableDisplayBounds wpf = new DisplaySizeWpf(pixels, ratio);
        Assert.False(wpf is IMutableDisplaySize);

        var observedWidths = new List<double>();
        IDisplaySize readOnlyView = wpf;
        using var subscription = readOnlyView.WhenAnyValue(e => e.Width).Subscribe(observedWidths.Add);

        wpf.X = 15;
        wpf.Y = 10;
        wpf.Width = 1280;
        wpf.Height = 360;

        Assert.Equal(15, wpf.X, 10);
        Assert.Equal(10, wpf.Y, 10);
        Assert.Equal(1280, wpf.Width, 10);
        Assert.Equal(360, wpf.Height, 10);
        Assert.Equal(new[] { 960d, 1280d }, observedWidths);
        Assert.Equal(0, wpf.OutsideWidth - wpf.Width);
        Assert.Equal(0, wpf.OutsideHeight - wpf.Height);
    }

    [Fact]
    public void DipScaleExposesMutableBoundsButNotFixedPixelBorders()
    {
        var layout = new MonitorsLayout(new ILayoutOptions.Design());
        var model = new PhysicalMonitorModel("MODEL");
        model.PhysicalSize.Width = 600;
        model.PhysicalSize.Height = 340;
        var monitor = new PhysicalMonitor("MONITOR", layout, model);
        var source = new DisplaySource("SOURCE") { Primary = true, AttachedToDesktop = true };
        source.InPixel.Set(new Rect(new Point(0, 0), new Size(1920, 1080)));
        var physicalSource = new PhysicalSource("DEVICE", monitor, source);
        monitor.ActiveSource = physicalSource;
        monitor.Sources.Add(physicalSource);
        layout.AddOrUpdatePhysicalMonitor(monitor);
        layout.AddOrUpdatePhysicalSource(physicalSource);

        IMutableDisplayBounds dip = new DisplayScaleDip(source.InPixel, new DisplayRatioValue(96), layout);
        Assert.False(dip is IMutableDisplaySize);

        var observedWidths = new List<double>();
        IDisplaySize readOnlyView = dip;
        using var subscription = readOnlyView.WhenAnyValue(e => e.Width).Subscribe(observedWidths.Add);

        dip.X = 12;
        dip.Y = 14;
        dip.Width = 960;
        dip.Height = 540;

        Assert.Equal(12, dip.X, 10);
        Assert.Equal(14, dip.Y, 10);
        Assert.Equal(960, dip.Width, 10);
        Assert.Equal(540, dip.Height, 10);
        Assert.Equal(new[] { 1920d, 960d }, observedWidths);
        Assert.Equal(0, dip.LeftBorder);
        Assert.Equal(0, dip.TopBorder);
        Assert.Equal(0, dip.RightBorder);
        Assert.Equal(0, dip.BottomBorder);
    }

    [Fact]
    public void RatioImplementationsAdvertiseOnlySupportedMutability()
    {
        IMutableDisplayRatio value = new DisplayRatioValue(2, 3);
        IMutableDisplayRatio registry = new DisplayRatioRegistry(null!);
        IDisplayRatio inverse = new DisplayInverseRatio(value);
        IDisplayRatio product = new DisplayRatioRatio(value, new DisplayRatioValue(5, 7));

        value.X = 11;
        value.Y = 13;
        registry.X = 17;
        registry.Y = 19;

        Assert.Equal(11, value.X);
        Assert.Equal(13, value.Y);
        Assert.Equal(17, registry.X);
        Assert.Equal(19, registry.Y);
        Assert.False(inverse is IMutableDisplayRatio);
        Assert.False(product is IMutableDisplayRatio);
        Assert.Equal(1.0 / 11, inverse.X, 10);
        Assert.Equal(1.0 / 13, inverse.Y, 10);
        Assert.Equal(55, product.X);
        Assert.Equal(91, product.Y);
    }
}
