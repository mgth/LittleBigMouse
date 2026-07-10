using HLab.Geo;
using LittleBigMouse.DisplayLayout.Monitors.Extensions;

namespace LittleBigMouse.DisplayLayout.Tests;

public class PixelLocationSolverTests
{
    const double Bezel = 20.0;

    /// <summary>Build a monitor whose panel sits at (x,y) mm with uniform bezels.</summary>
    static PixelPlacementMonitor Monitor(
        string id, double x, double y, double widthMm, double heightMm,
        double pixelWidth, double pixelHeight, bool primary = false, double bezel = Bezel)
        => new(
            id,
            new Rect(x, y, widthMm, heightMm),
            new Rect(x - bezel, y - bezel, widthMm + 2 * bezel, heightMm + 2 * bezel),
            new Size(pixelWidth, pixelHeight),
            primary);

    static Rect RectOf(PixelPlacementMonitor m, IReadOnlyDictionary<string, Point> solved)
        => new(solved[m.Id], m.PixelSize);

    static void AssertNoOverlap(IReadOnlyList<PixelPlacementMonitor> monitors, IReadOnlyDictionary<string, Point> solved)
    {
        for (var i = 0; i < monitors.Count; i++)
        for (var j = i + 1; j < monitors.Count; j++)
        {
            var a = RectOf(monitors[i], solved);
            var b = RectOf(monitors[j], solved);
            var overlapX = Math.Min(a.Right, b.Right) - Math.Max(a.X, b.X);
            var overlapY = Math.Min(a.Bottom, b.Bottom) - Math.Max(a.Y, b.Y);
            Assert.False(overlapX > 0 && overlapY > 0,
                $"{monitors[i].Id}{a} overlaps {monitors[j].Id}{b}");
        }
    }

    [Fact]
    public void DualMonitorSideBySide()
    {
        // Two 1920x1080 panels of 480x270mm, bezels touching (as SetLocationsFromSystemConfiguration lays them out).
        var a = Monitor("A", 0, 0, 480, 270, 1920, 1080, primary: true);
        var b = Monitor("B", 480 + 2 * Bezel, 0, 480, 270, 1920, 1080);

        var solved = PixelLocationSolver.Solve([a, b]);

        Assert.Equal(new Point(0, 0), solved["A"]);
        Assert.Equal(new Point(1920, 0), solved["B"]);
    }

    [Fact]
    public void RoundTripFromSystemIsStable()
    {
        // Mm layout equivalent to system rects (0,0,1920,1080) and (1920,0,1920,1080)
        // with equal pixel Y: re-solving must return exactly the original pixels,
        // which is what makes the no-op guard effective.
        var a = Monitor("A", 0, 0, 480, 270, 1920, 1080, primary: true);
        var b = Monitor("B", 480 + 2 * Bezel, 0, 480, 270, 1920, 1080);

        var solved = PixelLocationSolver.Solve([b, a]);

        Assert.Equal(new Point(0, 0), solved["A"]);
        Assert.Equal(new Point(1920, 0), solved["B"]);
    }

    [Fact]
    public void DifferentScalesShareCrossingPoint()
    {
        // 27" 4K at scale 2 (logical 1920x1080, panel 597.7x336.2mm) next to a 24" FHD
        // (531x299mm), physically centered vertically.
        var a = Monitor("A", 0, 0, 597.7, 336.2, 1920, 1080, primary: true);
        var bY = (336.2 - 299) / 2;
        var b = Monitor("B", 597.7 + 2 * Bezel, bY, 531, 299, 1920, 1080);

        var solved = PixelLocationSolver.Solve([a, b]);

        Assert.Equal(new Point(0, 0), solved["A"]);
        Assert.Equal(1920, solved["B"].X);

        // The physical midpoint of the shared span projects to the same pixel Y on both sides (±1 for rounding).
        var mid = (Math.Max(0, bY) + Math.Min(336.2, bY + 299)) / 2;
        var yOnA = solved["A"].Y + mid / (336.2 / 1080);
        var yOnB = solved["B"].Y + (mid - bY) / (299.0 / 1080);
        Assert.True(Math.Abs(yOnA - yOnB) <= 1, $"crossing mismatch: {yOnA} vs {yOnB}");
    }

