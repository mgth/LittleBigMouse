using HLab.Geo;
using LittleBigMouse.DisplayLayout.Monitors;
using LittleBigMouse.DisplayLayout.Monitors.Extensions;

namespace LittleBigMouse.DisplayLayout.Tests;

/// <summary>
/// The placement geometry of <see cref="MonitorsLayout.ForceCompact"/>, tested directly on
/// <see cref="CompactionSolver"/>: immutable rects in, translations out, no reactive graph.
/// <see cref="ForceCompactTests"/> keeps covering the same rules through the live model.
/// </summary>
public class CompactionSolverTests
{
    /// <summary>Suite-wide corridor requirement, mirroring the option's default.</summary>
    const double Req = 20.0;

    static readonly CompactionOptions Compacting =
        new(AllowOverlaps: false, AllowDiscontinuity: false, MinimalEdgeOverlap: Req);

    /// <summary>
    /// Zero bezel by default, so panel and outside bounds coincide and position assertions
    /// stay readable; pass a bezel to exercise the panel/outside distinction.
    /// </summary>
    static CompactionMonitor M(string id, double x, double y, double w, double h,
        bool primary = false, double bezel = 0)
        => new(id,
            new Rect(x, y, w, h),
            new Rect(x - bezel, y - bezel, w + 2 * bezel, h + 2 * bezel),
            primary);

    /// <summary>Apply the solved translations, so assertions read as final PANEL positions.</summary>
    static Dictionary<string, Rect> Place(IReadOnlyList<CompactionMonitor> monitors, CompactionOptions? options = null)
    {
        var offsets = CompactionSolver.Solve(monitors, options ?? Compacting);
        return monitors.ToDictionary(
            m => m.Id,
            m => offsets.TryGetValue(m.Id, out var d)
                ? new Rect(m.Bounds.X + d.X, m.Bounds.Y + d.Y, m.Bounds.Width, m.Bounds.Height)
                : m.Bounds);
    }

    static bool Overlaps(Rect a, Rect b)
        => Math.Min(a.Right, b.Right) - Math.Max(a.X, b.X) > CompactionSolver.ContactEpsilon
        && Math.Min(a.Bottom, b.Bottom) - Math.Max(a.Y, b.Y) > CompactionSolver.ContactEpsilon;

    /// <summary>Length of display surface the two share along their contact, mm.</summary>
    static double Corridor(Rect a, Rect b) => Math.Max(
        Math.Min(a.Right, b.Right) - Math.Max(a.X, b.X),
        Math.Min(a.Bottom, b.Bottom) - Math.Max(a.Y, b.Y));

    /// <summary>
    /// Mirrors CompactionSolver.AreConnected on zero-bezel rects: bezel contact (a corner
    /// counts) plus, when required, the crossing corridor on the panels.
    /// </summary>
    static bool Touches(Rect a, Rect b, double req = Req)
    {
        var d = a.Distance(b);

        if (Math.Max(d.Left, d.Right) > CompactionSolver.ContactEpsilon
            || Math.Max(d.Top, d.Bottom) > CompactionSolver.ContactEpsilon)
            return false;

        return req <= 0 || Corridor(a, b) >= req - CompactionSolver.ContactEpsilon;
    }

    /// <summary>True when every monitor is reachable from every other through contacts.</summary>
    static bool IsConnected(ICollection<Rect> rects, double req = Req)
    {
        if (rects.Count == 0) return true;
        var all = rects.ToList();
        var seen = new List<Rect> { all[0] };
        var todo = all.Skip(1).ToList();

        for (var grown = true; grown;)
        {
            grown = false;
            for (var i = todo.Count - 1; i >= 0; i--)
            {
                if (!seen.Any(s => Touches(s, todo[i], req))) continue;
                seen.Add(todo[i]);
                todo.RemoveAt(i);
                grown = true;
            }
        }
        return todo.Count == 0;
    }

    static string Signature(Dictionary<string, Rect> placed) => string.Join(
        " ",
        placed.OrderBy(p => p.Key, StringComparer.Ordinal)
              .Select(p => $"{p.Key}={Math.Round(p.Value.X, 6)},{Math.Round(p.Value.Y, 6)}"));

    // ---------------------------------------------------------------- overlaps

    [Fact]
    public void Overlap_IsResolved_AndPrimaryNeverMoves()
    {
        var monitors = new[]
        {
            M("A", 0, 0, 700, 400, primary: true),
            M("B", 300, 100, 700, 400),
        };

        var placed = Place(monitors);

        Assert.Equal(new Rect(0, 0, 700, 400), placed["A"]);
        Assert.False(Overlaps(placed["A"], placed["B"]));
        Assert.True(Touches(placed["A"], placed["B"]));
    }

