using System;
using System.Collections.Generic;
using System.Linq;

namespace LittleBigMouse.Plugin.Layout.Avalonia.BorderResistancePlugin;

/// <summary>What a snap target is derived from, so the guide can name its source.</summary>
public enum SnapKind
{
    /// <summary>One of this edge's own two ends.</summary>
    EdgeEnd,

    /// <summary>The middle of this edge.</summary>
    Middle,

    /// <summary>The visible edge of another screen.</summary>
    ScreenEdge,

    /// <summary>A boundary of a section already drawn on a parallel edge.</summary>
    Section
}

/// <summary>A place a boundary wants to land, and where that place comes from.</summary>
public readonly record struct SnapTarget(double Mm, SnapKind Kind);

/// <summary>A section on another parallel edge, in that edge's coordinates.</summary>
public readonly record struct ParallelBorderSpan(double OriginMm, BorderSpan Span);

/// <summary>Pure target collection and snapping rules for border sections.</summary>
public static class BorderSnapEngine
{
    public const double MatchToleranceMm = 0.001;

    public static IReadOnlyList<SnapTarget> BuildTargets(
        double edgeLengthMm,
        double edgeOriginMm,
        bool isVertical,
        IEnumerable<BorderRectangle> otherScreens,
        IEnumerable<BorderSpan> sectionsOnEdge,
        IEnumerable<ParallelBorderSpan> parallelSections)
    {
        var targets = new List<SnapTarget>
        {
            new(0, SnapKind.EdgeEnd),
            new(edgeLengthMm, SnapKind.EdgeEnd),
            new(edgeLengthMm / 2, SnapKind.Middle)
        };

        foreach (var bounds in otherScreens)
        {
            AddIfOnEdge((isVertical ? bounds.Top : bounds.Left) - edgeOriginMm, SnapKind.ScreenEdge);
            AddIfOnEdge((isVertical ? bounds.Bottom : bounds.Right) - edgeOriginMm, SnapKind.ScreenEdge);
        }

        foreach (var section in sectionsOnEdge)
        {
            AddIfOnEdge(section.From, SnapKind.Section);
            AddIfOnEdge(section.To, SnapKind.Section);
        }

        foreach (var section in parallelSections)
        {
            AddIfOnEdge(section.OriginMm + section.Span.From - edgeOriginMm, SnapKind.Section);
            AddIfOnEdge(section.OriginMm + section.Span.To - edgeOriginMm, SnapKind.Section);
        }

        return targets;

        void AddIfOnEdge(double value, SnapKind kind)
        {
            if (value < 0 || value > edgeLengthMm) return;
            if (targets.Any(t => Math.Abs(t.Mm - value) < MatchToleranceMm)) return;
            targets.Add(new SnapTarget(value, kind));
        }
    }

    public static SnapTarget? MatchedTarget(IEnumerable<SnapTarget> targets, double mm)
    {
        foreach (var target in targets)
        {
            if (Math.Abs(target.Mm - mm) < MatchToleranceMm) return target;
        }

        return null;
    }

    public static double Snap(
        double mm,
        double edgeLengthMm,
        double toleranceMm,
        IEnumerable<SnapTarget> targets,
        bool enabled = true)
    {
        var clamped = Math.Clamp(mm, 0, edgeLengthMm);
        if (!enabled) return clamped;

        var best = clamped;
        var bestDistance = toleranceMm;

        foreach (var target in targets)
        {
            var distance = Math.Abs(target.Mm - clamped);
            if (distance >= bestDistance) continue;
            bestDistance = distance;
            best = target.Mm;
        }

        return best;
    }

    public static double SnapMovedStart(
        double wantedFromMm,
        double lengthMm,
        double toleranceMm,
        IEnumerable<SnapTarget> targets)
    {
        var best = wantedFromMm;
        var bestDistance = double.MaxValue;

        foreach (var target in targets)
        {
            Consider(target.Mm);
            Consider(target.Mm - lengthMm);
        }

        return best;

        void Consider(double start)
        {
            var distance = Math.Abs(start - wantedFromMm);
            if (distance > toleranceMm || distance >= bestDistance) return;

            bestDistance = distance;
            best = start;
        }
    }
}
