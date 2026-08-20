#nullable enable
using System;
using HLab.Geo;

namespace LittleBigMouse.DisplayLayout.Monitors.Extensions;

/// <summary>
/// Which of the two coordinate axes a rule is expressed on. Every placement rule in this
/// folder is written once for one axis and applied to both; naming the axis is what lets the
/// two directions share the rule instead of each spelling out its own <c>vertical ? Y : X</c>.
/// </summary>
public enum Axis
{
    Horizontal,
    Vertical
}

/// <summary>
/// A segment <c>[Lo, Lo+Size]</c> on one axis — a monitor's extent along X or Y, in whatever
/// unit the caller is working in. Both solvers reason on these: the mm span of a panel, the
/// pixel span the system gives that same panel, the shared part of two spans.
/// </summary>
public readonly record struct Interval(double Lo, double Size)
{
    public double Hi => Lo + Size;

    /// <summary>The same span moved so it starts at <paramref name="lo"/>.</summary>
    public Interval MovedTo(double lo) => this with { Lo = lo };

    /// <summary>
    /// Length shared with <paramref name="other"/>; zero or negative when they do not overlap,
    /// the negative value being the gap between them.
    /// </summary>
    public double OverlapWith(Interval other) => Math.Min(Hi, other.Hi) - Math.Max(Lo, other.Lo);

    /// <summary>
    /// Midpoint of the part shared with <paramref name="other"/>. Defined even when the two do
    /// not overlap — it is then the midpoint of the gap — which is why neither solver needs a
    /// special case for a monitor whose panels only meet through their bezels.
    /// </summary>
    public double SharedMidpoint(Interval other)
        => (Math.Max(Lo, other.Lo) + Math.Min(Hi, other.Hi)) / 2;
}

/// <summary>How a span sits against another one along the axis they meet on.</summary>
public enum EdgeContact
{
    /// <summary>No edge in common on this axis.</summary>
    None,

    /// <summary>The span ends where the anchor starts — it sits on the low side.</summary>
    Before,

    /// <summary>The span starts where the anchor ends — it sits on the high side.</summary>
    After
}

/// <summary>
/// The geometry both placement directions share: read a rect along one axis, measure how two
/// rects intersect, decide whether they meet along an edge. Deliberately free of any monitor,
/// layout or ReactiveUI type, so the rules can be exercised on plain rectangles.
/// </summary>
public static class LayoutGeometry
{
    /// <summary>The axis a contact on <paramref name="axis"/> leaves to be resolved.</summary>
    public static Axis Perpendicular(this Axis axis)
        => axis == Axis.Horizontal ? Axis.Vertical : Axis.Horizontal;

    /// <summary>The extent of <paramref name="rect"/> along <paramref name="axis"/>.</summary>
    public static Interval On(this Rect rect, Axis axis) => axis == Axis.Horizontal
        ? new Interval(rect.X, rect.Width)
        : new Interval(rect.Y, rect.Height);

    /// <summary>The same rect moved so its extent along <paramref name="axis"/> starts at <paramref name="lo"/>.</summary>
    public static Rect MovedOn(this Rect rect, Axis axis, double lo) => axis == Axis.Horizontal
        ? new Rect(new Point(lo, rect.Y), rect.Size)
        : new Rect(new Point(rect.X, lo), rect.Size);

    public static Rect Translated(this Rect rect, double dx, double dy)
        => new(new Point(rect.X + dx, rect.Y + dy), rect.Size);

    /// <summary>Length <paramref name="a"/> and <paramref name="b"/> share along <paramref name="axis"/>.</summary>
    public static double OverlapOn(Rect a, Rect b, Axis axis) => a.On(axis).OverlapWith(b.On(axis));

    /// <summary>True when the two rects share actual surface, a shared edge alone not counting.</summary>
    public static bool Overlap(Rect a, Rect b)
        => OverlapOn(a, b, Axis.Horizontal) > 0 && OverlapOn(a, b, Axis.Vertical) > 0;

    /// <summary>
    /// Whether <paramref name="other"/> meets <paramref name="anchor"/> along one of their edges
    /// on this axis, allowing <paramref name="tolerance"/> of slack.
    /// <para>
    /// The two directions call this with very different slack and for very different reasons:
    /// zero on pixel edges, which the system reports as exact integers, and a few millimetres on
    /// bezel edges, which are hand-entered and rarely add up to exactly zero. The rule itself is
    /// the same, and so is the precedence — <see cref="EdgeContact.After"/> is tested first.
    /// Both readings can only hold at once for a span of zero or negative size.
    /// </para>
    /// </summary>
    public static EdgeContact ContactBetween(Interval anchor, Interval other, double tolerance)
    {
        if (Math.Abs(other.Lo - anchor.Hi) <= tolerance) return EdgeContact.After;
        if (Math.Abs(other.Hi - anchor.Lo) <= tolerance) return EdgeContact.Before;
        return EdgeContact.None;
    }

    /// <inheritdoc cref="ContactBetween(Interval,Interval,double)"/>
    public static EdgeContact ContactBetween(Rect anchor, Rect other, Axis axis, double tolerance)
        => ContactBetween(anchor.On(axis), other.On(axis), tolerance);
}

