using DynamicData;
using HLab.Geo;
using LittleBigMouse.DisplayLayout.Dimensions;
using LittleBigMouse.DisplayLayout.Monitors;
using LittleBigMouse.DisplayLayout.Monitors.Extensions;

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
        double width, double height, double x, double y, bool primary = false,
        double pixelX = 0, double pixelY = 0, double pixelWidth = 1000, double pixelHeight = 1000)
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
        source.InPixel.Set(new Rect(new Point(pixelX, pixelY), new Size(pixelWidth, pixelHeight)));

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

    /// <summary>
    /// A monitor stacked on top of the TV must stay aligned with it through a
    /// primary drag: both belong to the same touching cluster and travel as one
    /// block. The per-monitor greedy left it shifted sideways ("still touching"
    /// vertically, so it was never pulled back).
    /// </summary>
    [Fact]
    public void PrimaryDrag_StackedClusterComesBackAsOneBlock()
    {
        var layout = new MonitorsLayout(new ILayoutOptions.Design());

        var primary = AddMonitor(layout, "P", "PHL0001", 700, 400, 0, 0, primary: true);
        // TV touching the primary's right side, stacked monitor on top of the TV.
        var tv = AddMonitor(layout, "TV", "TV_0001", 1650, 920, 754, -300);
        var stacked = AddMonitor(layout, "S", "SAM0001", 700, 400, 954, -700);

        layout.UpdatePhysicalMonitors();
        layout.ForceCompact();

        // The {TV, stacked} cluster slides back 54 as one rigid block.
        Assert.Equal(700, tv.DepthProjection.X, 2);
        Assert.Equal(-300, tv.DepthProjection.Y, 2);
        Assert.Equal(900, stacked.DepthProjection.X, 2);
        Assert.Equal(-700, stacked.DepthProjection.Y, 2);
    }

    /// <summary>
    /// Compact must also work from arbitrary positions: two monitors overlapping
    /// each other while both border-to-border with the TV. The overlap is resolved
    /// (primary never moves) without tearing the existing contacts apart.
    /// </summary>
    [Fact]
    public void OverlappingMonitors_GetSpreadApart_PrimaryStays()
    {
        var layout = new MonitorsLayout(new ILayoutOptions.Design());

        var tv = AddMonitor(layout, "TV", "TV_0001", 1650, 920, 0, 0);
        // Both under the TV, touching its bottom edge, overlapping each other.
        var primary = AddMonitor(layout, "P", "PHL0001", 700, 400, 100, 920, primary: true);
        var overlapping = AddMonitor(layout, "S", "SAM0001", 700, 400, 400, 920);

        layout.UpdatePhysicalMonitors();
        layout.ForceCompact();

        // Primary untouched.
        Assert.Equal(100, primary.DepthProjection.X, 2);
        Assert.Equal(920, primary.DepthProjection.Y, 2);
        // TV untouched: still in contact with the primary.
        Assert.Equal(0, tv.DepthProjection.X, 2);
        Assert.Equal(0, tv.DepthProjection.Y, 2);

        // The overlap is gone…
        var p = primary.DepthProjection.OutsideBounds;
        var s = overlapping.DepthProjection.OutsideBounds;
        var overlapX = Math.Min(p.Right, s.Right) - Math.Max(p.Left, s.Left);
        var overlapY = Math.Min(p.Bottom, s.Bottom) - Math.Max(p.Top, s.Top);
        Assert.False(overlapX > 0.01 && overlapY > 0.01, $"still overlapping: {overlapX}x{overlapY}");

        // …and the freed monitor is still in contact with the rest, not teleported.
        var d1 = s.Distance(p).ToArray().Select(Math.Abs).Min();
        var d2 = s.Distance(tv.DepthProjection.OutsideBounds).ToArray().Select(Math.Abs).Min();
        Assert.True(d1 < 0.01 || d2 < 0.01, $"lost all contacts (to primary: {d1}, to tv: {d2})");
    }

    /// <summary>
    /// Fresh-install auto-placement: three 4K monitors side by side in PIXEL space
    /// whose physical widths differ wildly (two 32" and a TV). The monitor whose
    /// only pixel adjacency is with the MIDDLE one must end up beside it: the
    /// alignment equalities (same pixel Y/Bottom against the primary) are hints and
    /// must not consume it before its real neighbour is placed.
    /// </summary>
    [Fact]
    public void SystemPlacement_SideBySidePixels_StaysSideBySideInMm()
    {
        var layout = new MonitorsLayout(new ILayoutOptions.Design());

        // Collection order puts the TV first, like the real HEC/PHL/SAME layout.
        var tv = AddMonitor(layout, "TV", "TV_0001", 1650, 920, 0, 0,
            pixelX: 7680, pixelWidth: 3840, pixelHeight: 2160);
        var primary = AddMonitor(layout, "P", "PHL0001", 700, 400, 0, 0, primary: true,
            pixelX: 0, pixelWidth: 3840, pixelHeight: 2160);
        var middle = AddMonitor(layout, "S", "SAM0001", 700, 400, 0, 0,
            pixelX: 3840, pixelWidth: 3840, pixelHeight: 2160);

        layout.UpdatePhysicalMonitors();
        layout.SetLocationsFromSystemConfiguration();

        // P | S | H side by side, bottom-aligned like their pixel bounds.
        Assert.Equal(700, middle.DepthProjection.X, 2);
        Assert.Equal(1400, tv.DepthProjection.X, 2);
        Assert.Equal(400, middle.DepthProjection.Bounds.Bottom, 2);
        Assert.Equal(400, tv.DepthProjection.Bounds.Bottom, 2);
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
