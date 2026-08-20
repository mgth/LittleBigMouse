#nullable enable
using System.Collections.Generic;
using System.Linq;
using HLab.Geo;

namespace LittleBigMouse.DisplayLayout.Monitors.Extensions;

/// <summary>
/// Inverse of <see cref="PixelLocationSolver"/>: derive the physical (mm) layout from the pixel
/// configuration the system reports. Walks out from the primary over pixel-space edge adjacency
/// and turns each contact into a millimetre position, so monitors the user sees as touching in
/// the system's display settings come out touching here too.
///
/// Works on immutable snapshots and returns positions, never mutating a model: every caller
/// applies the result to a reactive <c>DepthProjection</c>, whose X/Y are the panel origin —
/// the same point as <see cref="MonitorSnapshot.MmLocation"/>.
///
/// What it deliberately does NOT do is close the gaps it leaves. Monitors with no pixel
/// adjacency to anything placed keep whatever their alignment hints gave them, and pulling
/// those into the block is <see cref="CompactionSolver"/>'s job, which the caller runs after.
/// </summary>
public static class SystemLocationSolver
{
    /// <summary>
    /// Pixel edges are integers the system reports verbatim: two monitors are adjacent when
    /// their edges are equal, not merely close.
    /// </summary>
    public const double PixelTolerance = 0;

    /// <summary>
    /// Millimetre panel origins, keyed by monitor id, for the monitors that actually move.
    /// Monitors that end where they started are absent — the caller writes to a reactive model,
    /// and an unchanged write is still a write.
    /// </summary>
    /// <param name="monitors">Every monitor of the layout: any of them can serve as an anchor.</param>
    /// <param name="toPlace">
    /// Ids allowed to move, or null for all of them. The "place already placed monitors too"
    /// switch: a monitor left out anchors nothing and moves nowhere, exactly as before.
    /// </param>
    public static IReadOnlyDictionary<string, Point> Solve(
        IReadOnlyList<MonitorSnapshot> monitors,
        IReadOnlySet<string>? toPlace = null)
    {
        var result = new Dictionary<string, Point>();

        var primary = monitors.FirstOrDefault(m => m.Primary);
        if (primary is null) return result;

        // Where every monitor currently sits. A monitor no rule ever fires on keeps this
        // position — which for a freshly built layout is the origin, where they are all stacked.
        var positions = monitors.ToDictionary(m => m.Id, m => m.MmLocation);

        var unplaced = monitors
            .Where(m => toPlace is null || toPlace.Contains(m.Id))
            .ToList();

        if (unplaced.Count == 0) return result;

        // Start with the primary display and walk outwards.
        var todo = new Queue<MonitorSnapshot>();
        todo.Enqueue(primary);

        while (todo.Count > 0)
        {
            foreach (var monitor in todo) unplaced.RemoveAll(m => m.Id == monitor.Id);

            var placed = todo.Dequeue();
            var placedAt = positions[placed.Id];

            foreach (var target in unplaced.ToList())
            {
                if (target.Id == placed.Id) continue;

                var (position, adjacent) = Against(placed, placedAt, target, positions[target.Id]);

                // Hints fire whether or not the monitor is claimed, so the position is kept
                // either way.
                positions[target.Id] = position;

                if (!adjacent) continue;

                unplaced.RemoveAll(m => m.Id == target.Id);

                // No compacting here. Every monitor not yet reached is still stacked at the
                // origin, so a compact run mid-walk resolves overlaps against screens that have
                // no position yet — and shoves the ones just placed correctly out of the way. It
                // is why running this twice used to give a better answer than running it once:
                // the second pass started from a layout that was already spread out, so the same
                // compacts found nothing to do.
                todo.Enqueue(target);
            }
        }

        foreach (var monitor in monitors)
        {
            var position = positions[monitor.Id];
            if (position != monitor.MmLocation) result[monitor.Id] = position;
        }

        return result;
    }

