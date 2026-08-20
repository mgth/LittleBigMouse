#nullable enable
using System;
using System.Collections.Generic;
using System.Linq;
using HLab.Geo;

namespace LittleBigMouse.DisplayLayout.Monitors.Extensions;

/// <summary>
/// Inverse of <see cref="SystemLocationSolver"/>: derive integer system pixel positions from the
/// physical (mm) layout. The system knows neither bezels nor exact scales, so physical adjacency
/// becomes exact pixel edge contact, and on the perpendicular axis the shared-span invariant of
/// <see cref="EdgeProjection"/> is solved the other way round — which is what makes a round trip
/// through the two directions land where it started.
///
/// Reads the millimetre rects of each <see cref="MonitorSnapshot"/> and only the SIZE of its
/// pixel rect: the pixel positions are the answer, not an input.
/// </summary>
public static class PixelLocationSolver
{
    /// <summary>
    /// Physical gaps up to this count as adjacent: bezel measurements are hand-entered,
    /// so touching monitors rarely land at exactly 0mm.
    /// </summary>
    public const double DefaultToleranceMm = 5.0;

    public static IReadOnlyDictionary<string, Point> Solve(
        IReadOnlyList<MonitorSnapshot> monitors,
        double toleranceMm = DefaultToleranceMm)
    {
        var placed = new Dictionary<string, Rect>();
        if (monitors.Count == 0) return new Dictionary<string, Point>();

        var primary = monitors.FirstOrDefault(m => m.Primary) ?? monitors[0];
        var order = new List<MonitorSnapshot>();

        void Place(MonitorSnapshot m, Point pos)
        {
            placed[m.Id] = new Rect(new Point(Math.Round(pos.X), Math.Round(pos.Y)), m.PixelSize);
            order.Add(m);
        }

        Place(primary, new Point(0, 0));

        // Spanning tree over physical adjacency: first placement wins, grid cycles are
        // reconciled by the overlap pass below.
        var todo = new Queue<MonitorSnapshot>();
        todo.Enqueue(primary);
        while (todo.Count > 0)
        {
            var a = todo.Dequeue();
            foreach (var b in monitors)
            {
                if (placed.ContainsKey(b.Id)) continue;
                if (!TryPlaceAdjacent(a, placed[a.Id], b, toleranceMm, out var pos)) continue;
                Place(b, pos);
                todo.Enqueue(b);
            }
        }

        // Physically detached islands: keep their mm arrangement at the primary's pitch,
        // then snap into contact — Windows requires a connected desktop, and on KWin a
        // gap makes the edge uncrossable without the engine.
        var pitchPrimary = Pitch(primary);
        foreach (var b in monitors
                     .Where(m => !placed.ContainsKey(m.Id))
                     .OrderBy(m => m.MmOutsideBounds.DistanceToTouch(placed.Values.Select(FromPixels(m))).DistanceHV())
                     .ToList())
        {
            var estimate = new Rect(new Point(
                Math.Round((b.MmBounds.X - primary.MmBounds.X) / pitchPrimary.X),
                Math.Round((b.MmBounds.Y - primary.MmBounds.Y) / pitchPrimary.Y)), b.PixelSize);
            Place(b, SnapToTouch(estimate, placed.Values.ToList()).Location);
        }

        // Rounding and inconsistent cycle constraints can leave small overlaps: push each
        // monitor (placement order, never the primary) out of the ones placed before it.
        for (var i = 1; i < order.Count; i++)
        {
            var others = order.Take(i).Select(m => placed[m.Id]).ToList();
            placed[order[i].Id] = ResolveOverlap(placed[order[i].Id], others);
        }

        // Re-anchor: repairs only move non-primary monitors, but keep the invariant hard.
        var origin = placed[primary.Id].Location;
        return monitors.Where(m => placed.ContainsKey(m.Id)).ToDictionary(
            m => m.Id,
            m => new Point(placed[m.Id].X - origin.X, placed[m.Id].Y - origin.Y));
    }

    static Point Pitch(MonitorSnapshot m) => new(
        m.Profile(Axis.Horizontal).Pitch,
        m.Profile(Axis.Vertical).Pitch);

    /// <summary>
    /// Island ordering needs mm-space distances to the already-placed group, but the
    /// group is stored in pixels: scale each placed rect back to mm through the island's
    /// own pitch — only the relative order matters, not the exact value.
    /// </summary>
    static Func<Rect, Rect> FromPixels(MonitorSnapshot m)
    {
        var pitch = Pitch(m);
        return r => new Rect(r.X * pitch.X, r.Y * pitch.Y, r.Width * pitch.X, r.Height * pitch.Y);
    }

