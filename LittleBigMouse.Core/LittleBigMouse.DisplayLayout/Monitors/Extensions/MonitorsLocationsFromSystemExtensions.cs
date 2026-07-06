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
        if (layout.PrimarySource == null) return;

        lock (CompactLock)
        {
            // List all display not positioned
            var unplacedScreens = placeAll ? layout.PhysicalMonitors.ToList() : layout.PhysicalMonitors.Where(s => !s.Placed).ToList();

            // start with primary display
            Queue<PhysicalMonitor> todo = new();
            todo.Enqueue(layout.PrimaryMonitor);

            while (todo.Count > 0)
            {
                foreach (var monitor in todo)
                {
                    unplacedScreens.Remove(monitor);
                }

                var placedScreen = todo.Dequeue();

                foreach (var screenToPlace in unplacedScreens)
                {
                    if (screenToPlace == placedScreen) continue;

                    var somethingDone = false;

                    //     __
                    //  __| A
                    // B  |__
                    //  __|
                    if (screenToPlace.ActiveSource.Source.InPixel.Bounds.X == placedScreen.ActiveSource.Source.InPixel.Bounds.Right)
                    {
                        screenToPlace.DepthProjection.X = placedScreen.DepthProjection.OutsideBounds.Right + screenToPlace.DepthProjection.LeftBorder;
                        somethingDone = true;
                    }
                    //B |___|_
                    //A  |    |
                    if (screenToPlace.ActiveSource.Source.InPixel.Bounds.Y == placedScreen.ActiveSource.Source.InPixel.Bounds.Bottom)
                    {
                        screenToPlace.DepthProjection.Y = placedScreen.DepthProjection.OutsideBounds.Bottom + screenToPlace.DepthProjection.TopBorder;
                        somethingDone = true;
                    }

                    //     __
                    //  __| B
                    // A  |__
                    //  __|
                    if (screenToPlace.ActiveSource.Source.InPixel.Bounds.Right == placedScreen.ActiveSource.Source.InPixel.Bounds.X)
                    {
                        screenToPlace.DepthProjection.X = placedScreen.DepthProjection.OutsideBounds.Left -
                            screenToPlace.DepthProjection.OutsideBounds.Width + screenToPlace.DepthProjection.LeftBorder;
                        somethingDone = true;
                    }

                    //A |___|_
                    //B  |    |

                    if (screenToPlace.ActiveSource.Source.InPixel.Bounds.Bottom == placedScreen.ActiveSource.Source.InPixel.Y)
                    {
                        screenToPlace.DepthProjection.Y = placedScreen.DepthProjection.OutsideBounds.Top -
                            screenToPlace.DepthProjection.OutsideBounds.Height + screenToPlace.DepthProjection.TopBorder;
                        somethingDone = true;
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
                        somethingDone = true;
                    }

                    //  ___   ___
                    // |   | |   |
                    if (screenToPlace.ActiveSource.Source.InPixel.Bounds.Y == placedScreen.ActiveSource.Source.InPixel.Bounds.Y)
                    {
                        screenToPlace.DepthProjection.Y = placedScreen.DepthProjection.Y;
                        somethingDone = true;
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
                        somethingDone = true;
                    }

                    //|___||___|
                    if (screenToPlace.ActiveSource.Source.InPixel.Bounds.Bottom == placedScreen.ActiveSource.Source.InPixel.Bounds.Bottom)
                    {
                        screenToPlace.DepthProjection.Y = placedScreen.DepthProjection.Bounds.Bottom -
                                               screenToPlace.DepthProjection.Bounds.Height;
                        somethingDone = true;
                    }
                    if (somethingDone)
                    {
                        layout.ForceCompact();
                        todo.Enqueue(screenToPlace);
                    }
                }
            }
        }

        layout.UpdatePhysicalMonitors();
    }
}