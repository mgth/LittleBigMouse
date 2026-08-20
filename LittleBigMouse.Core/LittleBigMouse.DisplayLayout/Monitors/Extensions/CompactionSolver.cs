#nullable enable
using System;
using System.Collections.Generic;
using System.Linq;
using HLab.Geo;

namespace LittleBigMouse.DisplayLayout.Monitors.Extensions;

/// <summary>
/// Solver input: one monitor reduced to what placement actually depends on — its identity,
/// its mm panel bounds (display surface, <c>DepthProjection.Bounds</c>) and its mm outside
/// bounds (panel plus bezels, <c>DepthProjection.OutsideBounds</c>). Plain record so the
/// geometry stays testable without the reactive model graph — the millimetre half of
/// <see cref="MonitorSnapshot"/>, which builds one through
/// <see cref="MonitorSnapshot.ForCompaction"/>. Compaction has no pixel dimension whatsoever, so
/// it keeps its own input rather than carry a field it must never read.
/// </summary>
public sealed record CompactionMonitor(string Id, Rect Bounds, Rect OutsideBounds, bool Primary);

/// <summary>
/// The layout options compaction obeys, snapshotted so the solve is a pure function.
/// <see cref="MinimalEdgeOverlap"/> is the corridor requirement in mm — see the class docs of
/// <see cref="CompactionSolver"/>.
/// </summary>
public readonly record struct CompactionOptions(
    bool AllowOverlaps,
    bool AllowDiscontinuity,
    double MinimalEdgeOverlap = 0);

/// <summary>
/// Pure geometry behind <see cref="MonitorsLayout.ForceCompact"/>: resolve overlaps, group
/// touching monitors into clusters, then pull each cluster against the primary's, and settle
/// until nothing overlaps and everything forms a single block.
///
/// Works on immutable inputs and returns translations, never absolute positions: every
/// caller applies them to a reactive <c>DepthProjection</c> whose X/Y are panel coordinates
/// while the solver reasons on rects that all translate together, so a translation transfers
/// between the two coordinate systems unchanged.
///
/// Contact is bezel against bezel (outside bounds touching), with corner-to-corner included.
/// On top of that, <see cref="CompactionOptions.MinimalEdgeOverlap"/> demands a crossing
/// corridor: at least that many mm of DISPLAY SURFACE (panels, bezels excluded) shared along
/// the contact. Bezels cannot carry the cursor, so two monitors whose outside rects overlap
/// only bezel-on-bezel are as uncrossable as a corner-only meeting — which is why the
/// requirement is measured on the panels. Zero keeps the bare contact rule.
/// </summary>
public static class CompactionSolver
{
    /// <summary>
    /// Contacts are computed values: allow rounding noise when deciding whether two
    /// monitors touch (or overlap, when overlaps are permitted).
    /// </summary>
    public const double ContactEpsilon = 0.5;

    /// <summary>
    /// Upper bound on push-apart / pull-together rounds after the first pass. Measured need
    /// on generated layouts is one; this is only a termination guarantee.
    /// </summary>
    const int SettlePasses = 8;

    /// <summary>The working copy of one monitor: both rects move together.</summary>
    readonly struct Body(Rect outside, Rect panel)
    {
        public Rect Outside { get; } = outside;
        public Rect Panel { get; } = panel;

        public Body Translate(double dx, double dy) => new(
            new Rect(new Point(Outside.X + dx, Outside.Y + dy), Outside.Size),
            new Rect(new Point(Panel.X + dx, Panel.Y + dy), Panel.Size));
    }