    /// <summary>
    /// Where <paramref name="b"/> goes if it is docked against <paramref name="a"/>: the bezels
    /// meet within tolerance on one axis, the panels genuinely overlap on the other. The axis
    /// carrying the contact fixes one coordinate exactly, the other comes from
    /// <see cref="PerpendicularOffset"/>.
    /// </summary>
    static bool TryPlaceAdjacent(
        MonitorSnapshot a, Rect aPx,
        MonitorSnapshot b, double tolerance,
        out Point pos)
    {
        foreach (var axis in Axes)
        {
            // A shared edge with no overlap across it is a corner meeting, which the cursor
            // cannot cross.
            if (LayoutGeometry.OverlapOn(a.MmOutsideBounds, b.MmOutsideBounds, axis.Perpendicular()) <= 0) continue;

            var contact = LayoutGeometry.ContactBetween(a.MmOutsideBounds, b.MmOutsideBounds, axis, tolerance);
            if (contact == EdgeContact.None) continue;

            var anchorPx = aPx.On(axis);
            var lo = contact == EdgeContact.After ? anchorPx.Hi : anchorPx.Lo - b.PixelBounds.On(axis).Size;

            pos = Origin(axis, lo, PerpendicularOffset(a, aPx, b, axis.Perpendicular()));
            return true;
        }

        pos = default;
        return false;
    }

    /// <summary>
    /// Perpendicular coordinate of b so that the physical midpoint of the shared span maps
    /// to the same pixel on both monitors. Shared span is taken on the panels (pixels do
    /// not cover bezels); when only the bezels overlap the midpoint between the two panel
    /// spans is still the right anchor, so no special case is needed.
    /// </summary>
    static double PerpendicularOffset(MonitorSnapshot a, Rect aPx, MonitorSnapshot b, Axis axis)
        => Math.Round(EdgeProjection.PixelOrigin(
            a.Profile(axis).AtPixel(aPx.On(axis).Lo),
            b.Profile(axis)));

    static Point Origin(Axis axis, double along, double across) => axis == Axis.Horizontal
        ? new Point(along, across)
        : new Point(across, along);

    static readonly Axis[] Axes = [Axis.Horizontal, Axis.Vertical];

    /// <summary>
    /// Snap a detached island into contact: slide into the group band when no single
    /// translation can touch, then translate by the smallest positive distance. Results stay
    /// integer because inputs are integer.
    /// </summary>
    static Rect SnapToTouch(Rect rect, List<Rect> others)
    {
        var distance = rect.DistanceToTouch(others, true);

        if (distance.IsPositiveInfinity())
        {
            var left = others.Min(r => r.X);
            var top = others.Min(r => r.Y);
            var right = others.Max(r => r.Right);
            var bottom = others.Max(r => r.Bottom);

            var toLeft = left - rect.Right;
            var toTop = top - rect.Bottom;
            var toRight = rect.X - right;
            var toBottom = rect.Y - bottom;

            // Slide along the axis with the smaller gap so the rect straddles the group
            // band, then one translation on the other axis touches.
            if (Math.Max(toLeft, toRight) <= Math.Max(toTop, toBottom))
                rect.X = toLeft >= toRight ? left - Math.Round(rect.Width / 2) : right - Math.Round(rect.Width / 2);
            else
                rect.Y = toTop >= toBottom ? top - Math.Round(rect.Height / 2) : bottom - Math.Round(rect.Height / 2);

            distance = rect.DistanceToTouch(others, true);
        }

        var min = distance.MinPositive();
        if (min > 0 && !double.IsInfinity(min))
        {
            if (distance.Left > 0 && distance.Left <= min) rect.X -= distance.Left;
            else if (distance.Top > 0 && distance.Top <= min) rect.Y -= distance.Top;
            else if (distance.Right > 0 && distance.Right <= min) rect.X += distance.Right;
            else if (distance.Bottom > 0 && distance.Bottom <= min) rect.Y += distance.Bottom;
        }

        return ResolveOverlap(rect, others);
    }

    static Rect ResolveOverlap(Rect rect, List<Rect> others)
    {
        // Each push is along one axis by the smallest amount; a few iterations settle
        // rounding-sized overlaps, the bound only guards against pathological inputs.
        for (var i = 0; i < 8; i++)
        {
            var conflict = others.FirstOrDefault(r => LayoutGeometry.Overlap(r, rect));
            if (conflict.IsEmpty || conflict is { Width: 0, Height: 0 }) break;

            var moves = new (double Amount, bool Horizontal)[]
            {
                (conflict.X - rect.Right, true),
                (conflict.Right - rect.X, true),
                (conflict.Y - rect.Bottom, false),
                (conflict.Bottom - rect.Y, false),
            };
            var best = moves.OrderBy(m => Math.Abs(m.Amount)).First();
            if (best.Horizontal) rect.X += best.Amount;
            else rect.Y += best.Amount;
        }
        return rect;
    }
}
