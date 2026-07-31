using DynamicData;
using HLab.Geo;
using LittleBigMouse.DisplayLayout.Dimensions;
using LittleBigMouse.DisplayLayout.Monitors;
using LittleBigMouse.DisplayLayout.Monitors.Extensions;
using Xunit.Abstractions;

namespace LittleBigMouse.DisplayLayout.Tests;

/// <summary>
/// Placing monitors from the system's pixel configuration.
/// <para>
/// Two ways in, and they had better agree: a first run with nothing stored places
/// everything from scratch, and the "Place from windows config" menu item does the
/// same thing on demand. A user who does not recognize their desktop on first launch,
/// clicks the button, and gets something better has been shown a worse answer for no
/// reason they can see.
/// </para>
/// </summary>
public class PlaceFromSystemTests(ITestOutputHelper output)
{
    static PhysicalMonitor AddMonitor(MonitorsLayout layout, string id,
        double widthMm, double heightMm,
        double pixelX, double pixelY, double pixelWidth, double pixelHeight,
        bool primary = false)
    {
        var model = new PhysicalMonitorModel($"PNP_{id}");
        model.PhysicalSize.Width = widthMm;
        model.PhysicalSize.Height = heightMm;
        model.PhysicalSize.LeftBorder = 0;
        model.PhysicalSize.TopBorder = 0;
        model.PhysicalSize.RightBorder = 0;
        model.PhysicalSize.BottomBorder = 0;

        var monitor = new PhysicalMonitor(id, layout, model);
        var source = new DisplaySource($"{id}-src") { AttachedToDesktop = true, Primary = primary };
        source.InPixel.Set(new Rect(new Point(pixelX, pixelY), new Size(pixelWidth, pixelHeight)));

        var physicalSource = new PhysicalSource($"{id}-dev", monitor, source);
        monitor.ActiveSource = physicalSource;
        monitor.Sources.Add(physicalSource);

        layout.AddOrUpdatePhysicalMonitor(monitor);
        layout.AddOrUpdatePhysicalSource(physicalSource);
        return monitor;
    }

    /// <summary>
    /// Three monitors side by side in pixels, of different sizes and DPI — the shape
    /// LittleBigMouse exists for. Every monitor starts where a freshly built layout
    /// starts them: at the origin, stacked.
    /// </summary>
    static MonitorsLayout ThreeInARow()
    {
        var layout = new MonitorsLayout(new ILayoutOptions.Design());
        AddMonitor(layout, "LEFT", 520, 320, -1920, 0, 1920, 1080);
        AddMonitor(layout, "MAIN", 600, 340, 0, 0, 2560, 1440, primary: true);
        AddMonitor(layout, "RIGHT", 700, 400, 2560, 0, 1920, 1080);
        return layout;
    }

    static string Describe(MonitorsLayout layout) => string.Join(" | ",
        layout.PhysicalMonitors
            .OrderBy(m => m.Id)
            .Select(m => $"{m.Id}@({m.DepthProjection.X:F1},{m.DepthProjection.Y:F1})"));

    /// <summary>
    /// A real desktop, read off the machine this was written on: two 1440p panels side
    /// by side with the second hanging 219 px lower, and a 1280x720 TV — big and
    /// coarse — to their right. The vertical offset is what a row of top-aligned test
    /// monitors never exercises, and it is the ordinary case: nobody bolts their
    /// screens to a shelf.
    /// </summary>
    static MonitorsLayout RealDesktop()
    {
        var layout = new MonitorsLayout(new ILayoutOptions.Design());
        AddMonitor(layout, "MAIN", 600, 340, 0, 0, 2560, 1440, primary: true);
        AddMonitor(layout, "SIDE", 600, 340, 2560, -219, 2560, 1440);
        AddMonitor(layout, "TV", 1650, 920, 5120, 0, 1280, 720);
        return layout;
    }

    [Fact]
    public void PlacingTwiceLandsWherePlacingOnceDid_OnARealDesktop()
    {
        var layout = RealDesktop();

        layout.SetLocationsFromSystemConfiguration();
        var once = Describe(layout);
        output.WriteLine($"once:  {once}");

        layout.SetLocationsFromSystemConfiguration();
        var twice = Describe(layout);
        output.WriteLine($"twice: {twice}");

        Assert.Equal(once, twice);
    }

    [Fact]
    public void ARealDesktopKeepsItsOrderAndItsOffset()
    {
        var layout = RealDesktop();
        layout.SetLocationsFromSystemConfiguration();
        output.WriteLine(Describe(layout));

        var main = layout.PhysicalMonitors.Single(m => m.Id == "MAIN");
        var side = layout.PhysicalMonitors.Single(m => m.Id == "SIDE");
        var tv = layout.PhysicalMonitors.Single(m => m.Id == "TV");

        Assert.True(main.DepthProjection.X < side.DepthProjection.X, $"MAIN left of SIDE: {Describe(layout)}");
        Assert.True(side.DepthProjection.X < tv.DepthProjection.X, $"SIDE left of TV: {Describe(layout)}");

        // SIDE's top edge is 219 px above MAIN's (Windows Y grows downward). On a panel
        // 1440 px tall and 340 mm high that is 219 * 340 / 1440 = 51.7 mm up — the two
        // screens still overlap over most of their height. Anything near a full screen
        // height means the offset was not carried across at all.
        var expected = -219.0 * 340.0 / 1440.0;
        var actual = side.DepthProjection.Y - main.DepthProjection.Y;
        output.WriteLine($"offset: expected {expected:F1} mm, got {actual:F1} mm");

        Assert.True(Math.Abs(actual - expected) < 5.0,
            $"SIDE sits {actual:F1} mm from MAIN, expected {expected:F1} mm: {Describe(layout)}");
    }

