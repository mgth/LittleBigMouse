using LittleBigMouse.Plugin.Layout.Avalonia.BorderResistancePlugin;
using Xunit;

namespace LittleBigMouse.Ui.Avalonia.Tests;

public sealed class BorderSectionGeometryTests
{
    [Fact]
    public void FreeGapIsBoundedByPartialSectionsOnBothSides()
    {
        var occupied = new[] { new BorderSpan(70, 100), new BorderSpan(0, 20) };

        var gap = BorderSectionGeometry.FreeGapAround(occupied, 50, 120);

        Assert.Equal(new BorderSpan(20, 70), gap);
    }

    [Fact]
    public void ReferenceInsideASectionProducesAnEmptyGap()
    {
        var gap = BorderSectionGeometry.FreeGapAround(
            [new BorderSpan(20, 70)], 40, 100);

        Assert.Equal(new BorderSpan(40, 40), gap);
    }

    [Fact]
    public void ReferenceOnASectionBoundaryBelongsToTheAdjacentGap()
    {
        var occupied = new[] { new BorderSpan(10, 30), new BorderSpan(50, 80) };

        Assert.Equal(
            new BorderSpan(30, 50),
            BorderSectionGeometry.FreeGapAround(occupied, 30, 100));
        Assert.Equal(
            new BorderSpan(30, 50),
            BorderSectionGeometry.FreeGapAround(occupied, 50, 100));
    }

    [Fact]
    public void LargestFreeSpanHandlesOverlapsAndKeepsTheFirstEqualGap()
    {
        var occupied = new[]
        {
            new BorderSpan(40, 60),
            new BorderSpan(20, 40),
            new BorderSpan(80, 90)
        };

        var span = BorderSectionGeometry.LargestFreeSpan(occupied, new BorderSpan(0, 100));

        Assert.Equal(new BorderSpan(0, 20), span);
    }

    [Fact]
    public void ClampRestrictsBothEndsToTheFreeGap()
    {
        Assert.Equal(
            new BorderSpan(20, 70),
            BorderSectionGeometry.ClampToFreeSpace(
                new BorderSpan(5, 90), new BorderSpan(20, 70)));
    }

    [Fact]
    public void ResizeOrdersInvertedEndsBeforeClamping()
    {
        var resized = BorderSectionGeometry.Resize(
            [new BorderSpan(0, 20), new BorderSpan(80, 100)],
            new BorderSpan(30, 60),
            90,
            10,
            100,
            2);

        Assert.Equal(new BorderSpan(20, 80), resized);
    }

    [Fact]
    public void MinimumLengthIsInclusive()
    {
        Assert.Equal(
            new BorderSpan(10, 12),
            BorderSectionGeometry.Create([], 10, 12, 100, 2));
        Assert.Null(BorderSectionGeometry.Create([], 10, 11.999, 100, 2));
    }

    [Fact]
    public void MoveKeepsLengthAndStopsAtBothNeighbours()
    {
        var occupied = new[] { new BorderSpan(0, 20), new BorderSpan(80, 100) };
        var current = new BorderSpan(30, 50);

        Assert.Equal(
            new BorderSpan(20, 40),
            BorderSectionGeometry.Move(occupied, current, -100, 100));
        Assert.Equal(
            new BorderSpan(60, 80),
            BorderSectionGeometry.Move(occupied, current, 100, 100));
    }

    [Fact]
    public void ExpandAndFillUseTheWholeContainingGap()
    {
        var occupied = new[] { new BorderSpan(0, 20), new BorderSpan(80, 100) };

        Assert.Equal(
            new BorderSpan(20, 80),
            BorderSectionGeometry.Expand(
                occupied, new BorderSpan(40, 50), 100, 2));
        Assert.Equal(
            new BorderSpan(20, 80),
            BorderSectionGeometry.CreateFilling(occupied, 60, 100, 2));
    }

    [Fact]
    public void MirrorClipsToTheFacingEdgeAndKeepsItsLargestFreePart()
    {
        var plan = BorderSectionGeometry.PlanMirror(
            sourceOriginMm: 0,
            source: new BorderSpan(10, 90),
            targetOriginMm: 30,
            targetLengthMm: 50,
            targetOccupied: [new BorderSpan(0, 15)],
            minimumLengthMm: 2);

        Assert.Equal(new BorderSpan(15, 50), plan);
    }

    [Fact]
    public void MirrorReturnsNothingWhenNoUsableTargetPartRemains()
    {
        var plan = BorderSectionGeometry.PlanMirror(
            0,
            new BorderSpan(10, 20),
            100,
            50,
            [],
            2);

        Assert.Null(plan);
    }
}

public sealed class BorderSnapEngineTests
{
    [Fact]
    public void BuildTargetsKeepsOnlyVisiblePartialScreenEdges()
    {
        var targets = BorderSnapEngine.BuildTargets(
            edgeLengthMm: 100,
            edgeOriginMm: 20,
            isVertical: true,
            otherScreens:
            [
                new BorderRectangle(0, 10, 50, 60),
                new BorderRectangle(0, 90, 50, 150)
            ],
            sectionsOnEdge: [],
            parallelSections: []);

        Assert.DoesNotContain(targets, target => target.Mm == -10);
        Assert.Contains(targets, target => target == new SnapTarget(40, SnapKind.ScreenEdge));
        Assert.Contains(targets, target => target == new SnapTarget(70, SnapKind.ScreenEdge));
        Assert.DoesNotContain(targets, target => target.Mm == 130);
    }

