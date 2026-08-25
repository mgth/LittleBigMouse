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

    /// <summary>
    /// Try to place monitors according to the system's pixel configuration.
    /// <para>
    /// The geometry lives in <see cref="SystemLocationSolver"/>, a pure function over immutable
    /// snapshots; this method only bridges it to the reactive model — snapshot, solve, apply,
    /// then close whatever gaps the walk left. Same split as
    /// <see cref="MonitorsLayout.ForceCompact"/> and <see cref="CompactionSolver"/>, and for the
    /// same reasons: the placement rules can be tested without a reactive graph, and each
    /// monitor is written at most once instead of once per intermediate rule.
    /// </para>
    /// </summary>
    /// <param name="layout"></param>
    /// <param name="placeAll">reset already placed monitors</param>
    public static void SetLocationsFromSystemConfiguration(this IMonitorsLayout layout, bool placeAll = true)
    {
        var primarySource = layout.PrimarySource;
        var primaryMonitor = layout.PrimaryMonitor;
        if (primarySource == null || primaryMonitor == null) return;

        lock (CompactLock)
        {
            // A monitor with no active source has no pixel rect to place it from: it goes
            // through the walk as neither an anchor nor a candidate. Layouts are built one
            // monitor at a time and this runs on the result, so the half-built states are
            // reachable and must not take the whole placement down with them.
            var monitors = layout.PhysicalMonitors.Where(m => m.ActiveSource?.Source != null).ToList();

            // List all displays not positioned
            var toPlace = (placeAll ? monitors : monitors.Where(m => !m.Placed))
                .Select(m => m.Id)
                .ToHashSet();

            // Nothing to place: leave the layout untouched — the final compact would
            // otherwise reshape a layout the user deliberately saved.
            if (toPlace.Count == 0) return;

            var snapshot = monitors
                .Select(m => m.Snapshot(ReferenceEquals(m, primaryMonitor)))
                .ToList();

            var positions = SystemLocationSolver.Solve(snapshot, toPlace);

            foreach (var monitor in monitors)
            {
                if (!positions.TryGetValue(monitor.Id, out var position)) continue;

                var projection = monitor.DepthProjection;
                using (projection.DelayChangeNotifications())
                {
                    projection.X = position.X;
                    projection.Y = position.Y;
                }
            }

            // Monitors with no pixel adjacency to any placed monitor keep their
            // alignment hints (or 0,0); one final compact snaps them into contact.
            layout.ForceCompact();
        }

        layout.UpdatePhysicalMonitors();
    }
}
