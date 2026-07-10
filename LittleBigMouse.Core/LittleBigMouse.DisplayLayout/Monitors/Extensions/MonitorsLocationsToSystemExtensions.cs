#nullable enable
using System;
using System.Collections.Generic;
using System.Linq;
using HLab.Geo;

namespace LittleBigMouse.DisplayLayout.Monitors.Extensions;

/// <summary>
/// Solver input: one system-positionable monitor. Mm rects come from DepthProjection
/// (Bounds = panel, OutsideBounds = with bezels); PixelSize is the size the OS positions
/// with (logical pixels on Wayland, physical pixels on Windows/X11).
/// Plain record so the solver stays testable without the reactive model graph.
/// </summary>
public sealed record PixelPlacementMonitor(
    string Id,
    Rect MmBounds,
    Rect MmOutsideBounds,
    Size PixelSize,
    bool Primary);

/// <summary>Result of <see cref="MonitorsLocationsToSystemExtensions.ComputePixelLocationsFromPhysical"/> for one source.</summary>
public sealed record SystemPlacement(Rect PixelBounds, double? Scale);

/// <summary>
/// Inverse of <see cref="MonitorsLocationsFromSystemExtensions.SetLocationsFromSystemConfiguration"/>:
/// derive integer system pixel positions from the physical (mm) layout. The system knows
/// neither bezels nor exact scales, so physical adjacency becomes exact pixel edge contact,
/// and on the perpendicular axis the physical midpoint of the shared span projects to the
/// same pixel coordinate on both sides (least cursor-crossing distortion without the engine).
/// </summary>
public static class PixelLocationSolver
{
    /// <summary>
    /// Physical gaps up to this count as adjacent: bezel measurements are hand-entered,
    /// so touching monitors rarely land at exactly 0mm.
    /// </summary>
    public const double DefaultToleranceMm = 5.0;