    /// <summary>
    /// Translations to apply, keyed by monitor id. Monitors that must not move are absent
    /// from the result — in particular the primary, which anchors the layout at its current
    /// position and is never returned.
    /// </summary>
    public static IReadOnlyDictionary<string, Vector> Solve(
        IReadOnlyList<CompactionMonitor> monitors,
        CompactionOptions options)
    {
        var result = new Dictionary<string, Vector>();
        if (monitors.Count < 2) return result;

        // Canonical order. Both phases below are order-sensitive — overlaps are pushed apart
        // one monitor at a time, and equally distant clusters are pulled in encounter order —
        // while the caller's order comes from an unkeyed SourceCache binding, i.e. it is
        // arbitrary and may differ between runs for the very same set of monitors. Sorting by
        // id first makes the outcome a function of the monitors alone: the same physical
        // arrangement can no longer compact two different ways (the #450 family of scrambles).
        var ordered = monitors.OrderBy(m => m.Id, StringComparer.Ordinal).ToList();

        // Nothing to pull against until the primary is known.
        var primary = IndexOfPrimary(ordered);
        if (primary < 0) return result;

        var req = options.MinimalEdgeOverlap;

        // Working copy: the solve mutates these bodies the way the reactive model used to be
        // mutated in place, so each step sees the results of the previous one.
        var bodies = new Body[ordered.Count];
        for (var i = 0; i < ordered.Count; i++)
            bodies[i] = new Body(ordered[i].OutsideBounds, ordered[i].Bounds);

        if (!options.AllowOverlaps) ResolveOverlaps(bodies, primary);

        if (!options.AllowDiscontinuity)
        {
            PullClustersTogether(bodies, primary, req);

            // A cluster is pulled against the monitors anchored SO FAR — the clusters still
            // queued behind it are invisible obstacles, so it can land straight on top of
            // one. The victim is then already overlapping when its own turn comes, and a
            // pull only knows how to close gaps, so the overlap used to survive to the end.
            // Push the pair apart and pull back together whatever that split off, until the
            // layout is settled. Real layouts need at most one extra pass; the bound only
            // guards against pathological inputs.
            if (!options.AllowOverlaps)
                for (var pass = 0; pass < SettlePasses && !IsSettled(bodies, req); pass++)
                {
                    ResolveOverlaps(bodies, primary);
                    PullClustersTogether(bodies, primary, req);
                }
        }

        for (var i = 0; i < ordered.Count; i++)
        {
            var offset = new Vector(
                bodies[i].Outside.X - ordered[i].OutsideBounds.X,
                bodies[i].Outside.Y - ordered[i].OutsideBounds.Y);

            if (offset.X != 0 || offset.Y != 0) result[ordered[i].Id] = offset;
        }

        return result;
    }

    static int IndexOfPrimary(IReadOnlyList<CompactionMonitor> monitors)
    {
        for (var i = 0; i < monitors.Count; i++)
            if (monitors[i].Primary) return i;
        return -1;
    }

    /// <summary>
    /// Pull each connected cluster of touching monitors as a RIGID group toward the
    /// primary's cluster, nearest first. Moving clusters — not individual monitors —
    /// preserves the relative arrangement: after a primary drag the whole translated group
    /// comes back as one block instead of each monitor grabbing the first free edge.
    /// </summary>
    static void PullClustersTogether(Body[] bodies, int primary, double req)
    {
        var clusters = BuildClusters(bodies, req);

        // Primary monitor anchors everything: its cluster never moves.
        var anchored = clusters.First(c => c.Contains(primary));
        var todo = clusters.Where(c => !ReferenceEquals(c, anchored)).ToList();

        while (todo.Count > 0)
        {
            var cluster = todo
                .OrderBy(c => PullCost(bodies, c, anchored, req))
                .First();
            todo.Remove(cluster);

            var (dx, dy) = ClusterPull(bodies, cluster, anchored, req);
            if (dx != 0 || dy != 0)
                foreach (var i in cluster) bodies[i] = bodies[i].Translate(dx, dy);

            anchored.AddRange(cluster);
        }
    }

