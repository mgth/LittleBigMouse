using System;
using System.Collections.Generic;
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
    /// <summary>Band thickness in DIPs. Thinner than the 100-DIP rulers: it carries no digits.</summary>
    const double BandThickness = 24.0;

    static readonly List<ScreenSectionsWindow> Windows = [];

    /// <summary>
    /// One screen, with what it takes to put the reference line where the layout
    /// says it falls: the monitor for millimetres, the screen for pixels.
    /// </summary>
    sealed record Screen(PhysicalMonitor Monitor, PixelRect Bounds, double Scaling, GuideLineWindow Line);

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

        foreach (var screen in Screens) screen.Line.Close();
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

            foreach (var (kind, window) in windows)
            {
                var (position, width, height) = kind switch
                {
                    BorderSideKind.Top =>
                        (new PixelPoint(bounds.X, bounds.Y), bounds.Width, thickness),
                    BorderSideKind.Bottom =>
                        (new PixelPoint(bounds.X, (int)(bounds.Bottom - thickness)), bounds.Width, thickness),
                    BorderSideKind.Left =>
                        (new PixelPoint(bounds.X, bounds.Y), thickness, (double)bounds.Height),
                    _ =>
                        (new PixelPoint((int)(bounds.Right - thickness), bounds.Y), thickness, (double)bounds.Height)
                };

                window.ShowAt(position, width, height, scaling);
                Windows.Add(window);
            }

            // Placed and measured with the bands rather than when a boundary
            // catches something: a window made mid-gesture arrives late, and one
            // moved mid-gesture arrives in the wrong place. Shown once so the
            // windowing system settles its size and position, then out of the way
            // until a guide needs it.
            var line = new GuideLineWindow();
            line.ShowAt(new PixelPoint(bounds.X, bounds.Y), bounds.Width, bounds.Height, scaling);
            line.Hide();

            Screens.Add(new Screen(monitor, bounds, scaling, line));
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
            if (guide is { } g && Place(screen, g)) continue;

            screen.Line.Hide();
        }
    }

    static bool Place(Screen screen, BorderGuide guide)
    {
        // The edge's own ends need no explaining: the band already ends there. Same
        // rule as on the layout map.
        if (guide.Kind == SnapKind.EdgeEnd) return false;

        var projection = screen.Monitor.DepthProjection;

        var originMm = guide.IsVertical ? projection.Y : projection.X;
        var lengthMm = guide.IsVertical ? projection.Height : projection.Width;

        if (lengthMm <= 0) return false;
        if (guide.LayoutMm < originMm || guide.LayoutMm > originMm + lengthMm) return false;

        // A fraction of the screen, and nothing more: the window covers it, so it
        // turns that into its own pixels without anyone here knowing its scaling.
        screen.Line.Mark((guide.LayoutMm - originMm) / lengthMm, guide.IsVertical, guide.Kind);

        return true;
    }
}