    public static IReadOnlyDictionary<string, Point> Solve(
        IReadOnlyList<PixelPlacementMonitor> monitors,
        double toleranceMm = DefaultToleranceMm)
    {
        var placed = new Dictionary<string, Rect>();
        if (monitors.Count == 0) return new Dictionary<string, Point>();

        var primary = monitors.FirstOrDefault(m => m.Primary) ?? monitors[0];
        var order = new List<PixelPlacementMonitor>();

        void Place(PixelPlacementMonitor m, Point pos)
        {
            placed[m.Id] = new Rect(new Point(Math.Round(pos.X), Math.Round(pos.Y)), m.PixelSize);
            order.Add(m);
        }

        Place(primary, new Point(0, 0));

        // Spanning tree over physical adjacency: first placement wins, grid cycles are
        // reconciled by the overlap pass below.
        var todo = new Queue<PixelPlacementMonitor>();
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

    static Point Pitch(PixelPlacementMonitor m) => new(
        m.MmBounds.Width / m.PixelSize.Width,
        m.MmBounds.Height / m.PixelSize.Height);

    /// <summary>
    /// Island ordering needs mm-space distances to the already-placed group, but the
    /// group is stored in pixels: scale each placed rect back to mm through the island's
    /// own pitch — only the relative order matters, not the exact value.
    /// </summary>
    static Func<Rect, Rect> FromPixels(PixelPlacementMonitor m)
    {
        var pitch = Pitch(m);
        return r => new Rect(r.X * pitch.X, r.Y * pitch.Y, r.Width * pitch.X, r.Height * pitch.Y);
    }

    static bool TryPlaceAdjacent(
        PixelPlacementMonitor a, Rect aPx,
        PixelPlacementMonitor b, double tolerance,
        out Point pos)
    {
        var oa = a.MmOutsideBounds;
        var ob = b.MmOutsideBounds;

        var verticalOverlap = Math.Min(oa.Bottom, ob.Bottom) - Math.Max(oa.Y, ob.Y) > 0;
        var horizontalOverlap = Math.Min(oa.Right, ob.Right) - Math.Max(oa.X, ob.X) > 0;

        if (verticalOverlap && Math.Abs(ob.X - oa.Right) <= tolerance)
        {
            pos = new Point(aPx.Right, PerpendicularOffset(a, aPx.Y, b, vertical: true));
            return true;
        }
        if (verticalOverlap && Math.Abs(oa.X - ob.Right) <= tolerance)
        {
            pos = new Point(aPx.X - b.PixelSize.Width, PerpendicularOffset(a, aPx.Y, b, vertical: true));
            return true;
        }
        if (horizontalOverlap && Math.Abs(ob.Y - oa.Bottom) <= tolerance)
        {
            pos = new Point(PerpendicularOffset(a, aPx.X, b, vertical: false), aPx.Bottom);
            return true;
        }
        if (horizontalOverlap && Math.Abs(oa.Y - ob.Bottom) <= tolerance)
        {
            pos = new Point(PerpendicularOffset(a, aPx.X, b, vertical: false), aPx.Y - b.PixelSize.Height);
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
    static double PerpendicularOffset(PixelPlacementMonitor a, double aPxOrigin, PixelPlacementMonitor b, bool vertical)
    {
        double aLo, aSize, bLo, bSize, aPxSize, bPxSize;
        if (vertical)
        {
            (aLo, aSize, aPxSize) = (a.MmBounds.Y, a.MmBounds.Height, a.PixelSize.Height);
            (bLo, bSize, bPxSize) = (b.MmBounds.Y, b.MmBounds.Height, b.PixelSize.Height);
        }
        else
        {
            (aLo, aSize, aPxSize) = (a.MmBounds.X, a.MmBounds.Width, a.PixelSize.Width);
            (bLo, bSize, bPxSize) = (b.MmBounds.X, b.MmBounds.Width, b.PixelSize.Width);
        }

        var mid = (Math.Max(aLo, bLo) + Math.Min(aLo + aSize, bLo + bSize)) / 2;

        var pitchA = aSize / aPxSize;
        var pitchB = bSize / bPxSize;

        return Math.Round(aPxOrigin + (mid - aLo) / pitchA - (mid - bLo) / pitchB);
    }

    /// <summary>
    /// Pixel-space equivalent of <see cref="PhysicalMonitor.PlaceAuto"/>: slide into the
    /// group band when no single translation can touch, then translate by the smallest
    /// positive distance. Results stay integer because inputs are integer.
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

            // Like PlaceAuto: slide along the axis with the smaller gap so the rect
            // straddles the group band, then one translation on the other axis touches.
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
            var conflict = others.FirstOrDefault(r =>
                Math.Min(r.Right, rect.Right) - Math.Max(r.X, rect.X) > 0 &&
                Math.Min(r.Bottom, rect.Bottom) - Math.Max(r.Y, rect.Y) > 0);
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

public static class MonitorsLocationsToSystemExtensions
{
    /// <summary>
    /// Compute the system pixel configuration matching the physical layout. With
    /// <paramref name="adjustScale"/> (Wayland only) per-output scales are recomputed so a
    /// logical pixel covers the same physical size everywhere (the primary's current logical
    /// pitch), quantized to 1/120 — the fractional-scale protocol unit KWin rounds to.
    /// Scale is null when unchanged; Windows callers ignore it entirely.
    /// </summary>
    public static Dictionary<DisplaySource, SystemPlacement> ComputePixelLocationsFromPhysical(
        this IMonitorsLayout layout, bool adjustScale = false)
    {
        var result = new Dictionary<DisplaySource, SystemPlacement>();

        var primary = layout.PrimaryMonitor;
        if (primary?.ActiveSource == null) return result;

        var monitors = layout.PhysicalMonitors
            .Where(m => m.ActiveSource?.Source.AttachedToDesktop == true)
            .ToList();
        if (monitors.Count == 0) return result;

        var inputs = new List<PixelPlacementMonitor>();
        var sizes = new Dictionary<string, (DisplaySource Source, Size PixelSize, double? Scale)>();

        // The primary's logical pitch (mm per logical pixel) is the homogeneity target:
        // its own scale never changes, everything else converges to it.
        var targetPitch = primary.DepthProjection.Bounds.Width / primary.ActiveSource.Source.InPixel.Width;

        foreach (var monitor in monitors)
        {
            var source = monitor.ActiveSource.Source;
            var pixelSize = source.InPixel.Bounds.Size;
            double? newScale = null;

            if (adjustScale)
            {
                var scale = source.EffectiveDpi.X / 96;
                var native = new Size(source.InPixel.Width * scale, source.InPixel.Height * scale);
                var physicalPitch = monitor.DepthProjection.Bounds.Width / native.Width;

                var wanted = Math.Round(targetPitch / physicalPitch * 120) / 120;
                wanted = Math.Clamp(wanted, 0.5, 3.0);

                // 1/240 = half a protocol step: below that the quantized scale is the
                // one the compositor already runs, don't emit a no-op change.
                if (Math.Abs(wanted - scale) >= 1.0 / 240)
                {
                    newScale = wanted;
                    pixelSize = new Size(Math.Round(native.Width / wanted), Math.Round(native.Height / wanted));
                }
            }

            inputs.Add(new PixelPlacementMonitor(
                monitor.Id,
                monitor.DepthProjection.Bounds,
                monitor.DepthProjection.OutsideBounds,
                pixelSize,
                monitor == primary));
            sizes[monitor.Id] = (source, pixelSize, newScale);
        }

        var solved = PixelLocationSolver.Solve(inputs);

        foreach (var (id, position) in solved)
        {
            var (source, pixelSize, scale) = sizes[id];
            result[source] = new SystemPlacement(new Rect(position, pixelSize), scale);
        }

        return result;
    }
}