    /// <summary>
    /// Smallest displacement of the span <paramref name="lo"/>..<paramref name="lo"/>+<paramref name="size"/>
    /// so that it overlaps the anchor span by at least <paramref name="required"/> (clamped to
    /// what the two sizes allow). Zero when the current position already satisfies it — the
    /// arrangement the user built is preserved, never recentred.
    /// </summary>
    static double SlideInto(double lo, double size, double anchorLo, double anchorSize, double required)
    {
        var r = Math.Min(required, Math.Min(size, anchorSize));

        var min = anchorLo + r - (lo + size);
        var max = anchorLo + anchorSize - r - lo;

        return Math.Max(min, Math.Min(0.0, max));
    }

    /// <summary>
    /// Cheapest way to park <paramref name="m"/> against <paramref name="a"/>: one of the four
    /// sides, bezels flush, plus the minimal perpendicular slide that yields the required
    /// corridor — on the panels when a corridor is demanded, on the outside bounds (touch,
    /// corner included) when it is not.
    /// </summary>
    static (double Cost, double Dx, double Dy) BestDock(Body m, Body a, double req)
    {
        var slideY = req > 0
            ? SlideInto(m.Panel.Y, m.Panel.Height, a.Panel.Y, a.Panel.Height, req)
            : SlideInto(m.Outside.Y, m.Outside.Height, a.Outside.Y, a.Outside.Height, 0);

        var slideX = req > 0
            ? SlideInto(m.Panel.X, m.Panel.Width, a.Panel.X, a.Panel.Width, req)
            : SlideInto(m.Outside.X, m.Outside.Width, a.Outside.X, a.Outside.Width, 0);

        // Right of, left of, below, above the anchor.
        Span<(double Dx, double Dy)> options =
        [
            (a.Outside.Right - m.Outside.X, slideY),
            (a.Outside.X - m.Outside.Right, slideY),
            (slideX, a.Outside.Bottom - m.Outside.Y),
            (slideX, a.Outside.Y - m.Outside.Bottom)
        ];

        var best = (Cost: double.PositiveInfinity, Dx: 0.0, Dy: 0.0);

        foreach (var (dx, dy) in options)
        {
            // Strictly closer only, so the first option wins ties and the canonical input
            // order makes the choice reproducible.
            var cost = new Vector(dx, dy).Length;
            if (cost >= best.Cost) continue;

            best = (cost, dx, dy);
        }

        return best;
    }

    /// <summary>
    /// Translation bringing <paramref name="cluster"/> into contact with the anchored group:
    /// the cheapest docking over every (cluster monitor, anchored monitor) pair. The whole
    /// cluster then moves by it, so its internal arrangement is preserved.
    /// </summary>
    static (double Dx, double Dy) ClusterPull(Body[] bodies, List<int> cluster, List<int> anchored, double req)
    {
        var best = (Cost: double.PositiveInfinity, Dx: 0.0, Dy: 0.0);

        foreach (var m in cluster)
        foreach (var a in anchored)
        {
            var dock = BestDock(bodies[m], bodies[a], req);
            if (dock.Cost >= best.Cost) continue;

            best = dock;
        }

        return (best.Dx, best.Dy);
    }

    /// <summary>How far this cluster has to travel to dock — the "nearest first" ordering key.</summary>
    static double PullCost(Body[] bodies, List<int> cluster, List<int> anchored, double req)
    {
        var best = double.PositiveInfinity;

        foreach (var m in cluster)
        foreach (var a in anchored)
            best = Math.Min(best, BestDock(bodies[m], bodies[a], req).Cost);

        return best;
    }