    [Fact]
    public void BoundarySnapIsStrictlyInsideTolerance()
    {
        var targets = new[] { new SnapTarget(20, SnapKind.Section) };

        Assert.Equal(16, BorderSnapEngine.Snap(16, 100, 4, targets));
        Assert.Equal(20, BorderSnapEngine.Snap(16.001, 100, 4, targets));
    }

    [Fact]
    public void EquidistantSnapKeepsTheFirstCandidate()
    {
        var targets = new[]
        {
            new SnapTarget(40, SnapKind.Section),
            new SnapTarget(60, SnapKind.ScreenEdge)
        };

        Assert.Equal(40, BorderSnapEngine.Snap(50, 100, 20, targets));
    }

    [Fact]
    public void MovedSectionSnapsAtToleranceButKeepsFirstEquidistantCandidate()
    {
        var targets = new[]
        {
            new SnapTarget(40, SnapKind.Section),
            new SnapTarget(60, SnapKind.ScreenEdge)
        };

        Assert.Equal(40, BorderSnapEngine.SnapMovedStart(50, 20, 10, targets));
    }

    [Fact]
    public void NoTargetInReachKeepsTheWantedPosition()
    {
        Assert.Equal(
            50,
            BorderSnapEngine.SnapMovedStart(
                50, 20, 4, [new SnapTarget(10, SnapKind.Section)]));
    }

    [Fact]
    public void ExactMatchUsesTheHistoricalOneMicrometreTolerance()
    {
        var targets = new[] { new SnapTarget(0, SnapKind.EdgeEnd) };

        Assert.NotNull(BorderSnapEngine.MatchedTarget(targets, 0.000999));
        Assert.Null(BorderSnapEngine.MatchedTarget(targets, 0.001));
    }
}

public sealed class FacingEdgeResolverTests
{
    static readonly BorderRectangle Source = new(0, 0, 100, 100);

    [Fact]
    public void PartialOverlapCountsButAContactAtOneCornerDoesNot()
    {
        var candidates = new[]
        {
            new FacingEdgeCandidate<string>("partial", new BorderRectangle(110, 90, 210, 190)),
            new FacingEdgeCandidate<string>("corner", new BorderRectangle(110, 100, 210, 200))
        };

        var facing = FacingEdgeResolver.FindAll(
            BorderSideKind.Right, Source, 20, candidates).ToArray();

        Assert.Collection(facing, candidate => Assert.Equal("partial", candidate.Value));
    }

    [Fact]
    public void GapExactlyAtMaximumTravelDistanceIsIncluded()
    {
        var candidate = new FacingEdgeCandidate<string>(
            "at-limit", new BorderRectangle(120, 20, 220, 80));

        var facing = FacingEdgeResolver.FindNearest(
            BorderSideKind.Right, Source, 20, [candidate]);

        Assert.Equal("at-limit", facing?.Value);
        Assert.Equal(BorderSideKind.Left, facing?.Kind);
    }

    [Theory]
    [InlineData(BorderSideKind.Left, -120, 20, -20, 80, BorderSideKind.Right)]
    [InlineData(BorderSideKind.Top, 20, -120, 80, -20, BorderSideKind.Bottom)]
    [InlineData(BorderSideKind.Right, 120, 20, 220, 80, BorderSideKind.Left)]
    [InlineData(BorderSideKind.Bottom, 20, 120, 80, 220, BorderSideKind.Top)]
    public void EveryDirectionResolvesItsOppositeEdge(
        BorderSideKind sourceKind,
        double left,
        double top,
        double right,
        double bottom,
        BorderSideKind expectedKind)
    {
        var candidate = new FacingEdgeCandidate<string>(
            "target", new BorderRectangle(left, top, right, bottom));

        var facing = FacingEdgeResolver.FindNearest(
            sourceKind, Source, 20, [candidate]);

        Assert.Equal("target", facing?.Value);
        Assert.Equal(expectedKind, facing?.Kind);
    }

    [Fact]
    public void EquidistantCandidatesKeepLayoutOrder()
    {
        var candidates = new[]
        {
            new FacingEdgeCandidate<string>("first", new BorderRectangle(110, 0, 210, 40)),
            new FacingEdgeCandidate<string>("second", new BorderRectangle(110, 60, 210, 100))
        };

        var facing = FacingEdgeResolver.FindNearest(
            BorderSideKind.Right, Source, 20, candidates);

        Assert.Equal("first", facing?.Value);
    }

    [Fact]
    public void BehindOutOfRangeAndNonOverlappingCandidatesYieldNoTarget()
    {
        var candidates = new[]
        {
            new FacingEdgeCandidate<string>("behind", new BorderRectangle(50, 20, 90, 80)),
            new FacingEdgeCandidate<string>("far", new BorderRectangle(121, 20, 221, 80)),
            new FacingEdgeCandidate<string>("gap", new BorderRectangle(110, 101, 210, 200))
        };

        Assert.Null(FacingEdgeResolver.FindNearest(
            BorderSideKind.Right, Source, 20, candidates));
    }
}
