using System.Collections.Generic;
using System.Linq;
using LittleBigMouse.Platform.Linux;

namespace LittleBigMouse.DisplayLayout.Tests;

/// <summary>
/// Correctness of <see cref="KScreenGapGuard.ComputeShifts"/>: one cumulative +1 shift per
/// output at or beyond every shared edge, so each interior boundary opens by exactly one
/// logical pixel while relative alignment is preserved. Exercises the axis independence,
/// pre-existing gaps (no-op), rotation/scale (via logical dimensions), and larger grids.
///
/// A naive O(n^2) reference (<see cref="ReferenceShifts"/>) is kept alongside the production
/// implementation and both are asserted equal on every synthetic layout, so a future rewrite
/// of ComputeShifts is protected against regressions.
/// </summary>
public class KScreenGapGuardTests
{
    static LinuxMonitor M(string name, double x, double y, double w, double h)
        => new()
        {
            ConnectorName = name,
            LogicalX = x,
            LogicalY = y,
            LogicalWidth = w,
            LogicalHeight = h,
            PixelWidth = (int)w,
            PixelHeight = (int)h,
            Enabled = true
        };

    // ---- Naive reference: the "obvious" per-edge algorithm, deliberately unoptimized. ----
    // For every ordered pair that shares an edge, mark the boundary; then each monitor shifts
    // by the number of boundaries at or before its origin. Same contract as ComputeShifts.
    static Dictionary<string, (int Dx, int Dy)> ReferenceShifts(IReadOnlyList<LinuxMonitor> monitors)
    {
        static int R(double v) => (int)System.Math.Round(v);
        static bool Overlaps(int s1, int l1, int s2, int l2)
            => System.Math.Min(s1 + l1, s2 + l2) - System.Math.Max(s1, s2) > 0;

        var xCuts = new HashSet<int>();
        var yCuts = new HashSet<int>();
        foreach (var a in monitors)
        foreach (var b in monitors)
        {
            if (ReferenceEquals(a, b)) continue;
            int ax = R(a.LogicalX), ay = R(a.LogicalY), aw = R(a.LogicalWidth), ah = R(a.LogicalHeight);
            int bx = R(b.LogicalX), by = R(b.LogicalY), bw = R(b.LogicalWidth), bh = R(b.LogicalHeight);
            if (ax + aw == bx && Overlaps(ay, ah, by, bh)) xCuts.Add(bx);
            if (ay + ah == by && Overlaps(ax, aw, bx, bw)) yCuts.Add(by);
        }

        var result = new Dictionary<string, (int Dx, int Dy)>();
        foreach (var m in monitors)
        {
            var dx = xCuts.Count(c => c <= R(m.LogicalX));
            var dy = yCuts.Count(c => c <= R(m.LogicalY));
            if (dx != 0 || dy != 0) result[m.ConnectorName] = (dx, dy);
        }
        return result;
    }

    static void AssertMatchesReference(IReadOnlyList<LinuxMonitor> monitors)
    {
        var actual = KScreenGapGuard.ComputeShifts(monitors);
        var expected = ReferenceShifts(monitors);
        Assert.Equal(expected.OrderBy(e => e.Key), actual.OrderBy(e => e.Key));
    }

    [Fact]
    public void SingleMonitor_NoShift()
    {
        var shifts = KScreenGapGuard.ComputeShifts(new[] { M("A", 0, 0, 1920, 1080) });
        Assert.Empty(shifts);
    }

    [Fact]
    public void TwoSideBySide_RightOutputShiftsByOne()
    {
        var monitors = new[]
        {
            M("A", 0, 0, 1920, 1080),
            M("B", 1920, 0, 1920, 1080),
        };
        var shifts = KScreenGapGuard.ComputeShifts(monitors);

        Assert.False(shifts.ContainsKey("A"));       // origin: no cut at or before it
        Assert.Equal((1, 0), shifts["B"]);           // one vertical edge crossed
        AssertMatchesReference(monitors);
    }

    [Fact]
    public void TwoStacked_BottomOutputShiftsByOne()
    {
        var monitors = new[]
        {
            M("A", 0, 0, 1920, 1080),
            M("B", 0, 1080, 1920, 1080),
        };
        var shifts = KScreenGapGuard.ComputeShifts(monitors);

        Assert.False(shifts.ContainsKey("A"));
        Assert.Equal((0, 1), shifts["B"]);
        AssertMatchesReference(monitors);
    }