    [Fact]
    public void GridTwoByTwo()
    {
        var pitch = 480.0 + 2 * Bezel;   // panel + bezels, horizontal
        var pitchV = 270.0 + 2 * Bezel;

        var monitors = new[]
        {
            Monitor("A", 0, 0, 480, 270, 1920, 1080, primary: true),
            Monitor("B", pitch, 0, 480, 270, 1920, 1080),
            Monitor("C", 0, pitchV, 480, 270, 1920, 1080),
            Monitor("D", pitch, pitchV, 480, 270, 1920, 1080),
        };

        var solved = PixelLocationSolver.Solve(monitors);

        Assert.Equal(new Point(0, 0), solved["A"]);
        Assert.Equal(new Point(1920, 0), solved["B"]);
        Assert.Equal(new Point(0, 1080), solved["C"]);
        // D is constrained through two BFS paths; both must agree here.
        Assert.Equal(new Point(1920, 1080), solved["D"]);
        AssertNoOverlap(monitors, solved);
    }

    [Fact]
    public void SmallPhysicalGapCountsAsAdjacent()
    {
        // 3mm of air between bezels: below tolerance, still exact pixel contact.
        var a = Monitor("A", 0, 0, 480, 270, 1920, 1080, primary: true);
        var b = Monitor("B", 480 + 2 * Bezel + 3, 0, 480, 270, 1920, 1080);

        var solved = PixelLocationSolver.Solve([a, b]);

        Assert.Equal(1920, solved["B"].X);
        Assert.Equal(0, solved["B"].Y);
    }

    [Fact]
    public void DetachedIslandSnapsIntoContact()
    {
        // 500mm of air, diagonal offset: not adjacent, must be pulled into contact
        // (Windows needs a connected desktop; a KWin gap is uncrossable without the engine).
        var monitors = new[]
        {
            Monitor("A", 0, 0, 480, 270, 1920, 1080, primary: true),
            Monitor("B", 480 + 2 * Bezel + 500, 400, 480, 270, 1920, 1080),
        };

        var solved = PixelLocationSolver.Solve(monitors);

        var a = RectOf(monitors[0], solved);
        var b = RectOf(monitors[1], solved);

        AssertNoOverlap(monitors, solved);

        // Touching = zero distance on exactly one axis with overlap on the other.
        var touchX = (b.X == a.Right || a.X == b.Right) && Math.Min(a.Bottom, b.Bottom) - Math.Max(a.Y, b.Y) > 0;
        var touchY = (b.Y == a.Bottom || a.Y == b.Bottom) && Math.Min(a.Right, b.Right) - Math.Max(a.X, b.X) > 0;
        Assert.True(touchX || touchY, $"island not in contact: A{a} B{b}");
    }

    [Fact]
    public void PrimaryAlwaysAtOrigin()
    {
        // Primary in the middle of a row, listed last.
        var left = Monitor("L", 0, 0, 480, 270, 1920, 1080);
        var main = Monitor("M", 480 + 2 * Bezel, 0, 480, 270, 1920, 1080, primary: true);
        var right = Monitor("R", 2 * (480 + 2 * Bezel), 0, 480, 270, 1920, 1080);

        var solved = PixelLocationSolver.Solve([left, right, main]);

        Assert.Equal(new Point(0, 0), solved["M"]);
        Assert.Equal(new Point(-1920, 0), solved["L"]);
        Assert.Equal(new Point(1920, 0), solved["R"]);
    }

    [Fact]
    public void VerticallyStackedWithDifferentWidths()
    {
        // Laptop panel under a wide monitor, horizontally centered in mm.
        var a = Monitor("A", 0, 0, 597.7, 336.2, 2560, 1440, primary: true);
        var bX = (597.7 - 302) / 2;
        var b = Monitor("B", bX, 336.2 + 2 * Bezel, 302, 189, 1920, 1200);

        var solved = PixelLocationSolver.Solve([a, b]);

        Assert.Equal(1440, solved["B"].Y);

        var mid = (Math.Max(0, bX) + Math.Min(597.7, bX + 302)) / 2;
        var xOnA = solved["A"].X + mid / (597.7 / 2560);
        var xOnB = solved["B"].X + (mid - bX) / (302.0 / 1920);
        Assert.True(Math.Abs(xOnA - xOnB) <= 1, $"crossing mismatch: {xOnA} vs {xOnB}");
    }
}