    [Fact]
    public void Overlap_ShortestFreePushWins()
    {
        // B sits 100mm into A's right edge: the shortest way out is 100mm to the right,
        // not 600mm back to the left.
        var monitors = new[]
        {
            M("A", 0, 0, 700, 400, primary: true),
            M("B", 600, 0, 700, 400),
        };

        var placed = Place(monitors);

        Assert.Equal(700, placed["B"].X, 6);
        Assert.Equal(0, placed["B"].Y, 6);
    }

    [Fact]
    public void ExactlyStackedMonitors_AreSeparated()
    {
        // Fully coincident rects: degenerate input must still terminate and separate.
        var monitors = new[]
        {
            M("A", 0, 0, 700, 400, primary: true),
            M("B", 0, 0, 700, 400),
            M("C", 0, 0, 700, 400),
        };

        var placed = Place(monitors);

        Assert.Equal(new Rect(0, 0, 700, 400), placed["A"]);
        foreach (var (x, y) in new[] { ("A", "B"), ("A", "C"), ("B", "C") })
            Assert.False(Overlaps(placed[x], placed[y]), $"{x}/{y} still overlap");
    }

    [Fact]
    public void AllowOverlaps_LeavesOverlapsAlone()
    {
        var monitors = new[]
        {
            M("A", 0, 0, 700, 400, primary: true),
            M("B", 300, 100, 700, 400),
        };

        var placed = Place(monitors,
            new CompactionOptions(AllowOverlaps: true, AllowDiscontinuity: false, MinimalEdgeOverlap: Req));

        // Already one cluster (they touch), so the compaction phase has nothing to pull.
        Assert.Equal(new Rect(300, 100, 700, 400), placed["B"]);
    }

    // ---------------------------------------------------------------- clusters

    [Fact]
    public void DisjointClusters_ArePulledIntoContact()
    {
        var monitors = new[]
        {
            M("A", 0, 0, 700, 400, primary: true),
            M("B", 2000, 0, 700, 400),
        };

        var placed = Place(monitors);

        Assert.Equal(700, placed["B"].X, 6);
        Assert.Equal(0, placed["B"].Y, 6);
    }

    [Fact]
    public void Cluster_TravelsAsOneRigidBlock()
    {
        // {B, C} touch each other but are far from the primary: they must arrive together,
        // keeping their 200mm relative offset, rather than each grabbing a free edge.
        var monitors = new[]
        {
            M("A", 0, 0, 700, 400, primary: true),
            M("B", 1500, -300, 1650, 920),
            M("C", 1700, -700, 700, 400),
        };

        var placed = Place(monitors);

        var db = placed["B"].X - 1500;
        var dc = placed["C"].X - 1700;
        Assert.Equal(db, dc, 6);
        Assert.Equal(placed["B"].Y - -300, placed["C"].Y - -700, 6);
        Assert.Equal(700, placed["B"].X, 6);
    }

    [Fact]
    public void ThreeDisjointIslands_AllEndUpConnected()
    {
        var monitors = new[]
        {
            M("A", 0, 0, 700, 400, primary: true),
            M("B", 5000, 0, 700, 400),
            M("C", -5000, 3000, 700, 400),
            M("D", 9000, -4000, 700, 400),
        };

        var placed = Place(monitors);

        Assert.True(IsConnected(placed.Values), $"layout still disjoint: {Signature(placed)}");
        Assert.Equal(new Rect(0, 0, 700, 400), placed["A"]);
    }

    [Fact]
    public void NearestClusterIsPulledFirst()
    {
        // The far TV comes first in the input; the near neighbour must still take the
        // primary's right edge, with the TV landing beyond it (issue #450).
        var monitors = new[]
        {
            M("C_tv", 1454, -294, 1650, 920),
            M("A_primary", 0, 0, 700, 400, primary: true),
            M("B_near", 754, 16, 700, 400),
        };

        var placed = Place(monitors);

        Assert.Equal(700, placed["B_near"].X, 6);
        Assert.Equal(16, placed["B_near"].Y, 6);
        Assert.Equal(1400, placed["C_tv"].X, 6);
        Assert.Equal(-294, placed["C_tv"].Y, 6);
    }

    // ------------------------------------------------------------- compaction

