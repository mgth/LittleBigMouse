#nullable enable
using System;
using System.Collections.Generic;
using System.Linq;
using HLab.Geo;

namespace LittleBigMouse.DisplayLayout.Monitors.Extensions;

/// <summary>
/// Solver input: one monitor reduced to what placement actually depends on — its
/// identity and its mm outside bounds (panel plus bezels, i.e.
/// <c>DepthProjection.OutsideBounds</c>). Plain record so the geometry stays testable
/// without the reactive model graph, same contract as <see cref="PixelPlacementMonitor"/>.
/// </summary>
public sealed record CompactionMonitor(string Id, Rect OutsideBounds, bool Primary);

/// <summary>The two layout options compaction obeys, snapshotted so the solve is a pure function.</summary>
public readonly record struct CompactionOptions(bool AllowOverlaps, bool AllowDiscontinuity);

/// <summary>
/// Pure geometry behind <see cref="MonitorsLayout.ForceCompact"/>: resolve overlaps, group
/// touching monitors into clusters, then pull each cluster against the primary's, and settle
/// until nothing overlaps and everything forms a single block.
///
/// Works on immutable inputs and returns translations, never absolute positions: every
/// caller applies them to a reactive <c>DepthProjection</c> whose X/Y are panel coordinates
/// while the solver reasons in outside (bezel) coordinates. The two differ by a constant
/// border offset, so a translation transfers between them unchanged.
///
/// Contact means a shared edge of non-zero length. Two monitors meeting at a single corner
/// are NOT connected: the cursor cannot cross a point, so a corner-only meeting is a hole in
/// the layout rather than a link, and compaction has to keep pulling until a real edge is
/// shared.
///
/// <see cref="PixelLocationSolver"/> reaches the same requirement by another route: it calls
/// <c>DistanceToTouch</c> with <c>zero: true</c>, which counts a zero-length perpendicular
/// overlap as a gap and so falls through to its slide-then-touch branch. This solver takes the
/// requirement from the connectivity rule itself, which is what its cluster phase needs.
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

        // Working copy: the solve mutates these rects the way the reactive model used to be
        // mutated in place, so each step sees the results of the previous one.
        var bounds = new Rect[ordered.Count];
        for (var i = 0; i < ordered.Count; i++) bounds[i] = ordered[i].OutsideBounds;

        if (!options.AllowOverlaps) ResolveOverlaps(bounds, primary);

        if (!options.AllowDiscontinuity)
        {
            PullClustersTogether(bounds, primary);

            // A cluster is pulled against the monitors anchored SO FAR — the clusters still
            // queued behind it are invisible obstacles, so it can land straight on top of
            // one. The victim is then already overlapping when its own turn comes, and a
            // pull only knows how to close gaps, so the overlap used to survive to the end.
            // Push the pair apart and pull back together whatever that split off, until the
            // layout is settled. Real layouts need at most one extra pass; the bound only
            // guards against pathological inputs.
            if (!options.AllowOverlaps)
                for (var pass = 0; pass < SettlePasses && !IsSettled(bounds); pass++)
                {
                    ResolveOverlaps(bounds, primary);
                    PullClustersTogether(bounds, primary);
                }
        }

        for (var i = 0; i < ordered.Count; i++)
        {
            var offset = new Vector(
                bounds[i].X - ordered[i].OutsideBounds.X,
                bounds[i].Y - ordered[i].OutsideBounds.Y);

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
    static void PullClustersTogether(Rect[] bounds, int primary)
    {
        var clusters = BuildClusters(bounds);

        // Primary monitor anchors everything: its cluster never moves.
        var anchored = clusters.First(c => c.Contains(primary));
        var todo = clusters.Where(c => !ReferenceEquals(c, anchored)).ToList();

        while (todo.Count > 0)
        {
            var cluster = todo
                .OrderBy(c => PullCost(bounds, c, anchored))
                .First();
            todo.Remove(cluster);

            var (dx, dy) = ClusterPull(bounds, cluster, anchored);
            if (dx != 0 || dy != 0)
                foreach (var i in cluster) bounds[i] = Translate(bounds[i], dx, dy);

            anchored.AddRange(cluster);
        }
    }

    /// <summary>
    /// Cheapest way to park <paramref name="m"/> against <paramref name="a"/> so the two share
    /// a real edge — one of the four sides, plus whatever slide the perpendicular axis needs.
    ///
    /// That perpendicular coordinate is kept as it is when it already spans a real part of the
    /// anchor, because that is the arrangement the user built. It is only recentred when the
    /// two share nothing on that axis, which is the case that used to produce corner contacts:
    /// closing both gaps then left the monitor meeting the anchor at a single point.
    /// </summary>
    static (double Cost, double Dx, double Dy) BestDock(Rect m, Rect a)
    {
        var overlapX = Math.Min(m.Right, a.Right) - Math.Max(m.X, a.X);
        var overlapY = Math.Min(m.Bottom, a.Bottom) - Math.Max(m.Y, a.Y);

        var slideX = overlapX > ContactEpsilon ? 0 : a.X + a.Width / 2 - (m.X + m.Width / 2);
        var slideY = overlapY > ContactEpsilon ? 0 : a.Y + a.Height / 2 - (m.Y + m.Height / 2);

        // Right of, left of, below, above the anchor.
        Span<(double Dx, double Dy)> options =
        [
            (a.Right - m.X, slideY),
            (a.X - m.Right, slideY),
            (slideX, a.Bottom - m.Y),
            (slideX, a.Y - m.Bottom)
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
    static (double Dx, double Dy) ClusterPull(Rect[] bounds, List<int> cluster, List<int> anchored)
    {
        var best = (Cost: double.PositiveInfinity, Dx: 0.0, Dy: 0.0);

        foreach (var m in cluster)
        foreach (var a in anchored)
        {
            var dock = BestDock(bounds[m], bounds[a]);
            if (dock.Cost >= best.Cost) continue;

            best = dock;
        }

        return (best.Dx, best.Dy);
    }

    /// <summary>How far this cluster has to travel to dock — the "nearest first" ordering key.</summary>
    static double PullCost(Rect[] bounds, List<int> cluster, List<int> anchored)
    {
        var best = double.PositiveInfinity;

        foreach (var m in cluster)
        foreach (var a in anchored)
            best = Math.Min(best, BestDock(bounds[m], bounds[a]).Cost);

        return best;
    }

    /// <summary>
    /// Push overlapping monitors apart. The primary never moves. Out of the four ways to
    /// leave an overlap, take the shortest push that does not land on yet another monitor
    /// (a blind least-penetration push can bounce between two neighbours forever); fall back
    /// to the shortest push when every direction is occupied, and iterate until stable.
    /// </summary>
    static void ResolveOverlaps(Rect[] bounds, int primary)
    {
        for (var pass = 0; pass < bounds.Length + 4; pass++)
        {
            var moved = false;
            for (var i = 0; i < bounds.Length; i++)
            {
                if (i == primary) continue;

                var overlapped = FirstOverlapping(bounds, bounds[i], i);
                if (overlapped < 0) continue;

                var d = bounds[i].Distance(bounds[overlapped]);

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
                    c => FirstOverlapping(bounds, Translate(bounds[i], c.dx, c.dy), i) < 0,
                    candidates[0]);

                bounds[i] = Translate(bounds[i], free.dx, free.dy);
                moved = true;
            }
            if (!moved) return;
        }
    }

    /// <summary>Index of the first monitor <paramref name="rect"/> overlaps, ignoring <paramref name="self"/>.</summary>
    static int FirstOverlapping(Rect[] bounds, Rect rect, int self)
    {
        for (var i = 0; i < bounds.Length; i++)
        {
            if (i == self) continue;
            if (Overlap(rect, bounds[i])) return i;
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
    /// Connected means touching (or overlapping) on both axes AND sharing a segment of
    /// non-zero length on at least one of them. A corner-only meeting is deliberately not
    /// connected — the cursor cannot cross a single point.
    /// </summary>
    static bool AreConnected(Rect a, Rect b)
    {
        var d = a.Distance(b);

        // Positive = gap on that axis; negative = length the two actually share.
        var gapX = Math.Max(d.Left, d.Right);
        var gapY = Math.Max(d.Top, d.Bottom);

        if (gapX > ContactEpsilon || gapY > ContactEpsilon) return false;

        return Math.Min(gapX, gapY) < -ContactEpsilon;
    }

    /// <summary>
    /// Connected components over the "touches or overlaps" relation, as index lists.
    /// </summary>
    static List<List<int>> BuildClusters(Rect[] bounds)
    {
        var clusters = new List<List<int>>();
        var remaining = new List<int>(Enumerable.Range(0, bounds.Length));

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
                    if (!cluster.Any(m => AreConnected(bounds[m], bounds[remaining[i]]))) continue;
                    cluster.Add(remaining[i]);
                    remaining.RemoveAt(i);
                    grown = true;
                }
            }
            clusters.Add(cluster);
        }
        return clusters;
    }

    /// <summary>What compaction is meant to produce: no overlap left, and one single block.</summary>
    static bool IsSettled(Rect[] bounds)
    {
        for (var i = 0; i < bounds.Length; i++)
            if (FirstOverlapping(bounds, bounds[i], i) >= 0) return false;

        return BuildClusters(bounds).Count == 1;
    }
}
