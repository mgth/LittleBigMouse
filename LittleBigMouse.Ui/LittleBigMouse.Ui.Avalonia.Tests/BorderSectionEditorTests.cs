using DynamicData;
using HLab.Geo;
using LittleBigMouse.DisplayLayout;
using LittleBigMouse.DisplayLayout.Dimensions;
using LittleBigMouse.DisplayLayout.Monitors;
using LittleBigMouse.Plugin.Layout.Avalonia.BorderResistancePlugin;
using Xunit;

namespace LittleBigMouse.Ui.Avalonia.Tests;

/// <summary>
/// The section editor's logic, tested without a display: coordinate conversion,
/// snapping onto neighbouring monitors, overlap prevention and mirroring. The
/// gestures themselves need a human, but everything they compute does not — and
/// this is where the mistakes that would silently corrupt a layout live.
/// </summary>
public sealed class BorderSectionEditorTests
{
    // Two 1920x1080 monitors, 480x270 mm each, side by side and vertically aligned.
    static MonitorsLayout TwoMonitors(out PhysicalMonitor left, out PhysicalMonitor right)
    {
        var layout = new MonitorsLayout(new ILayoutOptions.Design()) { Id = "TEST" };

        left = AddMonitor(layout, "LEFT", -480, 0);
        right = AddMonitor(layout, "RIGHT", 0, 0);

        return layout;
    }

    static PhysicalMonitor AddMonitor(
        MonitorsLayout layout, string id, double xMm, double yMm,
        double widthMm = 480, double heightMm = 270)
    {
        var model = new PhysicalMonitorModel($"PNP-{id}");
        model.PhysicalSize.Width = widthMm;
        model.PhysicalSize.Height = heightMm;

        var monitor = new PhysicalMonitor(id, layout, model);
        var source = new DisplaySource($"SRC-{id}") { AttachedToDesktop = true };
        source.InPixel.Set(new Rect(new Point(0, 0), new Size(1920, 1080)));

        var physicalSource = new PhysicalSource($"DEV-{id}", monitor, source);
        monitor.ActiveSource = physicalSource;
        monitor.Sources.Add(physicalSource);

        layout.AddOrUpdatePhysicalMonitor(monitor);
        layout.AddOrUpdatePhysicalSource(physicalSource);

        monitor.DepthProjection.X = xMm;
        monitor.DepthProjection.Y = yMm;

        return monitor;
    }

    static BorderSideViewModel RightEdgeOf(PhysicalMonitor monitor, double pixelLength = 540)
        => new(monitor, BorderSideKind.Right, monitor.BorderResistance.Right) { PixelLength = pixelLength };

    [Fact]
    public void PixelsAndMillimetresRoundTrip()
    {
        var side = RightEdgeOf(TwoMonitorsLeft());

        // 540 UI px covering a 270 mm edge: 2 px per mm.
        Assert.Equal(270, side.LengthMm);
        Assert.Equal(200, side.ToPixels(100));
        Assert.Equal(100, side.ToMm(200));
    }

    [Fact]
    public void SnapTargetsIncludeTheInnerEdgesOfNeighbours()
    {
        var layout = TwoMonitors(out var left, out var right);

        // Push the right monitor down: its inner top edge now falls mid-way along
        // the left monitor's right edge.
        right.DepthProjection.Y = 100;

        var targets = RightEdgeOf(left).SnapTargetsMm();

        // Own ends and centre.
        Assert.Contains(0.0, targets);
        Assert.Contains(270.0, targets);
        Assert.Contains(135.0, targets);

        // The neighbour's visible top edge, relative to this edge's origin.
        Assert.Contains(100.0, targets);

        Assert.NotNull(layout);
    }

    [Fact]
    public void SnappingUsesTheVisibleAreaNotTheBezel()
    {
        var layout = TwoMonitors(out var left, out var right);
        right.DepthProjection.Y = 100;

        // A fat bezel must not move the snap target: zones are built from the
        // visible rectangle, so snapping on the outer edge would leave a sliver.
        right.Model.PhysicalSize.TopBorder = 20;

        var targets = RightEdgeOf(left).SnapTargetsMm();

        Assert.Contains(100.0, targets);
        Assert.DoesNotContain(80.0, targets);
        Assert.NotNull(layout);
    }