    /// <summary>
    /// The property the perpendicular rule exists for: place from the system, push the
    /// result back to the system, and land on the pixel configuration you started from.
    /// <para>
    /// Without it the two directions disagreed, so every pass through the pair moved the
    /// desktop a little further from where it began.
    /// </para>
    /// </summary>
    [Fact]
    public void PlacingFromTheSystemAndBackIsANoOp()
    {
        var layout = RealDesktop();
        layout.SetLocationsFromSystemConfiguration();
        output.WriteLine(Describe(layout));

        var solved = PixelLocationSolver.Solve(
            [.. layout.PhysicalMonitors.Select(m => new PixelPlacementMonitor(
                m.Id,
                m.DepthProjection.Bounds,
                m.DepthProjection.OutsideBounds,
                new Size(m.ActiveSource.Source.InPixel.Bounds.Width, m.ActiveSource.Source.InPixel.Bounds.Height),
                m.ActiveSource.Source.Primary))]);

        // Both sides are anchored on the primary, so compare against the system's own
        // pixel positions relative to it.
        var primaryPx = layout.PhysicalMonitors
            .Single(m => m.ActiveSource.Source.Primary).ActiveSource.Source.InPixel.Bounds;

        foreach (var monitor in layout.PhysicalMonitors)
        {
            var px = monitor.ActiveSource.Source.InPixel.Bounds;
            var expected = new Point(px.X - primaryPx.X, px.Y - primaryPx.Y);
            var actual = solved[monitor.Id];
            output.WriteLine($"{monitor.Id}: system ({expected.X},{expected.Y}) -> solved ({actual.X},{actual.Y})");

            Assert.True(Math.Abs(actual.X - expected.X) <= 1, $"{monitor.Id} X: {actual.X} vs {expected.X}");
            Assert.True(Math.Abs(actual.Y - expected.Y) <= 1, $"{monitor.Id} Y: {actual.Y} vs {expected.Y}");
        }
    }

    [Fact]
    public void PlacingTwiceLandsWherePlacingOnceDid()
    {
        // The startup path places from scratch; the menu item places again over the
        // result. Both call the same method, so the only way they can disagree is if
        // the method's answer depends on where the monitors already were — and then
        // "what you get on first launch" is not "what the button gives you".
        var layout = ThreeInARow();

        layout.SetLocationsFromSystemConfiguration();
        var once = Describe(layout);
        output.WriteLine($"once:  {once}");

        layout.SetLocationsFromSystemConfiguration();
        var twice = Describe(layout);
        output.WriteLine($"twice: {twice}");

        Assert.Equal(once, twice);
    }

    [Fact]
    public void PlacingKeepsThePixelOrderOfTheDesktop()
    {
        // Whatever the millimetre widths, three monitors laid out left to right in the
        // system must come out left to right here. This is the weakest possible reading
        // of "matches the Windows configuration", and the one a user checks first.
        var layout = ThreeInARow();

        layout.SetLocationsFromSystemConfiguration();
        output.WriteLine(Describe(layout));

        var left = layout.PhysicalMonitors.Single(m => m.Id == "LEFT");
        var main = layout.PhysicalMonitors.Single(m => m.Id == "MAIN");
        var right = layout.PhysicalMonitors.Single(m => m.Id == "RIGHT");

        Assert.True(left.DepthProjection.X < main.DepthProjection.X,
            $"LEFT must stay left of MAIN: {Describe(layout)}");
        Assert.True(main.DepthProjection.X < right.DepthProjection.X,
            $"MAIN must stay left of RIGHT: {Describe(layout)}");
    }

    [Fact]
    public void PlacingLeavesNoOverlapAndNoGap()
    {
        // Monitors adjacent in pixels are adjacent in millimetres: edge to edge, which
        // is the whole point of deriving the physical layout from the system one.
        var layout = ThreeInARow();

        layout.SetLocationsFromSystemConfiguration();
        output.WriteLine(Describe(layout));

        var ordered = layout.PhysicalMonitors.OrderBy(m => m.DepthProjection.X).ToList();
        for (var i = 1; i < ordered.Count; i++)
        {
            var previous = ordered[i - 1].DepthProjection.OutsideBounds;
            var current = ordered[i].DepthProjection.OutsideBounds;
            Assert.True(Math.Abs(current.Left - previous.Right) < 0.001,
                $"{ordered[i].Id} should touch {ordered[i - 1].Id}: {Describe(layout)}");
        }
    }
}
