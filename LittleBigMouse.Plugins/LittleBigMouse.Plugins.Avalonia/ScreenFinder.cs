#nullable enable
using System;
using Avalonia.Controls;
using Avalonia.Platform;

namespace LittleBigMouse.Plugins.Avalonia;

public static class ScreenFinder
{
    /// <summary>
    /// Find the screen displaying a monitor from its layout-space bounds.
    /// The layout space and the windowing-system space may differ: KWin maps
    /// every XWayland output with a single global factor (the largest output
    /// scale), so a window aimed at a monitor must target the matching Avalonia
    /// screen, not the layout coordinates. Both spaces share the same origin
    /// and a common scale factor, so the right screen has matching proportions
    /// at the scaled position. On Windows the factor is 1 and the match is exact.
    /// </summary>
    public static Screen? FromLayoutBounds(Screens screens, HLab.Geo.Rect layoutBounds)
    {
        Screen? best = null;
        var bestOffset = double.MaxValue;

        foreach (var screen in screens.All)
        {
            var bounds = screen.Bounds;
            var kx = bounds.Width / layoutBounds.Width;
            var ky = bounds.Height / layoutBounds.Height;

            if (Math.Abs(kx - ky) > 0.01 * kx) continue;

            var offset = Math.Abs(bounds.X - layoutBounds.X * kx)
                         + Math.Abs(bounds.Y - layoutBounds.Y * ky);

            // Allow a few layout-space pixels of rounding error.
            if (offset > 5.0 * kx) continue;
            if (offset >= bestOffset) continue;

            bestOffset = offset;
            best = screen;
        }

        return best;
    }
}