    [Fact]
    public void SnapPullsOnlyWithinTolerance()
    {
        var side = RightEdgeOf(TwoMonitorsLeft());

        // 8 px of tolerance at 2 px/mm = 4 mm.
        Assert.Equal(135, side.Snap(133));
        Assert.Equal(120, side.Snap(120));

        // Ctrl suspends snapping, as when dragging a monitor.
        Assert.Equal(133, side.Snap(133, enabled: false));
    }

    [Fact]
    public void CreatedSectionsNeverOverlap()
    {
        var side = RightEdgeOf(TwoMonitorsLeft());

        Assert.NotNull(side.Create(50, 150));

        // Start in free space at 200 and sweep back across the existing section:
        // the new one stops at its edge instead of straddling it.
        var second = side.Create(200, 100);

        Assert.NotNull(second);
        Assert.Equal(150, second!.From);
        Assert.Equal(200, second.To);
    }

    [Fact]
    public void ASectionTooShortToBeIntentionalIsRejected()
    {
        var side = RightEdgeOf(TwoMonitorsLeft());
        Assert.Null(side.Create(100, 101));
        Assert.Empty(side.Side.Sections.Items);
    }

    [Fact]
    public void MovingASectionStopsAtItsNeighbour()
    {
        var side = RightEdgeOf(TwoMonitorsLeft());
        var first = side.Create(0, 100)!;
        var second = side.Create(120, 200)!;

        // Push the second one hard into the first: it must not swallow it.
        side.MoveBy(second, -200);

        Assert.Equal(100, second.From);
        Assert.Equal(180, second.To);
        Assert.Equal(0, first.From);
        Assert.Equal(100, first.To);
    }

    [Fact]
    public void SectionCountIsCapped()
    {
        var side = RightEdgeOf(TwoMonitorsLeft());

        // 270 mm edge, 10 mm sections: room for far more than the cap.
        for (var i = 0; i < BorderSideViewModel.MaximumSections + 5; i++)
            side.Create(i * 12, i * 12 + 10);

        Assert.Equal(BorderSideViewModel.MaximumSections, side.Side.Sections.Count);
    }

    [Fact]
    public void MirroringLandsAtTheSameAbsolutePosition()
    {
        var layout = TwoMonitors(out var left, out var right);
        right.DepthProjection.Y = 60;

        var side = RightEdgeOf(left);
        var section = side.Create(100, 200)!;
        section.Move = 7;
        section.MoveBlock = true;

        Assert.True(side.MirrorToFacingEdge(section));

        var mirrored = Assert.Single(right.BorderResistance.Left.Sections.Items);

        // Left edge origin is 60 mm lower, so the same absolute band 100..200
        // becomes 40..140 there.
        Assert.Equal(40, mirrored.From);
        Assert.Equal(140, mirrored.To);
        Assert.Equal(7, mirrored.Move);
        Assert.True(mirrored.MoveBlock);
        Assert.NotNull(layout);
    }

    [Fact]
    public void MirroringWorksAcrossBezels()
    {
        // The case every real setup is in, and the one the first version missed:
        // monitors are placed frame against frame, so their VISIBLE areas end up
        // separated by the sum of the two bezels. Testing adjacency by "do the
        // visible edges touch" found a facing edge only on borderless monitors —
        // which is to say only in the other tests here.
        var layout = TwoMonitors(out var left, out var right);

        left.Model.PhysicalSize.RightBorder = 12;
        right.Model.PhysicalSize.LeftBorder = 10;
        right.DepthProjection.X = 22;

        var side = RightEdgeOf(left);
        var section = side.Create(100, 200)!;

        Assert.True(side.MirrorToFacingEdge(section));
        Assert.Single(right.BorderResistance.Left.Sections.Items);
        Assert.NotNull(layout);
    }

