using System;
using System.Collections.Generic;

namespace LittleBigMouse.Plugin.Layout.Avalonia.BorderResistancePlugin;

public enum BorderSideKind { Left, Top, Right, Bottom }

/// <summary>A UI-framework-independent rectangle in layout millimetres.</summary>
public readonly record struct BorderRectangle(double Left, double Top, double Right, double Bottom);

public readonly record struct FacingEdgeCandidate<T>(T Value, BorderRectangle Bounds);

public readonly record struct FacingEdge<T>(T Value, BorderSideKind Kind);

/// <summary>Finds monitor edges reachable across a source edge.</summary>
public static class FacingEdgeResolver
{
    public static FacingEdge<T>? FindNearest<T>(
        BorderSideKind sourceKind,
        BorderRectangle source,
        double maximumGapMm,
        IEnumerable<FacingEdgeCandidate<T>> candidates)
    {
        FacingEdge<T>? best = null;
        var bestGap = double.MaxValue;

        foreach (var candidate in FindAll(sourceKind, source, maximumGapMm, candidates))
        {
            var gap = GapTo(sourceKind, source, candidate.Bounds);
            if (gap >= bestGap) continue;

            bestGap = gap;
            best = new FacingEdge<T>(candidate.Value, Opposite(sourceKind));
        }

        return best;
    }

    public static IEnumerable<FacingEdgeCandidate<T>> FindAll<T>(
        BorderSideKind sourceKind,
        BorderRectangle source,
        double maximumGapMm,
        IEnumerable<FacingEdgeCandidate<T>> candidates)
    {
        var vertical = sourceKind is BorderSideKind.Left or BorderSideKind.Right;

        foreach (var candidate in candidates)
        {
            var gap = GapTo(sourceKind, source, candidate.Bounds);
            if (gap < 0 || gap > maximumGapMm) continue;

            var overlaps = vertical
                ? candidate.Bounds.Bottom > source.Top && candidate.Bounds.Top < source.Bottom
                : candidate.Bounds.Right > source.Left && candidate.Bounds.Left < source.Right;

            if (overlaps) yield return candidate;
        }
    }

    static BorderSideKind Opposite(BorderSideKind kind) => kind switch
    {
        BorderSideKind.Left => BorderSideKind.Right,
        BorderSideKind.Right => BorderSideKind.Left,
        BorderSideKind.Top => BorderSideKind.Bottom,
        _ => BorderSideKind.Top
    };

    static double GapTo(BorderSideKind kind, BorderRectangle source, BorderRectangle other) => kind switch
    {
        BorderSideKind.Left => source.Left - other.Right,
        BorderSideKind.Right => other.Left - source.Right,
        BorderSideKind.Top => source.Top - other.Bottom,
        _ => other.Top - source.Bottom
    };
}
