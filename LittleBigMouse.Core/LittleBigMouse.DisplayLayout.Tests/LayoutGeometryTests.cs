using HLab.Geo;
using LittleBigMouse.DisplayLayout.Monitors.Extensions;

namespace LittleBigMouse.DisplayLayout.Tests;

/// <summary>
/// The primitives both placement directions are built on, tested on plain rectangles. They are
/// shared precisely because the two directions must agree on what "adjacent", "overlapping" and
/// "the shared span" mean; if they drift apart here, they drift apart everywhere.
/// </summary>
public class LayoutGeometryTests
{
    [Fact]
    public void On_ReadsTheRequestedAxis()
    {
        var rect = new Rect(10, 20, 30, 40);

        Assert.Equal(new Interval(10, 30), rect.On(Axis.Horizontal));
        Assert.Equal(new Interval(20, 40), rect.On(Axis.Vertical));
    }

    [Fact]
    public void Perpendicular_IsTheOtherAxis()
    {
        Assert.Equal(Axis.Vertical, Axis.Horizontal.Perpendicular());
        Assert.Equal(Axis.Horizontal, Axis.Vertical.Perpendicular());
    }

    [Fact]
    public void OverlapWith_IsNegativeForAGap()
    {
        Assert.Equal(5, new Interval(0, 10).OverlapWith(new Interval(5, 10)));

        // Touching is zero overlap, not positive: a shared edge is not a crossable corridor.
        Assert.Equal(0, new Interval(0, 10).OverlapWith(new Interval(10, 10)));

        // Disjoint: the value is the gap, which is what the callers order islands by.
        Assert.Equal(-3, new Interval(0, 10).OverlapWith(new Interval(13, 10)));
    }

    [Fact]
    public void SharedMidpoint_IsDefinedEvenWithoutOverlap()
    {
        // Overlapping: midpoint of the shared part.
        Assert.Equal(7.5, new Interval(0, 10).SharedMidpoint(new Interval(5, 10)));

        // Disjoint: midpoint of the GAP. Neither solver special-cases two panels that only meet
        // through their bezels, because this keeps returning the right anchor for them.
        Assert.Equal(11.5, new Interval(0, 10).SharedMidpoint(new Interval(13, 10)));
    }

    [Fact]
    public void Overlap_NeedsSurface_NotJustASharedEdge()
    {
        var a = new Rect(0, 0, 10, 10);

        Assert.True(LayoutGeometry.Overlap(a, new Rect(5, 5, 10, 10)));

        // Edge to edge, and corner to corner: neither is an overlap.
        Assert.False(LayoutGeometry.Overlap(a, new Rect(10, 0, 10, 10)));
        Assert.False(LayoutGeometry.Overlap(a, new Rect(10, 10, 10, 10)));
    }

    [Fact]
    public void ContactBetween_ReadsBothSides()
    {
        var anchor = new Interval(0, 10);

        Assert.Equal(EdgeContact.After, LayoutGeometry.ContactBetween(anchor, new Interval(10, 5), 0));
        Assert.Equal(EdgeContact.Before, LayoutGeometry.ContactBetween(anchor, new Interval(-5, 5), 0));
        Assert.Equal(EdgeContact.None, LayoutGeometry.ContactBetween(anchor, new Interval(12, 5), 0));
    }

    [Fact]
    public void ContactBetween_ToleranceIsWhatSeparatesTheTwoDirections()
    {
        var anchor = new Interval(0, 10);
        var threeAway = new Interval(13, 5);

        // Pixel edges are integers the system reports verbatim: 3 apart is not adjacent.
        Assert.Equal(EdgeContact.None, LayoutGeometry.ContactBetween(anchor, threeAway, 0));

        // Bezel widths are hand-entered millimetres: 3mm of air still counts.
        Assert.Equal(EdgeContact.After, LayoutGeometry.ContactBetween(anchor, threeAway, 5));
    }

    [Fact]
    public void ToPixelAndToMm_AreInverses()
    {
        // A 27" 4K panel at scale 2: 597.7mm of glass over 1920 logical pixels.
        var profile = new AxisProfile(new Interval(100, 597.7), new Interval(2560, 1920));

        Assert.Equal(2560, profile.ToPixel(100), 9);
        Assert.Equal(100, profile.ToMm(2560), 9);
        Assert.Equal(1234.5, profile.ToPixel(profile.ToMm(1234.5)), 6);
    }

    [Fact]
    public void HasPixels_IsFalseForAMonitorReportingNone()
    {
        Assert.False(new AxisProfile(new Interval(0, 300), new Interval(0, 0)).HasPixels);
        Assert.True(new AxisProfile(new Interval(0, 300), new Interval(0, 1080)).HasPixels);
    }

    /// <summary>
    /// The invariant itself: whichever unknown you solve for, the shared physical midpoint lands
    /// on the same coordinate on both monitors. This is the single property that makes the two
    /// directions each other's inverse.
    /// </summary>
    [Fact]
    public void PixelOriginAndMillimetreOrigin_SolveTheSameInvariant()
    {
        // 27" 4K at scale 2 next to a 24" FHD, physically centred: different pitches, which is
        // the case a closed-form pixel midpoint gets wrong.
        var anchor = new AxisProfile(new Interval(0, 336.2), new Interval(0, 1080));
        var targetMm = new Interval(18.6, 299);
        var target = new AxisProfile(targetMm, new Interval(0, 1080));

        // Forward: where does the target go in pixels?
        var pixelLo = EdgeProjection.PixelOrigin(anchor, target);

        // Backward: given that pixel answer, where does it go in millimetres?
        var mmLo = EdgeProjection.MillimetreOrigin(anchor, target.AtPixel(pixelLo));

        Assert.Equal(targetMm.Lo, mmLo, 6);
    }

    [Fact]
    public void MillimetreOrigin_MatchesTheClosedFormWhenPitchesAreEqual()
    {
        // Same pitch on both: the pixel midpoint and the mm midpoint agree, so the iteration
        // must land exactly on the plain proportional answer.
        var anchor = new AxisProfile(new Interval(0, 270), new Interval(0, 1080));
        var target = new AxisProfile(new Interval(0, 270), new Interval(-219, 1080));

        // 219 pixels up, at 0.25 mm/px.
        Assert.Equal(-54.75, EdgeProjection.MillimetreOrigin(anchor, target), 9);
    }
}
