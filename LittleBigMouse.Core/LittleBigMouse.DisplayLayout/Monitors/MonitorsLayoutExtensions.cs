#nullable enable
using System.Collections.Generic;
using System.Linq;

namespace LittleBigMouse.DisplayLayout.Monitors;

public static class MonitorsLayoutExtensions
{

    static readonly object CompactLock = new();

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
            var unattachedScreens = placeAll ? layout.PhysicalMonitors.ToList() : layout.PhysicalMonitors.Where(s => !s.Placed).ToList();

            // start with primary display
            var todo = new Queue<PhysicalMonitor>();
            todo.Enqueue(layout.PrimaryMonitor);

            while (todo.Count > 0)
            {
                foreach (var monitor in todo)
                {
                    unattachedScreens.Remove(monitor);
                }

                var placedScreen = todo.Dequeue();

                foreach (var screenToPlace in unattachedScreens)
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
                        todo.Enqueue(screenToPlace);
                    }
                }
            }
        }
        layout.UpdatePhysicalMonitors();
    }
}