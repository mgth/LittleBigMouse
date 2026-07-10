#nullable enable
using System;
using System.Collections.Generic;
using System.Linq;
using System.Runtime.Versioning;
using System.Threading.Tasks;
using HLab.Sys.Windows.API;
using LittleBigMouse.DisplayLayout.Monitors;
using LittleBigMouse.Plugins;

namespace LittleBigMouse.Platform.Windows;

/// <summary>
/// Writes per-screen wallpapers through the shell's IDesktopWallpaper COM API —
/// the same object WindowsLayoutMapping reads from. Screens are matched to
/// monitor device paths by GetMonitorRECT against their LogicalBounds (both in
/// desktop pixel coordinates). Windows limitations mapped as-is: the fit
/// position and the background color are desktop-global, so the first image
/// style and the first solid color win when screens disagree.
/// </summary>
public class WindowsWallpaperService : IWallpaperService
{
    public bool IsSupported => OperatingSystem.IsWindows();

    public Task ApplyAsync(IReadOnlyList<ScreenWallpaper> screens)
        => Task.Run(() =>
        {
            if (!OperatingSystem.IsWindows() || screens.Count == 0) return;
            try
            {
                Apply(screens);
            }
            catch (Exception e)
            {
                Console.Error.WriteLine($"IDesktopWallpaper apply failed: {e.Message}");
            }
        });

    [SupportedOSPlatform("windows")]
    static void Apply(IReadOnlyList<ScreenWallpaper> screens)
    {
        var desktop = (IDesktopWallpaper)new DesktopWallpaperClass();

        uint count = 0;
        if (desktop.GetMonitorDevicePathCount(ref count) != HResult.Ok) return;

        // Device path per monitor rect; stale paths (unplugged history) have no rect.
        var monitors = new List<(string DevicePath, WinDef.Rect Rect)>();
        for (uint i = 0; i < count; i++)
        {
            var devicePath = "";
            WinDef.Rect rect = new();
            if (desktop.GetMonitorDevicePathAt(i, ref devicePath) != HResult.Ok) continue;
            if (desktop.GetMonitorRECT(devicePath, ref rect) != HResult.Ok) continue;
            monitors.Add((devicePath, rect));
        }

        var applied = false;
        foreach (var screen in screens)
        {
            var match = monitors.FirstOrDefault(m =>
                Math.Abs(m.Rect.X - screen.LogicalBounds.X) < 2 &&
                Math.Abs(m.Rect.Y - screen.LogicalBounds.Y) < 2);
            if (match.DevicePath is not { Length: > 0 } devicePath) continue;

            // An empty wallpaper shows the desktop background color.
            desktop.SetWallpaper(devicePath, screen.ImagePath ?? "");
            applied = true;
        }
        if (!applied) return;

        var style = screens.FirstOrDefault(s => s.ImagePath != null)?.Style;
        if (style != null)
            desktop.SetPosition(Position(style.Value));

        var color = screens.FirstOrDefault(s => s.ImagePath == null && s.Color != null)?.Color;
        if (color is { } rgb)
        {
            var bytes = rgb.To<byte>();
            // COLORREF is 0x00BBGGRR.
            desktop.SetBackgroundColor((uint)(bytes.Red | bytes.Green << 8 | bytes.Blue << 16));
        }
    }

    static DesktopWallpaperPosition Position(WallpaperStyle style) => style switch
    {
        WallpaperStyle.Fit => DesktopWallpaperPosition.Fit,
        WallpaperStyle.Stretch => DesktopWallpaperPosition.Stretch,
        WallpaperStyle.Tile => DesktopWallpaperPosition.Tile,
        WallpaperStyle.Center => DesktopWallpaperPosition.Center,
        _ => DesktopWallpaperPosition.Fill,
    };
}
