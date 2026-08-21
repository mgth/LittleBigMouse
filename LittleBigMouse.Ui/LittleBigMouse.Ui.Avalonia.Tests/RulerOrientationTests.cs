using System;
using LittleBigMouse.Plugin.Layout.Avalonia.Rulers;
using Xunit;

// The outline is in the UI's coordinates, so it speaks Avalonia's geometry.
using Point = global::Avalonia.Point;
using Rect = global::Avalonia.Rect;

namespace LittleBigMouse.Ui.Avalonia.Tests;

/// <summary>
/// A ruler is a trapezoid, not a rectangle: the side facing the monitor is inset by the ruler's
/// own thickness at each end, so two rulers meeting at a corner mitre into each other at 45°
/// instead of overlapping into a dark square. Which side that is depends on where the ruler sits,
/// which is the whole of what these four orientations disagree about.
/// </summary>
public sealed class RulerOrientationTests
{
    const double Thickness = 20;
    const double Span = 400;

    /// <summary>Horizontal rulers lie above (0) or below (2) the monitor.</summary>
    static readonly Rect Horizontal = new(0, 0, Span, Thickness);

    /// <summary>Vertical rulers stand to the right (1) or the left (3) of it.</summary>
    static readonly Rect Vertical = new(0, 0, Thickness, Span);

    static IReadOnlyList<Point> Outline(int orientation)
    {
        var bounds = orientation is 0 or 2 ? Horizontal : Vertical;
        return RulerOrientation.Create(Thickness, Span, bounds, orientation).ClipOutline();
    }

    [Theory]
    [InlineData(0)]
    [InlineData(1)]
    [InlineData(2)]
    [InlineData(3)]
    public void EveryOrientationOutlinesAQuadrilateral(int orientation)
    {
        Assert.Equal(4, Outline(orientation).Count);
    }

    [Theory]
    // The narrow side faces the monitor: below a top ruler, left of a right-hand ruler, and so on.
    [InlineData(0, 0, Thickness)]   // top ruler   -> narrow side is its bottom edge
    [InlineData(2, 0, 0.0)]         // bottom ruler-> narrow side is its top edge
    [InlineData(1, 1, 0.0)]         // right ruler -> narrow side is its left edge
    [InlineData(3, 1, Thickness)]   // left ruler  -> narrow side is its right edge
    public void TheSideFacingTheMonitorIsInsetByTheRulersOwnThickness(
        int orientation, int axis, double facingCoordinate)
    {
        // axis 0: the narrow side is a horizontal edge, so it is the Y coordinate that pins it
        // and the X coordinates that are inset. axis 1 is the transpose.
        var outline = Outline(orientation);

        double Along(Point p) => axis == 0 ? p.X : p.Y;
        double Across(Point p) => axis == 0 ? p.Y : p.X;

        var facing = outline.Where(p => Math.Abs(Across(p) - facingCoordinate) < double.Epsilon)
            .Select(Along)
            .OrderBy(v => v)
            .ToList();

        Assert.Equal(2, facing.Count);
        Assert.Equal(Thickness, facing[0]);          // inset by one thickness at the near end
        Assert.Equal(Span - Thickness, facing[1]);   // and one at the far end — a 45° mitre
    }

    [Theory]
    [InlineData(0, 0, Thickness)]
    [InlineData(2, 0, 0.0)]
    [InlineData(1, 1, 0.0)]
    [InlineData(3, 1, Thickness)]
    public void TheSideAwayFromTheMonitorSpansTheWholeRuler(
        int orientation, int axis, double facingCoordinate)
    {
        var outline = Outline(orientation);

        double Along(Point p) => axis == 0 ? p.X : p.Y;
        double Across(Point p) => axis == 0 ? p.Y : p.X;

        var back = outline.Where(p => Math.Abs(Across(p) - facingCoordinate) >= double.Epsilon)
            .Select(Along)
            .OrderBy(v => v)
            .ToList();

        Assert.Equal(2, back.Count);
        Assert.Equal(0, back[0]);
        Assert.Equal(Span, back[1]);
    }

    [Fact]
    public void AnOrientationOutsideTheFourQuartersIsRejected()
    {
        // Render dispatches on the same value; a silent fallback here would draw a ruler with
        // the wrong transform rather than tell anyone.
        Assert.Throws<ArgumentOutOfRangeException>(
            () => RulerOrientation.Create(Thickness, Span, Horizontal, 4));
    }

    [Theory]
    [InlineData(0, Span, Thickness)]
    [InlineData(2, Span, Thickness)]
    [InlineData(1, Span, Thickness)]
    [InlineData(3, Span, Thickness)]
    public void TheDisplayedLengthRunsAlongTheRulerWhicheverWayItFaces(
        int orientation, double length, double size)
    {
        var bounds = orientation is 0 or 2 ? Horizontal : Vertical;

        var o = RulerOrientation.Create(Thickness, Span, bounds, orientation);

        Assert.Equal(length, o.DisplayLength);
        Assert.Equal(size, o.DisplaySize);
    }
}
