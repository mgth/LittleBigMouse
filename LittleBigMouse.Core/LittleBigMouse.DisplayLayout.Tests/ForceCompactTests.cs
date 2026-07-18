using DynamicData;
using HLab.Geo;
using LittleBigMouse.DisplayLayout.Dimensions;
using LittleBigMouse.DisplayLayout.Monitors;

namespace LittleBigMouse.DisplayLayout.Tests;

/// <summary>
/// ForceCompact places monitors one by one, nearest first. The ordering key is
/// DistanceToTouch(...).DistanceHV(): if the infinite no-touch axis leaks into that
/// value, every reachable monitor compares equal (infinity) and the order degenerates
/// to collection order — a far big monitor placed first steals the place of a near
/// small one, which then gets ejected to a random side (issue #450).
/// </summary>
public class ForceCompactTests
{
    static PhysicalMonitor AddMonitor(MonitorsLayout layout, string id, string pnp,
        double width, double height, double x, double y, bool primary = false)
    {
        var model = new PhysicalMonitorModel(pnp);
        model.PhysicalSize.Width = width;
        model.PhysicalSize.Height = height;
        model.PhysicalSize.LeftBorder = 0;
        model.PhysicalSize.TopBorder = 0;
        model.PhysicalSize.RightBorder = 0;
        model.PhysicalSize.BottomBorder = 0;

        var monitor = new PhysicalMonitor(id, layout, model);
        var source = new DisplaySource($"{id}-src") { AttachedToDesktop = true, Primary = primary };
        source.InPixel.Set(new Rect(new Point(0, 0), new Size(1000, 1000)));

        var physicalSource = new PhysicalSource($"{id}-dev", monitor, source);
        monitor.ActiveSource = physicalSource;
        monitor.Sources.Add(physicalSource);

        layout.AddOrUpdatePhysicalMonitor(monitor);
        layout.AddOrUpdatePhysicalSource(physicalSource);

        monitor.DepthProjection.X = x;
        monitor.DepthProjection.Y = y;
        return monitor;
    }

    /// <summary>
    /// Layout: primary 700x400 at (0,0), a same-size neighbour touching its right
    /// side, a 1650x920 TV touching the neighbour's right side. All three then get
    /// translated away by (54, 6) — what dragging the primary does — and compacted.
    /// Each monitor must come back against its former neighbour: nearest first, so
    /// the TV lands on the neighbour, not on the primary over the neighbour's spot.
    /// </summary>
    [Fact]
    public void PrimaryDrag_CompactPreservesArrangement()
    {
        var layout = new MonitorsLayout(new ILayoutOptions.Design());

        // The far TV is deliberately FIRST in the collection: the degenerate
        // pre-fix ordering (all keys infinite) placed it first.
        var tv = AddMonitor(layout, "TV", "TV_0001", 1650, 920, 1454, -294);
        var primary = AddMonitor(layout, "P", "PHL0001", 700, 400, 0, 0, primary: true);
        var neighbour = AddMonitor(layout, "S", "SAM0001", 700, 400, 754, 16);

        layout.UpdatePhysicalMonitors();
        Assert.NotNull(layout.PrimaryMonitor);
        Assert.Same(primary, layout.PrimaryMonitor);

        layout.ForceCompact();

        // Neighbour pulled back against the primary's right edge, same height.
        Assert.Equal(700, neighbour.DepthProjection.X, 2);
        Assert.Equal(16, neighbour.DepthProjection.Y, 2);

        // TV pulled back against the neighbour's right edge, not teleported.
        Assert.Equal(1400, tv.DepthProjection.X, 2);
        Assert.Equal(-294, tv.DepthProjection.Y, 2);
    }

    [Fact]
    public void DistanceHV_FiniteAxisWins_OverInfiniteAxis()
    {
        // Side-by-side: horizontal touch at 54.47, no vertical touch possible.
        var d = new Thickness(54.47, double.PositiveInfinity, -1510, double.PositiveInfinity);
        Assert.Equal(54.47, d.DistanceHV(), 2);

        // Stacked: vertical touch at 33, no horizontal touch possible.
        var v = new Thickness(double.PositiveInfinity, 33, double.PositiveInfinity, -400);
        Assert.Equal(33, v.DistanceHV(), 2);
    }

    [Fact]
    public void DistanceHV_UnreachableByOneTranslation_StaysInfinite()
    {
        Assert.True(double.IsPositiveInfinity(MonitorExtensions.Infinity.DistanceHV()));
    }

    [Fact]
    public void DistanceHV_Overlap_StaysNegative()
    {
        // Full overlap: raw distances, all negative — unchanged behavior.
        var d = new Thickness(-100, -200, -300, -50);
        Assert.Equal(-50, d.DistanceHV(), 2);
    }
}