    [Fact]
    public void AlreadyCompactLayout_IsLeftUntouched()
    {
        var monitors = new[]
        {
            M("A", 0, 0, 700, 400, primary: true),
            M("B", 700, 0, 700, 400),
            M("C", 1400, 0, 700, 400),
        };

        Assert.Empty(CompactionSolver.Solve(monitors, Compacting));
    }

    [Fact]
    public void Compaction_IsIdempotent()
    {
        var monitors = new[]
        {
            M("A", 0, 0, 700, 400, primary: true),
            M("B", 1200, 250, 600, 340),
            M("C", -900, -700, 800, 350),
            M("D", 2500, 1500, 1650, 920),
        };

        var placed = Place(monitors);
        var again = monitors.Select(m => m with { Bounds = placed[m.Id], OutsideBounds = placed[m.Id] }).ToList();

        Assert.Empty(CompactionSolver.Solve(again, Compacting));
    }

    [Fact]
    public void AllowDiscontinuity_ResolvesOverlapsButKeepsGaps()
    {
        var monitors = new[]
        {
            M("A", 0, 0, 700, 400, primary: true),
            M("B", 300, 100, 700, 400),  // overlapping
            M("C", 5000, 0, 700, 400),   // far away
        };

        var placed = Place(monitors,
            new CompactionOptions(AllowOverlaps: false, AllowDiscontinuity: true, MinimalEdgeOverlap: Req));

        Assert.False(Overlaps(placed["A"], placed["B"]));
        Assert.Equal(new Rect(5000, 0, 700, 400), placed["C"]);
    }

    // ------------------------------------------------------------ degenerate

    [Fact]
    public void NoPrimary_ProducesNoOffsets()
    {
        var monitors = new[]
        {
            M("A", 0, 0, 700, 400),
            M("B", 5000, 0, 700, 400),
        };

        Assert.Empty(CompactionSolver.Solve(monitors, Compacting));
    }

    [Fact]
    public void SingleMonitor_ProducesNoOffsets()
        => Assert.Empty(CompactionSolver.Solve([M("A", 90, 90, 700, 400, primary: true)], Compacting));

    [Fact]
    public void Primary_IsNeverTranslated()
    {
        var monitors = new[]
        {
            M("A", 123, -456, 700, 400, primary: true),
            M("B", 4000, 4000, 700, 400),
            M("C", 200, -300, 700, 400),
        };

        Assert.False(CompactionSolver.Solve(monitors, Compacting).ContainsKey("A"));
    }

    // ----------------------------------------------- determinism / permutation

    /// <summary>
    /// Every ordering of the same monitors must compact identically. The caller's order comes
    /// from an unkeyed SourceCache binding — arbitrary, and not stable between runs — so any
    /// order sensitivity here is a layout that rearranges itself for no reason.
    /// </summary>
    [Fact]
    public void InputOrderPermutation_DoesNotChangeResult()
    {
        var monitors = new[]
        {
            M("A", 0, 0, 700, 400, primary: true),
            M("B", 900, 30, 600, 340),
            M("C", -1200, -500, 1650, 920),
            M("D", 850, 900, 800, 350),
            M("E", 2400, -1400, 340, 600),
        };

        var expected = Signature(Place(monitors));
        var count = 0;

        foreach (var order in Permutations(monitors.Length))
        {
            var permuted = order.Select(i => monitors[i]).ToList();
            Assert.Equal(expected, Signature(Place(permuted)));
            count++;
        }

        Assert.Equal(120, count);
    }

    /// <summary>200 generated layouts — touching, gapped, overlapping, disjoint.</summary>
    static IEnumerable<(int Case, List<CompactionMonitor> Monitors, int Primary)> GeneratedLayouts()
    {
        var rng = new Lcg(0xC0FFEE);
        (double W, double H)[] panels =
            [(700, 400), (600, 340), (1650, 920), (520, 330), (800, 350), (340, 600)];

        for (var c = 0; c < 200; c++)
        {
            var count = 2 + rng.Next(5);
            var monitors = new List<CompactionMonitor>();
            var primary = rng.Next(count);

            for (var j = 0; j < count; j++)
            {
                var (w, h) = panels[rng.Next(panels.Length)];
                var x = (rng.Next(9) - 4) * 500 + (rng.Next(21) - 10) * 20;
                var y = (rng.Next(7) - 3) * 450 + (rng.Next(21) - 10) * 20;
                monitors.Add(M($"M{j}", x, y, w, h, primary: j == primary));
            }

            yield return (c, monitors, primary);
        }
    }

