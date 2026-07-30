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
        }
    }
}
