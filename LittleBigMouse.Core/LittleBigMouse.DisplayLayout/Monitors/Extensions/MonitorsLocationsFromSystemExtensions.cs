#nullable enable
using System;
using System.Collections.Generic;
using System.Linq;

namespace LittleBigMouse.DisplayLayout.Monitors.Extensions;

public static class MonitorsLocationsFromSystemExtensions
{

    static readonly object CompactLock = new();

    /// <summary>
    /// Re-anchor the physical layout so the primary monitor sits at (0,0) mm, the
    /// same convention as pixel coordinates. Saved locations are anchored on the
    /// primary that was active at save time: after a primary change they restore
    /// the same relative geometry anchored elsewhere, which breaks the code relying
    /// on the convention (dragging the primary translates every other monitor).
    /// Pure translation: relative geometry is preserved, no compacting needed.
    /// Must be called sequentially once the layout is complete and loaded — not in
    /// reaction to PrimaryMonitor changes, which go through partially built states.
    /// </summary>
    public static void AnchorOnPrimary(this IMonitorsLayout layout)
    {
        var primary = layout.PrimaryMonitor;
        if (primary == null) return;

        var dx = primary.DepthProjection.X;
        var dy = primary.DepthProjection.Y;

        // already anchored: do not touch anything, so Saved states stay pristine
        if (Math.Abs(dx) < 0.001 && Math.Abs(dy) < 0.001) return;

        foreach (var monitor in layout.PhysicalMonitors)
        {
            var projection = monitor.DepthProjection;
            using (projection.DelayChangeNotifications())
            {
                projection.X -= dx;
                projection.Y -= dy;
            }
        }

        layout.UpdatePhysicalMonitors();
    }

    public static void SetSizesFromSystemConfiguration(this IMonitorsLayout layout, bool placeAll = true)
    {
        // TODO
    }

