using System;
using System.Collections.Generic;
using System.Linq;
using Avalonia;
using Avalonia.Platform;
using LittleBigMouse.DisplayLayout.Monitors;
using LittleBigMouse.Plugins.Avalonia;

namespace LittleBigMouse.Plugin.Layout.Avalonia.BorderResistancePlugin;

/// <summary>
/// The bands mirroring the sections onto the real monitors.
/// <para>
/// State lives here rather than on a monitor's view model because there is only
/// ever one overlay: it covers every screen at once, so the toggle shown on each
/// monitor of the layout map has to be the same switch, not four independent ones
/// disagreeing about whether the bands are up.
/// </para>
/// </summary>
public static class ScreenSectionsOverlay
{
    /// <summary>
    /// Band thickness in DIPs. Thinner than the 100-DIP rulers: it carries no digits.
    /// <para>
    /// Not thinner than 40, though. A window is not granted less than the platform's
    /// minimum — the decorations' own size, still applied to a window that has none
    /// (AvaloniaUI/Avalonia#20251) — which on Windows lands at 56 px across a
    /// horizontal band and 44 down a vertical one at 150%, and 103 at 300%. Asking
    /// for 24 got each band whatever its own axis allowed, so the horizontal ones
    /// came out visibly fatter than the vertical ones, and differently again from one
    /// screen to the next. Above the floor the request is honoured exactly, and the
    /// four bands of every screen match: 60 px at 150%, 120 at 300%, 40 at 100%.
    /// The rulers have never met this because 100 DIP clears it everywhere.
    /// </para>
    /// </summary>
    const double BandThickness = 40.0;

    static readonly List<ScreenSectionsWindow> Windows = [];

    /// <summary>
    /// One screen, with what it takes to put the reference line where the layout
    /// says it falls: the monitor for millimetres, the screen for pixels.
    /// </summary>
    /// <param name="Across">Holds a horizontal line, for a guide on a vertical edge.</param>
    /// <param name="Down">Holds a vertical one.</param>
    sealed record Screen(
        PhysicalMonitor Monitor, PixelRect Bounds, double Scaling,
        GuideLineWindow Across, GuideLineWindow Down);

    static readonly List<Screen> Screens = [];

    static ScreenSectionsOverlay()
    {
        // Subscribed for good: with the overlay down there are no screens to draw
        // on, and the notification costs an empty loop.
        BorderSectionGuide.Changed += OnGuideChanged;
    }

    public static event Action? Changed;

    public static bool Visible
    {
        get;
        private set;
    }

    public static void Toggle(IMonitorsLayout? layout, bool visible)
    {
        Visible = visible;
        Rebuild(layout);
        Changed?.Invoke();
    }