    /// <summary>
    /// The two invariants that hold for every layout, over the whole generated spread:
    /// the result does not depend on input order, and the primary never moves.
    /// </summary>
    [Fact]
    public void GeneratedLayouts_AreOrderInvariant_AndKeepPrimaryAnchored()
    {
        foreach (var (c, monitors, primary) in GeneratedLayouts())
        {
            var placed = Place(monitors);
            var reference = Signature(placed);

            // A few rotations plus the reverse catch encounter-order leaks without
            // running every permutation of every case.
            for (var r = 1; r < monitors.Count; r++)
            {
                var rotated = monitors.Skip(r).Concat(monitors.Take(r)).ToList();
                Assert.Equal(reference, Signature(Place(rotated)));
            }
            Assert.Equal(reference, Signature(Place(monitors.AsEnumerable().Reverse().ToList())));

            Assert.Equal(monitors[primary].OutsideBounds, placed[$"M{primary}"]);
        }
    }

    /// <summary>
    /// The post-conditions of a compaction, over the whole generated spread and in both
    /// regimes — bare contact (0, the corner-crossing case) and a demanded corridor: nothing
    /// overlaps and everything forms one block connected at that requirement. The spread is
    /// deliberately harsher than real desktops — monitors scattered over 4m with no relation
    /// to one another. Under the loose regime these used to fail on 11 and 55 of the 200
    /// layouts; see <see cref="PulledClusterLandingOnAQueuedOne_IsRepaired"/> and
    /// <see cref="DiagonalPull_LandsInContact_NotBetweenTwoMonitors"/> for the mechanisms.
    /// </summary>
    [Theory]
    [InlineData(0.0)]
    [InlineData(20.0)]
    [InlineData(40.0)]
    public void GeneratedLayouts_EndUpOverlapFree_AndConnected(double req)
    {
        var options = Compacting with { MinimalEdgeOverlap = req };
        var total = 0;

        foreach (var (c, monitors, _) in GeneratedLayouts())
        {
            var placed = Place(monitors, options);
            var rects = placed.Values.ToList();
            total++;

            for (var i = 0; i < rects.Count; i++)
                for (var j = i + 1; j < rects.Count; j++)
                    Assert.False(Overlaps(rects[i], rects[j]),
                        $"case {c}: overlap left in {Signature(placed)}");

            Assert.True(IsConnected(rects, req), $"case {c}: still disjoint: {Signature(placed)}");

            // Settling must reach a fixed point: compacting the result changes nothing.
            var again = monitors.Select(m => m with { Bounds = placed[m.Id], OutsideBounds = placed[m.Id] }).ToList();
            Assert.Empty(CompactionSolver.Solve(again, options));
        }

        Assert.Equal(200, total);
    }

    /// <summary>
    /// A cluster is pulled against the monitors anchored SO FAR, so a cluster still queued
    /// behind it is an invisible obstacle and can be landed on. Here M2 is pulled up onto M1,
    /// which has not been anchored yet; when M1's own turn comes it is already overlapped, and
    /// a pull only closes gaps, so the overlap used to survive. Taken from the generated
    /// corpus (case 40), reduced to the three monitors that matter.
    /// </summary>
    [Fact]
    public void PulledClusterLandingOnAQueuedOne_IsRepaired()
    {
        var monitors = new[]
        {
            M("M0", 1100, -880, 520, 330, primary: true),
            M("M1", -520, 200, 700, 400),
            M("M2", 120, 960, 600, 340),
            M("M3", 900, 1060, 800, 350),
            M("M4", -2060, -310, 700, 400),
        };

        var placed = Place(monitors);

        Assert.False(Overlaps(placed["M1"], placed["M2"]),
            $"M1/M2 still overlap: {Signature(placed)}");
        Assert.True(IsConnected(placed.Values.ToList()), Signature(placed));
    }

    /// <summary>
    /// When no single-axis translation can reach the anchored group, the cluster must still
    /// land on a real edge. Closing the horizontal and vertical gaps against the per-axis
    /// nearest monitors independently left this cluster aligned with M1's bottom edge and M0's
    /// right edge — touching neither. Taken from the generated corpus (case 1).
    /// </summary>
    [Fact]
    public void DiagonalPull_LandsInContact_NotBetweenTwoMonitors()
    {
        var monitors = new[]
        {
            M("M0", 380, -1040, 340, 600),
            M("M1", -620, -720, 340, 600, primary: true),
            M("M2", 800, 610, 520, 330),
        };

        var placed = Place(monitors);

        Assert.True(IsConnected(placed.Values.ToList()),
            $"M2 landed touching nothing: {Signature(placed)}");
        Assert.Equal(new Rect(-620, -720, 340, 600), placed["M1"]);
    }