    /// <summary>
    /// Everything one already-placed monitor says about where another one goes, and whether that
    /// amounts to claiming it.
    /// <para>
    /// Only a crossable pixel-space edge adjacency PLACES a monitor — one the two actually
    /// straddle, a shared corner not counting. Everything else here is a hint: applying a hint
    /// must not consume the monitor, or one whose only real adjacency is with a later-placed
    /// neighbour never gets its adjacency rule tested (e.g. P|S|H side by side: H aligned on P by
    /// Y equality used to be swallowed before S could claim it; and the diagonal of a 2x2 grid,
    /// claimed by the primary on both axes at once, used to be dragged into the primary's bezels).
    /// </para>
    /// <para>
    /// Contact rule, then alignment hints, then edge projection — each stage overwriting the
    /// coordinate the previous one set. That precedence is the behaviour, not an accident; see
    /// each stage for what it exists to override.
    /// </para>
    /// </summary>
    static (Point Position, bool Adjacent) Against(
        MonitorSnapshot placed, Point placedAt, MonitorSnapshot target, Point current)
    {
        // The anchor at its solved position; the target's own position is the unknown, so only
        // its sizes and bezels are ever read.
        var anchor = placed.MovedTo(placedAt);

        var position = current;
        var horizontal = Claim(anchor, target, Axis.Horizontal, ref position);
        var vertical = Claim(anchor, target, Axis.Vertical, ref position);

        // The offset along the contact edge, on the axis the contact is NOT on. Last, so it
        // overrides the alignment equalities in Claim: those only fire on an exact match, and a
        // monitor sitting 219px higher than its neighbour matches none of them — it used to keep
        // whatever it had and be shoved by the compact, landing a whole screen height away.
        if (horizontal) Project(anchor, target, Axis.Vertical, ref position);
        if (vertical) Project(anchor, target, Axis.Horizontal, ref position);

        return (position, horizontal || vertical);
    }

    /// <summary>
    /// The contact rule and the two alignment hints on one axis. Returns whether a CROSSABLE
    /// contact fired, which is the only thing that claims the monitor — every rule here suggests
    /// a position, none of the others consumes the target.
    /// </summary>
    static bool Claim(MonitorSnapshot anchor, MonitorSnapshot target, Axis axis, ref Point position)
    {
        // Contact: flush against the anchor's bezel edge, plus the target's own bezel, so the two
        // panels end up as far apart as their frames make them.
        //
        //     __                    B |___|_
        //  __| A       and its       A  |    |
        // B  |__       transposes
        //  __|
        var contact = LayoutGeometry.ContactBetween(anchor.PixelBounds, target.PixelBounds, axis, PixelTolerance);

        // A shared edge the two monitors do not straddle is a corner meeting, and the cursor has
        // nowhere to cross it — the same rule <see cref="PixelLocationSolver.TryPlaceAdjacent"/>
        // applies in the other direction. The contact still SUGGESTS a position, so a monitor
        // whose only neighbour is diagonal keeps landing on that diagonal rather than staying
        // stacked at the origin, but it no longer claims: claiming on both axes at once is what
        // used to let the two perpendicular projections overwrite each other's coordinate and
        // pull a diagonal neighbour panel-flush into its anchor's bezels.
        var crossable = LayoutGeometry.OverlapOn(anchor.PixelBounds, target.PixelBounds, axis.Perpendicular()) > 0;

        var outside = anchor.MmOutsideBounds.On(axis);

        position = contact switch
        {
            EdgeContact.After => position.With(axis, outside.Hi + target.LowBorder(axis)),
            EdgeContact.Before => position.With(axis,
                outside.Lo - target.MmOutsideBounds.On(axis).Size + target.LowBorder(axis)),
            _ => position
        };

        // Alignment hints, panel edge to panel edge. They come after the contact rule and
        // override it: two monitors flush on one edge in pixels are meant to read as flush in
        // millimetres, whichever other side they also happen to touch on.
        //
        //  __        ___   ___      __            |___||___|
        // |     and |   | |   |       |    and
        // |__                       __|
        var anchorPixel = anchor.PixelBounds.On(axis);
        var targetPixel = target.PixelBounds.On(axis);
        var anchorPanel = anchor.MmBounds.On(axis);

        if (targetPixel.Lo == anchorPixel.Lo) position = position.With(axis, anchorPanel.Lo);
        if (targetPixel.Hi == anchorPixel.Hi)
            position = position.With(axis, anchorPanel.Hi - target.MmBounds.On(axis).Size);

        return contact != EdgeContact.None && crossable;
    }

    /// <summary>Solve the shared-midpoint invariant along <paramref name="axis"/>.</summary>
    static void Project(MonitorSnapshot anchor, MonitorSnapshot target, Axis axis, ref Point position)
    {
        var anchorProfile = anchor.Profile(axis);
        var targetProfile = target.Profile(axis);

        // A monitor reporting no pixels has nothing to be proportional to.
        if (!anchorProfile.HasPixels || !targetProfile.HasPixels) return;

        position = position.With(axis, EdgeProjection.MillimetreOrigin(anchorProfile, targetProfile));
    }

    static Point With(this Point point, Axis axis, double value)
        => axis == Axis.Horizontal ? new Point(value, point.Y) : new Point(point.X, value);
}