    static void Rebuild(IMonitorsLayout? layout)
    {
        foreach (var window in Windows) window.Close();
        Windows.Clear();

        foreach (var screen in Screens)
        {
            screen.Across.Close();
            screen.Down.Close();
        }

        Screens.Clear();

        if (!Visible || layout == null) return;

        foreach (var source in layout.PhysicalSources)
        {
            if (!source.Source.AttachedToDesktop) continue;

            // Every edge, like the rulers: an edge with nothing on it still tells you
            // that there is nothing on it, and a band that appears and disappears
            // depending on its contents is harder to read than a constant frame.
            var monitor = source.Monitor;
            var windows = new List<(BorderSideKind Kind, ScreenSectionsWindow Window)>();

            foreach (var kind in (BorderSideKind[])
                     [BorderSideKind.Top, BorderSideKind.Bottom, BorderSideKind.Left, BorderSideKind.Right])
            {
                var side = new BorderSideViewModel(monitor, kind, BorderSideViewModel.SideOf(monitor, kind));
                windows.Add((kind, new ScreenSectionsWindow(side)));
            }

            // Layout space and windowing-system space can differ — KWin maps every
            // XWayland output with one global factor — so the geometry comes from
            // the matching Avalonia screen, as the rulers do. Screens is readable
            // before the window is shown.
            var layoutBounds = source.Source.InPixel.Bounds;
            var screen = ScreenFinder.FromLayoutBounds(windows[0].Window.Screens, layoutBounds);

            var bounds = screen?.Bounds ?? new PixelRect(
                (int)layoutBounds.X, (int)layoutBounds.Y,
                (int)layoutBounds.Width, (int)layoutBounds.Height);
            var scaling = screen?.Scaling ?? source.Source.EffectiveDpi.Y / 96.0;
            var thickness = BandThickness * scaling;

            var band = (int)System.Math.Round(thickness);

            foreach (var (kind, window) in windows)
            {
                // Each band is anchored to the edge it measures: the platform may
                // grant it more thickness than it asks for, and it is the outer edge
                // that has to stay put.
                var (rect, anchorRight, anchorBottom) = kind switch
                {
                    BorderSideKind.Top =>
                        (new PixelRect(bounds.X, bounds.Y, bounds.Width, band), false, false),
                    BorderSideKind.Bottom =>
                        (new PixelRect(bounds.X, bounds.Bottom - band, bounds.Width, band), false, true),
                    BorderSideKind.Left =>
                        (new PixelRect(bounds.X, bounds.Y, band, bounds.Height), false, false),
                    _ =>
                        (new PixelRect(bounds.Right - band, bounds.Y, band, bounds.Height), true, false)
                };

                window.ShowAt(rect, scaling, anchorRight, anchorBottom);
                Windows.Add(window);
            }

            // Made and measured with the bands rather than when a boundary catches
            // something: a window made mid-gesture arrives late. One per
            // orientation, each at the size it will keep for good, so that a guide
            // only ever has to move one of them.
            var line = (int)System.Math.Round(GuideLineWindow.ThicknessDip * scaling);

            var across = new GuideLineWindow(isVertical: true);
            across.ShowAt(new PixelRect(bounds.X, bounds.Y, bounds.Width, line), scaling);

            var down = new GuideLineWindow(isVertical: false);
            down.ShowAt(new PixelRect(bounds.X, bounds.Y, line, bounds.Height), scaling);

            Screens.Add(new Screen(monitor, bounds, scaling, across, down));
        }

        // Out past the rightmost screen, far enough that no part of a parked window
        // lands on anything.
        var park = new PixelPoint(Screens.Max(s => s.Bounds.Right) + 100, Screens.Min(s => s.Bounds.Y));

        foreach (var screen in Screens)
        {
            screen.Across.ParkingSpot = park;
            screen.Down.ParkingSpot = park;
        }
    }

    /// <summary>
    /// Put the reference line on every screen it crosses. The bands draw the guide
    /// on the edge it belongs to; this is the line that says what it lined up WITH,
    /// which is the part that lives away from that edge.
    /// </summary>
    static void OnGuideChanged(BorderGuide? guide)
    {
        foreach (var screen in Screens)
        {
            var shown = guide is { } g ? Place(screen, g) : null;

            if (shown != screen.Across) screen.Across.Clear();
            if (shown != screen.Down) screen.Down.Clear();
        }
    }

    /// <returns>The band now showing the line on this screen, if any.</returns>
    static GuideLineWindow? Place(Screen screen, BorderGuide guide)
    {
        // The edge's own ends need no explaining: the band already ends there. Same
        // rule as on the layout map.
        if (guide.Kind == SnapKind.EdgeEnd) return null;

        var projection = screen.Monitor.DepthProjection;

        var originMm = guide.IsVertical ? projection.Y : projection.X;
        var lengthMm = guide.IsVertical ? projection.Height : projection.Width;

        if (lengthMm <= 0) return null;
        if (guide.LayoutMm < originMm || guide.LayoutMm > originMm + lengthMm) return null;

        // Millimetres to this screen's pixels, in the proportion the bands use.
        var ratio = (guide.LayoutMm - originMm) / lengthMm;
        var bounds = screen.Bounds;
        var thickness = (int)System.Math.Round(GuideLineWindow.ThicknessDip * screen.Scaling);

        var line = guide.IsVertical ? screen.Across : screen.Down;

        // The band is laid across the line, which runs down its middle.
        var rect = guide.IsVertical
            ? new PixelRect(
                bounds.X, (int)System.Math.Round(bounds.Y + ratio * bounds.Height) - thickness / 2,
                bounds.Width, thickness)
            : new PixelRect(
                (int)System.Math.Round(bounds.X + ratio * bounds.Width) - thickness / 2, bounds.Y,
                thickness, bounds.Height);

        line.Mark(rect, guide.Kind);

        return line;
    }
}