    /// <summary>
    /// With a corridor demanded, a monitor meeting the primary at exactly one corner is not
    /// done: it must slide — by the MINIMUM, not to the centre — until the panels share the
    /// required span. With no requirement (corner-crossing can cross a corner), the very same
    /// layout is left untouched.
    /// </summary>
    [Fact]
    public void CornerOnlyContact_SlidesMinimally_OrIsAcceptedAtZero()
    {
        var monitors = new[]
        {
            M("A", 0, 0, 700, 400, primary: true),
            M("B", 700, 400, 600, 340), // touches A at the single point (700,400)
        };

        // req = 20: slide up by exactly the missing 20mm, nothing more.
        var placed = Place(monitors);
        Assert.Equal(new Rect(700, 380, 600, 340), placed["B"]);
        Assert.Equal(Req, Corridor(placed["A"], placed["B"]), 6);

        // req = 0: the corner is a valid contact, B stays where the user put it.
        var loose = Place(monitors, Compacting with { MinimalEdgeOverlap = 0 });
        Assert.Equal(new Rect(700, 400, 600, 340), loose["B"]);
    }

    /// <summary>
    /// Bezels cannot carry the cursor: two monitors whose OUTSIDE rects overlap comfortably
    /// while their panels share nothing must still be slid into a real corridor. With
    /// req = 0 the same bezel-on-bezel contact is accepted as is.
    /// </summary>
    [Fact]
    public void BezelOnlyContact_IsNotACorridor()
    {
        // 20mm bezels: outside rects overlap 30mm vertically, panels are 10mm apart.
        var monitors = new[]
        {
            M("A", 0, 0, 700, 400, primary: true, bezel: 20),
            M("B", 740, 410, 600, 340, bezel: 20),
        };

        var placed = Place(monitors);

        // Slid up 30mm: 20mm of PANELS shared, measured on the display surface.
        Assert.Equal(Req, Corridor(placed["A"], placed["B"]), 6);
        Assert.Equal(new Rect(740, 380, 600, 340), placed["B"]);

        var loose = Place(monitors, Compacting with { MinimalEdgeOverlap = 0 });
        Assert.Equal(new Rect(740, 410, 600, 340), loose["B"]);
    }

    /// <summary>
    /// The slide is the minimal displacement into the valid band, never a recentring: B could
    /// reach a 20mm corridor by moving 220mm up, so it must not travel the 570mm to A's centre.
    /// </summary>
    [Fact]
    public void Slide_IsMinimal_NotACentring()
    {
        var monitors = new[]
        {
            M("A", 0, 0, 700, 400, primary: true),
            M("B", 1200, 600, 600, 340),
        };

        var placed = Place(monitors);

        Assert.Equal(new Rect(700, 380, 600, 340), placed["B"]);
        Assert.Equal(Req, Corridor(placed["A"], placed["B"]), 6);
    }

    [Fact]
    public void RepeatedSolves_ReturnTheSameResult()
    {
        var monitors = new[]
        {
            M("A", 0, 0, 700, 400, primary: true),
            M("B", 1300, 40, 600, 340),
            M("C", 1310, 50, 800, 350),
        };

        var first = Signature(Place(monitors));
        for (var i = 0; i < 5; i++) Assert.Equal(first, Signature(Place(monitors)));
    }

    // ------------------------------------------------------------------ tools

    /// <summary>Deterministic LCG: System.Random's sequence is not contractually stable.</summary>
    sealed class Lcg(ulong seed)
    {
        ulong _s = seed;
        public int Next(int maxExclusive)
        {
            _s = unchecked(_s * 6364136223846793005UL + 1442695040888963407UL);
            return (int)((_s >> 16) % (ulong)maxExclusive);
        }
    }

    static IEnumerable<int[]> Permutations(int n)
    {
        var idx = Enumerable.Range(0, n).ToArray();
        while (true)
        {
            yield return (int[])idx.Clone();

            var i = n - 2;
            while (i >= 0 && idx[i] >= idx[i + 1]) i--;
            if (i < 0) yield break;

            var j = n - 1;
            while (idx[j] <= idx[i]) j--;
            (idx[i], idx[j]) = (idx[j], idx[i]);
            Array.Reverse(idx, i + 1, n - i - 1);
        }
    }
}