    /// <summary>
    /// Push overlapping monitors apart. The primary never moves. Out of the four ways to
    /// leave an overlap, take the shortest push that does not land on yet another monitor
    /// (a blind least-penetration push can bounce between two neighbours forever); fall back
    /// to the shortest push when every direction is occupied, and iterate until stable.
    /// </summary>
    static void ResolveOverlaps(Body[] bodies, int primary)
    {
        for (var pass = 0; pass < bodies.Length + 4; pass++)
        {
            var moved = false;
            for (var i = 0; i < bodies.Length; i++)
            {
                if (i == primary) continue;

                var overlapped = FirstOverlapping(bodies, bodies[i].Outside, i);
                if (overlapped < 0) continue;

                var d = bodies[i].Outside.Distance(bodies[overlapped].Outside);

                // Right, left, below, above — as (dx, dy) displacements, shortest first.
                // OrderBy is stable, so equal-length pushes keep this order.
                var candidates = new[]
                    {
                        (dx: -d.Left, dy: 0.0),
                        (dx: d.Right, dy: 0.0),
                        (dx: 0.0, dy: -d.Top),
                        (dx: 0.0, dy: d.Bottom)
                    }
                    .OrderBy(c => Math.Abs(c.dx) + Math.Abs(c.dy))
                    .ToList();

                var free = candidates.FirstOrDefault(
                    c => FirstOverlapping(bodies, Translate(bodies[i].Outside, c.dx, c.dy), i) < 0,
                    candidates[0]);

                bodies[i] = bodies[i].Translate(free.dx, free.dy);
                moved = true;
            }
            if (!moved) return;
        }
    }

    /// <summary>Index of the first monitor <paramref name="rect"/> overlaps, ignoring <paramref name="self"/>.</summary>
    static int FirstOverlapping(Body[] bodies, Rect rect, int self)
    {
        for (var i = 0; i < bodies.Length; i++)
        {
            if (i == self) continue;
            if (Overlap(rect, bodies[i].Outside)) return i;
        }
        return -1;
    }

    static Rect Translate(Rect rect, double dx, double dy)
        => new(new Point(rect.X + dx, rect.Y + dy), rect.Size);

    static bool Overlap(Rect a, Rect b)
    {
        var d = a.Distance(b);
        return Math.Max(d.Left, d.Right) < -ContactEpsilon
            && Math.Max(d.Top, d.Bottom) < -ContactEpsilon;
    }

    /// <summary>
    /// Connected means bezels in contact (outside bounds touching or overlapping on both
    /// axes, a corner counts) AND, when a corridor is required, at least that many mm of
    /// panel shared on one axis — the span the cursor can actually cross.
    /// </summary>
    static bool AreConnected(Body a, Body b, double req)
    {
        var d = a.Outside.Distance(b.Outside);

        if (Math.Max(d.Left, d.Right) > ContactEpsilon
            || Math.Max(d.Top, d.Bottom) > ContactEpsilon) return false;

        if (req <= 0) return true;

        var corridorX = Math.Min(a.Panel.Right, b.Panel.Right) - Math.Max(a.Panel.X, b.Panel.X);
        var corridorY = Math.Min(a.Panel.Bottom, b.Panel.Bottom) - Math.Max(a.Panel.Y, b.Panel.Y);

        return Math.Max(corridorX, corridorY) >= req - ContactEpsilon;
    }

    /// <summary>What compaction is meant to produce: no overlap left, and one single block.</summary>
    static bool IsSettled(Body[] bodies, double req)
    {
        for (var i = 0; i < bodies.Length; i++)
            if (FirstOverlapping(bodies, bodies[i].Outside, i) >= 0) return false;

        return BuildClusters(bodies, req).Count == 1;
    }

    /// <summary>
    /// Connected components over the "touches or overlaps" relation, as index lists.
    /// </summary>
    static List<List<int>> BuildClusters(Body[] bodies, double req)
    {
        var clusters = new List<List<int>>();
        var remaining = new List<int>(Enumerable.Range(0, bodies.Length));

        while (remaining.Count > 0)
        {
            var cluster = new List<int> { remaining[0] };
            remaining.RemoveAt(0);

            var grown = true;
            while (grown)
            {
                grown = false;
                for (var i = remaining.Count - 1; i >= 0; i--)
                {
                    if (!cluster.Any(m => AreConnected(bodies[m], bodies[remaining[i]], req))) continue;
                    cluster.Add(remaining[i]);
                    remaining.RemoveAt(i);
                    grown = true;
                }
            }
            clusters.Add(cluster);
        }
        return clusters;
    }
}
