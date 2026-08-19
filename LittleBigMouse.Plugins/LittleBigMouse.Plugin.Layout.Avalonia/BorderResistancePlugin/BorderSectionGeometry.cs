using System;
using System.Collections.Generic;
using System.Linq;

namespace LittleBigMouse.Plugin.Layout.Avalonia.BorderResistancePlugin;

/// <summary>An interval along a monitor edge, expressed in millimetres.</summary>
public readonly record struct BorderSpan(double From, double To)
{
    public double Length => To - From;
}

/// <summary>
/// Pure interval arithmetic used while drawing, resizing, moving and mirroring
/// border sections.
/// </summary>
public static class BorderSectionGeometry
{
    public static BorderSpan Order(double first, double second) =>
        first <= second ? new(first, second) : new(second, first);

    /// <summary>The free interval containing <paramref name="referenceMm"/>.</summary>
    public static BorderSpan FreeGapAround(
        IEnumerable<BorderSpan> occupied, double referenceMm, double edgeLengthMm)
    {
        var low = 0.0;
        var high = edgeLengthMm;

        foreach (var other in occupied.OrderBy(s => s.From))
        {
            if (other.From < referenceMm && other.To > referenceMm)
                return new BorderSpan(referenceMm, referenceMm);

            if (other.To <= referenceMm && other.To > low) low = other.To;
            if (other.From >= referenceMm && other.From < high) high = other.From;
        }

        return new BorderSpan(low, high);
    }

    /// <summary>The longest free part of <paramref name="wanted"/>, if one exists.</summary>
    public static BorderSpan? LargestFreeSpan(
        IEnumerable<BorderSpan> occupied, BorderSpan wanted)
    {
        var best = new BorderSpan(0, 0);
        var cursor = wanted.From;

        foreach (var section in occupied
                     .Where(s => s.To > wanted.From && s.From < wanted.To)
                     .OrderBy(s => s.From))
        {
            if (section.From > cursor)
                Consider(cursor, Math.Min(section.From, wanted.To));

            cursor = Math.Max(cursor, section.To);
            if (cursor >= wanted.To) break;
        }

        if (cursor < wanted.To) Consider(cursor, wanted.To);

        return best.Length > 0 ? best : null;

        void Consider(double low, double high)
        {
            if (high - low > best.Length) best = new BorderSpan(low, high);
        }
    }

    /// <summary>Restricts an interval to the supplied free gap.</summary>
    public static BorderSpan ClampToFreeSpace(BorderSpan wanted, BorderSpan freeGap) =>
        new(Math.Max(wanted.From, freeGap.From), Math.Min(wanted.To, freeGap.To));

    public static BorderSpan? Create(
        IEnumerable<BorderSpan> occupied,
        double anchorMm,
        double toMm,
        double edgeLengthMm,
        double minimumLengthMm)
    {
        var wanted = Order(anchorMm, toMm);
        var free = FreeGapAround(occupied, anchorMm, edgeLengthMm);
        var result = ClampToFreeSpace(wanted, free);
        return result.Length >= minimumLengthMm ? result : null;
    }

    public static BorderSpan? Resize(
        IEnumerable<BorderSpan> occupiedWithoutCurrent,
        BorderSpan current,
        double fromMm,
        double toMm,
        double edgeLengthMm,
        double minimumLengthMm)
    {
        var reference = (current.From + current.To) / 2;
        var free = FreeGapAround(occupiedWithoutCurrent, reference, edgeLengthMm);
        var result = ClampToFreeSpace(Order(fromMm, toMm), free);
        return result.Length >= minimumLengthMm ? result : null;
    }

    public static BorderSpan? Expand(
        IEnumerable<BorderSpan> occupiedWithoutCurrent,
        BorderSpan current,
        double edgeLengthMm,
        double minimumLengthMm)
    {
        var result = FreeGapAround(
            occupiedWithoutCurrent, (current.From + current.To) / 2, edgeLengthMm);
        return result.Length >= minimumLengthMm ? result : null;
    }

    public static BorderSpan? CreateFilling(
        IEnumerable<BorderSpan> occupied,
        double atMm,
        double edgeLengthMm,
        double minimumLengthMm)
    {
        var result = FreeGapAround(occupied, atMm, edgeLengthMm);
        return result.Length >= minimumLengthMm ? result : null;
    }

    public static BorderSpan? Move(
        IEnumerable<BorderSpan> occupiedWithoutCurrent,
        BorderSpan current,
        double wantedFromMm,
        double edgeLengthMm)
    {
        var free = FreeGapAround(
            occupiedWithoutCurrent, (current.From + current.To) / 2, edgeLengthMm);

        if (free.Length < current.Length) return null;

        var from = Math.Clamp(wantedFromMm, free.From, free.To - current.Length);
        return new BorderSpan(from, from + current.Length);
    }

    /// <summary>Projects a section into a facing edge and retains its largest free part.</summary>
    public static BorderSpan? PlanMirror(
        double sourceOriginMm,
        BorderSpan source,
        double targetOriginMm,
        double targetLengthMm,
        IEnumerable<BorderSpan> targetOccupied,
        double minimumLengthMm)
    {
        var wanted = new BorderSpan(
            Math.Clamp(sourceOriginMm + source.From - targetOriginMm, 0, targetLengthMm),
            Math.Clamp(sourceOriginMm + source.To - targetOriginMm, 0, targetLengthMm));

        var free = LargestFreeSpan(targetOccupied, wanted);
        return free is { Length: var length } && length >= minimumLengthMm ? free : null;
    }
}