/// <summary>
/// One monitor reduced to a single axis: the mm span of its panel and the pixel span the system
/// gives that same panel. The two describe the same physical strip, so a coordinate can be
/// carried from one to the other — which is the whole content of <see cref="EdgeProjection"/>.
/// </summary>
public readonly record struct AxisProfile(Interval Mm, Interval Pixel)
{
    /// <summary>Millimetres per pixel along this axis.</summary>
    public double Pitch => Mm.Size / Pixel.Size;

    /// <summary>A monitor reporting no pixels has nothing to be proportional to.</summary>
    public bool HasPixels => Pixel.Size > 0;

    public double ToPixel(double mm) => Pixel.Lo + (mm - Mm.Lo) / Pitch;

    public double ToMm(double pixel) => Mm.Lo + (pixel - Pixel.Lo) * Pitch;

    public AxisProfile AtMm(double lo) => this with { Mm = Mm.MovedTo(lo) };

    public AxisProfile AtPixel(double lo) => this with { Pixel = Pixel.MovedTo(lo) };
}

/// <summary>
/// The single invariant the two placement directions share, and the two ways to solve it.
/// <para>
/// When two monitors meet along an edge, their position on the perpendicular axis is fixed by
/// requiring that <em>the physical midpoint of the span their panels share maps to the same
/// coordinate on both</em> — the cursor crosses at that point without a jump, which is the least
/// distortion obtainable without the engine running.
/// </para>
/// <para>
/// "Place from system" knows both pixel positions and solves for the millimetre one;
/// "apply to system" knows both millimetre positions and solves for the pixel one. Same
/// equation, opposite unknown, and keeping the two literally the same expression is what makes
/// a round trip through the pair land where it started instead of drifting on every pass.
/// </para>
/// </summary>
public static class EdgeProjection
{
    /// <summary>
    /// The millimetre-space solve is implicit — the shared span moves with the answer — so it is
    /// iterated. The overlap can only change while the estimate crosses a span end, and each pass
    /// settles the estimate that follows from the current one, so a handful of passes reach the
    /// fixed point and further ones change nothing.
    /// </summary>
    const int SettlePasses = 4;

    const double Settled = 1e-9;

    /// <summary>
    /// Pixel origin <paramref name="target"/> must take for the invariant to hold against
    /// <paramref name="anchor"/>, whose own pixel origin is already known. Closed form: the mm
    /// positions of both are given, so the shared span does not depend on the answer. Unrounded —
    /// the caller decides, since the system only accepts integers.
    /// </summary>
    public static double PixelOrigin(AxisProfile anchor, AxisProfile target)
    {
        var mid = anchor.Mm.SharedMidpoint(target.Mm);
        return anchor.ToPixel(mid) - (mid - target.Mm.Lo) / target.Pitch;
    }

    /// <summary>
    /// Millimetre origin <paramref name="target"/> must take for the same invariant to hold, both
    /// pixel origins being known this time.
    /// <para>
    /// Seeded from the pixel-space midpoint, which is always defined and is already the answer
    /// whenever the two spans cover each other, then settled onto the millimetre-space midpoint —
    /// the one <see cref="PixelOrigin"/> uses. Taking the pixel midpoint and stopping there is
    /// closed-form and tempting, and it round-trips perfectly right up until two monitors have
    /// very different pitches, which is the case this code exists for.
    /// </para>
    /// </summary>
    public static double MillimetreOrigin(AxisProfile anchor, AxisProfile target)
    {
        var midPixel = anchor.Pixel.SharedMidpoint(target.Pixel);
        var mm = anchor.ToMm(midPixel) - (midPixel - target.Pixel.Lo) * target.Pitch;

        for (var pass = 0; pass < SettlePasses; pass++)
        {
            var mid = anchor.Mm.SharedMidpoint(target.AtMm(mm).Mm);
            var next = mid - (anchor.ToPixel(mid) - target.Pixel.Lo) * target.Pitch;

            if (Math.Abs(next - mm) < Settled) break;
            mm = next;
        }

        return mm;
    }
}

/// <summary>
/// One monitor as the placement solvers see it: an identity, the millimetre rect of its panel
/// (<c>DepthProjection.Bounds</c>), the millimetre rect including bezels
/// (<c>DepthProjection.OutsideBounds</c>), the pixel rect the system positions it with
/// (<c>ActiveSource.Source.InPixel.Bounds</c>) and whether it is the primary.
/// <para>
/// A plain record, so the geometry stays testable without building a reactive model graph, and
/// so both directions can be fed the very same snapshot list — which is what a round-trip test
/// needs. Each solver reads its own subset: "place from system" treats the pixel rect as the
/// input and the mm origins as the answer, "apply to system" does the opposite and only needs
/// the pixel <em>size</em>.
/// </para>
/// </summary>
public sealed record MonitorSnapshot(
    string Id,
    Rect MmBounds,
    Rect MmOutsideBounds,
    Rect PixelBounds,
    bool Primary)
{
    public Point MmLocation => MmBounds.Location;

    public Size PixelSize => PixelBounds.Size;

    public AxisProfile Profile(Axis axis) => new(MmBounds.On(axis), PixelBounds.On(axis));

    /// <summary>The bezel between the panel and the outside rect, on the low side of this axis.</summary>
    public double LowBorder(Axis axis) => MmBounds.On(axis).Lo - MmOutsideBounds.On(axis).Lo;

    /// <summary>The same monitor with its panel origin moved to <paramref name="mm"/>, bezels following.</summary>
    public MonitorSnapshot MovedTo(Point mm) => this with
    {
        MmBounds = new Rect(mm, MmBounds.Size),
        MmOutsideBounds = MmOutsideBounds.Translated(mm.X - MmBounds.X, mm.Y - MmBounds.Y)
    };

    /// <summary>
    /// The compaction view of this monitor. Compaction has no pixel dimension at all, so it keeps
    /// its own input record rather than carry a field it must never read.
    /// </summary>
    public CompactionMonitor ForCompaction() => new(Id, MmBounds, MmOutsideBounds, Primary);
}