    [Fact]
    public void CanMirrorFollowsTheMonitorsMovingAround()
    {
        // The button's enabled state has to answer the same question as the action,
        // and answer it again once the layout changes rather than going stale.
        var layout = TwoMonitors(out var left, out var right);

        var side = RightEdgeOf(left);
        var section = side.Create(100, 200)!;

        Assert.True(side.CanMirror(section));

        // Pushed out of reach.
        right.DepthProjection.X = 900;
        Assert.False(side.CanMirror(section));

        // Brought back.
        right.DepthProjection.X = 0;
        Assert.True(side.CanMirror(section));

        // Slid down until it no longer overlaps this edge at all.
        right.DepthProjection.Y = 400;
        Assert.False(side.CanMirror(section));

        Assert.NotNull(layout);
    }

    [Fact]
    public void MirroringTakesTheFreePartWhenTheFacingEdgeIsPartlyTaken()
    {
        // Straight from a real three-monitor layout. The Samsung sits 57.6 mm higher
        // than the Philips and 40 mm to its right; a section on the Samsung's left
        // edge covers absolute Y 138.4 to 334.4, while the Philips' right edge is
        // already occupied from 0 to 238.6. Only the bottom 96 mm face each other.
        var layout = new MonitorsLayout(new ILayoutOptions.Design()) { Id = "REAL" };

        var philips = AddMonitor(layout, "PHL", 0, 0, 698, 393);
        var samsung = AddMonitor(layout, "SAM", 738, -57.634, 697, 392);

        philips.BorderResistance.Right.Sections.Add(new BorderSection { From = 0, To = 238.558 });

        var side = new BorderSideViewModel(samsung, BorderSideKind.Left, samsung.BorderResistance.Left)
        {
            PixelLength = 392
        };
        var section = side.Create(196, 392)!;

        Assert.True(side.CanMirror(section));
        Assert.True(side.MirrorToFacingEdge(section));

        var mirrored = philips.BorderResistance.Right.Sections.Items
            .Single(s => s.From > 200);

        // The occupied stretch is skipped; the copy starts where it ends.
        Assert.Equal(238.558, mirrored.From, 3);
        Assert.Equal(334.366, mirrored.To, 3);
    }

    [Fact]
    public void CanMirrorIsFalseWhenTheFacingEdgeHasNoRoom()
    {
        var layout = TwoMonitors(out var left, out var right);

        var side = RightEdgeOf(left);
        var section = side.Create(100, 200)!;

        // The facing stretch is already taken, leaving nothing to land in.
        right.BorderResistance.Left.Sections.Add(new BorderSection { From = 0, To = 270 });

        Assert.False(side.CanMirror(section));
        Assert.False(side.MirrorToFacingEdge(section));
        Assert.NotNull(layout);
    }

    [Fact]
    public void MirroringDoesNothingWithoutAFacingMonitor()
    {
        var layout = TwoMonitors(out var left, out var right);

        // Beyond MaxTravelDistance: nothing faces the left monitor's right edge.
        right.DepthProjection.X = 900;

        var side = RightEdgeOf(left);
        var section = side.Create(100, 200)!;

        Assert.False(side.MirrorToFacingEdge(section));
        Assert.Empty(right.BorderResistance.Left.Sections.Items);
        Assert.NotNull(layout);
    }

    [Fact]
    public void FullyBlockedIsOnlyReportedWhenNothingCanCross()
    {
        var side = RightEdgeOf(TwoMonitorsLeft());
        Assert.False(side.IsFullyBlocked);

        // One wall over part of the edge: a gap remains.
        var section = side.Create(0, 100)!;
        section.DragBlock = true;
        Assert.False(side.IsFullyBlocked);

        // Cover the rest and the edge really is sealed.
        var rest = side.Create(100, 270)!;
        rest.MoveBlock = true;
        rest.DragBlock = true;
        Assert.True(side.IsFullyBlocked);

        // Opening drags anywhere rescues it.
        rest.DragBlock = false;
        Assert.False(side.IsFullyBlocked);
    }

    static PhysicalMonitor TwoMonitorsLeft()
    {
        TwoMonitors(out var left, out _);
        return left;
    }
}