    [Fact]
    public void AlreadyGapped_NoShift_Idempotent()
    {
        // A pre-existing 1px void between them: no shared edge, nothing to open.
        var monitors = new[]
        {
            M("A", 0, 0, 1920, 1080),
            M("B", 1921, 0, 1920, 1080),
        };
        Assert.Empty(KScreenGapGuard.ComputeShifts(monitors));

        // The output of a first Apply fed back in must produce no further shift.
        var gapped = new[]
        {
            M("A", 0, 0, 1920, 1080),
            M("B", 1921, 0, 1920, 1080),
        };
        Assert.Empty(KScreenGapGuard.ComputeShifts(gapped));
    }

    [Fact]
    public void OverlapOnlyAtCorner_DoesNotCount()
    {
        // B's left touches A's right but only at a single corner point (no y-overlap span):
        // not a crossable edge, must not shift. Guards the 2x2 corner quirk.
        var monitors = new[]
        {
            M("A", 0, 0, 1920, 1080),
            M("B", 1920, 1080, 1920, 1080),
        };
        var shifts = KScreenGapGuard.ComputeShifts(monitors);
        Assert.Empty(shifts);
        AssertMatchesReference(monitors);
    }

    [Fact]
    public void ThreeInARow_CumulativeShift()
    {
        var monitors = new[]
        {
            M("A", 0, 0, 1000, 1000),
            M("B", 1000, 0, 1000, 1000),
            M("C", 2000, 0, 1000, 1000),
        };
        var shifts = KScreenGapGuard.ComputeShifts(monitors);

        Assert.False(shifts.ContainsKey("A"));
        Assert.Equal((1, 0), shifts["B"]);   // one edge before it
        Assert.Equal((2, 0), shifts["C"]);   // two edges before it
        AssertMatchesReference(monitors);
    }

    [Fact]
    public void ScaledAndRotated_UsesLogicalDimensions()
    {
        // A portrait, HiDPI panel: logical size is what the source already computed
        // (mode / scale, swapped for rotation). ComputeShifts only sees logical geometry.
        var monitors = new[]
        {
            M("A", 0, 0, 1280, 720),   // 2560x1440 @2x
            M("B", 1280, 0, 720, 1280), // rotated portrait, logical
        };
        var shifts = KScreenGapGuard.ComputeShifts(monitors);
        Assert.Equal((1, 0), shifts["B"]);
        AssertMatchesReference(monitors);
    }

    [Fact]
    public void TwoByTwoGrid_MatchesReference()
    {
        var monitors = new[]
        {
            M("TL", 0, 0, 1920, 1080),
            M("TR", 1920, 0, 1920, 1080),
            M("BL", 0, 1080, 1920, 1080),
            M("BR", 1920, 1080, 1920, 1080),
        };
        var shifts = KScreenGapGuard.ComputeShifts(monitors);

        Assert.False(shifts.ContainsKey("TL"));
        Assert.Equal((1, 0), shifts["TR"]);
        Assert.Equal((0, 1), shifts["BL"]);
        Assert.Equal((1, 1), shifts["BR"]);
        AssertMatchesReference(monitors);
    }

    [Theory]
    [InlineData(2)]
    [InlineData(6)]
    [InlineData(16)]
    [InlineData(32)]
    public void HorizontalStrip_MatchesReferenceAtScale(int count)
    {
        var monitors = Enumerable.Range(0, count)
            .Select(i => M($"DP-{i}", i * 1920, 0, 1920, 1080))
            .ToArray();

        var shifts = KScreenGapGuard.ComputeShifts(monitors);

        // First output never moves, each subsequent one accumulates exactly one more pixel.
        Assert.False(shifts.ContainsKey("DP-0"));
        for (var i = 1; i < count; i++)
            Assert.Equal((i, 0), shifts[$"DP-{i}"]);

        AssertMatchesReference(monitors);
    }

    [Theory]
    [InlineData(4)]   // 2x2
    [InlineData(16)]  // 4x4
    [InlineData(36)]  // 6x6
    public void SquareGrid_MatchesReference(int total)
    {
        var side = (int)System.Math.Sqrt(total);
        var monitors = new List<LinuxMonitor>();
        for (var r = 0; r < side; r++)
        for (var c = 0; c < side; c++)
            monitors.Add(M($"R{r}C{c}", c * 1920, r * 1080, 1920, 1080));

        var shifts = KScreenGapGuard.ComputeShifts(monitors);

        // Cumulative on both axes: cell (r,c) shifts (c, r).
        for (var r = 0; r < side; r++)
        for (var c = 0; c < side; c++)
        {
            var expected = (c, r);
            if (expected == (0, 0)) Assert.False(shifts.ContainsKey($"R{r}C{c}"));
            else Assert.Equal(expected, shifts[$"R{r}C{c}"]);
        }

        AssertMatchesReference(monitors);
    }
}