    /// <summary>
    /// try to place windows according to windows placement
    /// </summary>
    /// <param name="layout"></param>
    /// <param name="placeAll">reset already placed windows</param>
    public static void SetLocationsFromSystemConfiguration(this IMonitorsLayout layout, bool placeAll = true)
    {
        var primarySource = layout.PrimarySource;
        var primaryMonitor = layout.PrimaryMonitor;
        if (primarySource == null || primaryMonitor == null) return;

        lock (CompactLock)
        {
            // List all display not positioned
            var unplacedScreens = placeAll ? layout.PhysicalMonitors.ToList() : layout.PhysicalMonitors.Where(s => !s.Placed).ToList();

            // Nothing to place: leave the layout untouched — the final compact would
            // otherwise reshape a layout the user deliberately saved.
            if (unplacedScreens.Count == 0) return;

            // start with primary display
            Queue<PhysicalMonitor> todo = new();
            todo.Enqueue(primaryMonitor);

            while (todo.Count > 0)
            {
                foreach (var monitor in todo)
                {
                    unplacedScreens.Remove(monitor);
                }

                var placedScreen = todo.Dequeue();

                foreach (var screenToPlace in unplacedScreens.ToList())
                {
                    if (screenToPlace == placedScreen) continue;

                    // Only a real pixel-space edge adjacency PLACES a monitor. The
                    // alignment equalities below (same X/Y/Right/Bottom) are hints:
                    // applying them must not consume the monitor, or a monitor whose
                    // only adjacency is with a later-placed neighbour never gets its
                    // adjacency rule tested (e.g. P|S|H side by side: H aligned on P
                    // by Y equality used to be swallowed before S could claim it).
                    var adjacent = false;
                    // Which axis the contact is on. The other one is not a matter of
                    // luck: it is computed below, and must not be left to whichever
                    // alignment equality happens to hold.
                    var horizontalContact = false;
                    var verticalContact = false;

                    //     __
                    //  __| A
                    // B  |__
                    //  __|
                    if (screenToPlace.ActiveSource.Source.InPixel.Bounds.X == placedScreen.ActiveSource.Source.InPixel.Bounds.Right)
                    {
                        screenToPlace.DepthProjection.X = placedScreen.DepthProjection.OutsideBounds.Right + screenToPlace.DepthProjection.LeftBorder;
                        adjacent = true;
                        horizontalContact = true;
                    }
                    //B |___|_
                    //A  |    |
                    if (screenToPlace.ActiveSource.Source.InPixel.Bounds.Y == placedScreen.ActiveSource.Source.InPixel.Bounds.Bottom)
                    {
                        screenToPlace.DepthProjection.Y = placedScreen.DepthProjection.OutsideBounds.Bottom + screenToPlace.DepthProjection.TopBorder;
                        adjacent = true;
                        verticalContact = true;
                    }

                    //     __
                    //  __| B
                    // A  |__
                    //  __|
                    if (screenToPlace.ActiveSource.Source.InPixel.Bounds.Right == placedScreen.ActiveSource.Source.InPixel.Bounds.X)
                    {
                        screenToPlace.DepthProjection.X = placedScreen.DepthProjection.OutsideBounds.Left -
                            screenToPlace.DepthProjection.OutsideBounds.Width + screenToPlace.DepthProjection.LeftBorder;
                        adjacent = true;
                        horizontalContact = true;
                    }

                    //A |___|_
                    //B  |    |

                    if (screenToPlace.ActiveSource.Source.InPixel.Bounds.Bottom == placedScreen.ActiveSource.Source.InPixel.Y)
                    {
                        screenToPlace.DepthProjection.Y = placedScreen.DepthProjection.OutsideBounds.Top -
                            screenToPlace.DepthProjection.OutsideBounds.Height + screenToPlace.DepthProjection.TopBorder;
                        adjacent = true;
                        verticalContact = true;
                    }


                    //  __
                    // |
                    // |__
                    //  __
                    // |
                    // |__
                    if (screenToPlace.ActiveSource.Source.InPixel.Bounds.X == placedScreen.ActiveSource.Source.InPixel.Bounds.X)
                    {
                        screenToPlace.DepthProjection.X = placedScreen.DepthProjection.X;
                    }

                    //  ___   ___
                    // |   | |   |
                    if (screenToPlace.ActiveSource.Source.InPixel.Bounds.Y == placedScreen.ActiveSource.Source.InPixel.Bounds.Y)
                    {
                        screenToPlace.DepthProjection.Y = placedScreen.DepthProjection.Y;
                    }

                    // __
                    //   |
                    // __|
                    // __
                    //   |
                    // __|
                    if (screenToPlace.ActiveSource.Source.InPixel.Bounds.Right == placedScreen.ActiveSource.Source.InPixel.Bounds.Right)
                    {
                        screenToPlace.DepthProjection.X = placedScreen.DepthProjection.Bounds.Right - screenToPlace.DepthProjection.Bounds.Width;
                    }

                    //|___||___|
                    if (screenToPlace.ActiveSource.Source.InPixel.Bounds.Bottom == placedScreen.ActiveSource.Source.InPixel.Bounds.Bottom)
                    {
                        screenToPlace.DepthProjection.Y = placedScreen.DepthProjection.Bounds.Bottom -
                                               screenToPlace.DepthProjection.Bounds.Height;
                    }
                    // The offset along the contact edge. Last, so it overrides the
                    // alignment equalities above: those only fire on an exact match, and
                    // a monitor sitting 219px higher than its neighbour matches none of
                    // them — it used to keep whatever it had and be shoved by the
                    // compact, landing a whole screen height away.
                    if (horizontalContact) PlaceAlongEdge(placedScreen, screenToPlace, vertical: true);
                    if (verticalContact) PlaceAlongEdge(placedScreen, screenToPlace, vertical: false);

                    if (adjacent)
                    {
                        unplacedScreens.Remove(screenToPlace);
                        // No compacting here. Every monitor not yet reached is still
                        // stacked at the origin, so a compact run mid-walk resolves
                        // overlaps against screens that have no position yet — and shoves
                        // the ones just placed correctly out of the way. It is why
                        // running this twice used to give a better answer than running it
                        // once: the second pass started from a layout that was already
                        // spread out, so the same compacts found nothing to do.
                        todo.Enqueue(screenToPlace);
                    }
                }
            }

            // Monitors with no pixel adjacency to any placed monitor keep their
            // alignment hints (or 0,0); one final compact snaps them into contact.
            layout.ForceCompact();
        }

        layout.UpdatePhysicalMonitors();
    }

    /// <summary>
    /// Position <paramref name="toPlace"/> along the edge it shares with
    /// <paramref name="placed"/> — the axis the contact is not on.
    /// <para>
    /// Exactly the inverse of <see cref="PixelLocationSolver"/>'s rule, and deliberately
    /// so: there, the physical midpoint of the shared span projects to the same pixel on
    /// both monitors. Here the same invariant is solved the other way round, which is
    /// what makes "apply to system" and "place from system" round-trip instead of
    /// drifting a little further apart on each pass.
    /// </para>
    /// <para>
    /// The midpoint is taken in MILLIMETRE space, as the inverse takes it, and that
    /// makes it implicit: the shared span depends on the position being solved for. It
    /// is settled by iterating from a pixel-space estimate — the overlap only shifts
    /// when one monitor's span stops covering the other's end, so a couple of passes
    /// reach the fixed point and further ones change nothing. Taking the pixel midpoint
    /// instead is closed-form and tempting, and it round-trips perfectly right up until
    /// two monitors have very different pitches, which is the case this whole file
    /// exists for.
    /// </para>
    /// </summary>
    static void PlaceAlongEdge(PhysicalMonitor placed, PhysicalMonitor toPlace, bool vertical)
    {
        var placedPx = placed.ActiveSource.Source.InPixel.Bounds;
        var toPlacePx = toPlace.ActiveSource.Source.InPixel.Bounds;

        // Pixels map to the panel, not to the bezel around it.
        var placedMm = placed.DepthProjection.Bounds;
        var toPlaceMm = toPlace.DepthProjection.Bounds;

        var (placedPxLo, placedPxSize) = vertical
            ? (placedPx.Y, placedPx.Height)
            : (placedPx.X, placedPx.Width);
        var (toPlacePxLo, toPlacePxSize) = vertical
            ? (toPlacePx.Y, toPlacePx.Height)
            : (toPlacePx.X, toPlacePx.Width);
        var (placedMmLo, placedMmSize) = vertical
            ? (placedMm.Y, placedMm.Height)
            : (placedMm.X, placedMm.Width);
        var toPlaceMmSize = vertical ? toPlaceMm.Height : toPlaceMm.Width;

        // A monitor reporting no pixels has nothing to be proportional to.
        if (placedPxSize <= 0 || toPlacePxSize <= 0) return;

        var pitchPlaced = placedMmSize / placedPxSize;
        var pitchToPlace = toPlaceMmSize / toPlacePxSize;

        // Seed from the pixel-space midpoint: always defined, and already the answer
        // whenever the two spans cover each other.
        var sharedPxLo = Math.Max(placedPxLo, toPlacePxLo);
        var sharedPxHi = Math.Min(placedPxLo + placedPxSize, toPlacePxLo + toPlacePxSize);
        var midPx = (sharedPxLo + sharedPxHi) / 2;
        var mm = placedMmLo
                 + (midPx - placedPxLo) * pitchPlaced
                 - (midPx - toPlacePxLo) * pitchToPlace;

        // Then settle on the millimetre-space midpoint, which is what the inverse uses.
        // Four passes: the overlap can only change while the estimate crosses a span
        // end, and each pass fixes the estimate that follows from the current one.
        for (var pass = 0; pass < 4; pass++)
        {
            var mid = (Math.Max(placedMmLo, mm) + Math.Min(placedMmLo + placedMmSize, mm + toPlaceMmSize)) / 2;
            var next = mid - pitchToPlace * (placedPxLo - toPlacePxLo + (mid - placedMmLo) / pitchPlaced);
            if (Math.Abs(next - mm) < 1e-9) break;
            mm = next;
        }

        if (vertical) toPlace.DepthProjection.Y = mm;
        else toPlace.DepthProjection.X = mm;
    }
}
