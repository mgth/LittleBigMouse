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
/// Writes per-screen wallpapers through the shell's IDesktopWallpaper COM API — the same
/// object WindowsLayoutMapping reads from. Screens are paired with the shell's monitor slots
/// by rectangle (LogicalBounds and GetMonitorRECT are both in desktop pixel coordinates), in
/// an order that follows how the shell paints those slots; see <see cref="ApplyByMonitorSlots"/>.
/// Windows limitations mapped as-is: the fit position and the background color are
/// desktop-global, so the first image style and the first solid color win when screens disagree.
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

        // File each image under the slot the shell will paint it with; its filing order and
        // its painting order are two different lists (see ApplyByMonitorSlots).
        var applied = ApplyByMonitorSlots(desktop, screens, out var unresolved);

        foreach (var screen in unresolved)
        {
            // SetWallpaper wants a display device interface path; anything else (the monitor
            // hardware id, an empty id) cannot be written this way.
            if (DisplayDevicePath(screen.DeviceId) is not { } deviceId) continue;

            // An empty wallpaper shows the desktop background color.
            if (desktop.SetWallpaper(deviceId, screen.ImagePath ?? "") == HResult.Ok)
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

    /// <summary>
    /// Returns the id when it is a display device interface path, the form
    /// <c>IDesktopWallpaper</c> accepts; null otherwise. Returning it (rather than a bool)
    /// carries the null-check through to the call site.
    /// </summary>
    static string? DisplayDevicePath(string? deviceId)
        => deviceId is { Length: > 0 } id
           && id.StartsWith(@"\\?\DISPLAY#", StringComparison.OrdinalIgnoreCase)
            ? id
            : null;

    /// <summary>
    /// Writes the screens through the shell's per-monitor wallpaper slots, using the pairing of
    /// <see cref="WallpaperSlotOrder"/>: for slot <c>k</c>, file the image of the monitor the
    /// shell paints there, under the path that files slot <c>k</c>. Screens left without a slot
    /// (no rectangle, a cloned or a stale entry) are returned in
    /// <paramref name="unresolved"/> for the device-path pass.
    /// </summary>
    [SupportedOSPlatform("windows")]
    static bool ApplyByMonitorSlots(IDesktopWallpaper desktop, IReadOnlyList<ScreenWallpaper> screens,
        out List<ScreenWallpaper> unresolved)
    {
        unresolved = screens.ToList();

        if (!WallpaperSlotOrder.TryRead(desktop, out var slots))
        {
            // Observed as RPC_X_NULL_REF_POINTER (0x800706F4) from a non-elevated caller.
            Console.Error.WriteLine(
                $"IDesktopWallpaper cannot enumerate monitors; {screens.Count} screen(s) fall back to " +
                "their device path.");
            return false;
        }

        var applied = false;

        foreach (var (filed, painted) in WallpaperSlotOrder.Pair(slots))
        {
            // The screen that must appear on the monitor painted at this slot.
            var match = unresolved.FirstOrDefault(s => WallpaperSlotOrder.Covers(painted, s.LogicalBounds));
            if (match is null) continue;

            if (desktop.SetWallpaper(filed.DevicePath, match.ImagePath ?? "") != HResult.Ok) continue;
            unresolved.Remove(match);
            applied = true;
        }

        return applied;
    }

    static DesktopWallpaperPosition Position(WallpaperStyle style) => style switch
    {
        WallpaperStyle.Fit => DesktopWallpaperPosition.Fit,
        WallpaperStyle.Stretch => DesktopWallpaperPosition.Stretch,
        WallpaperStyle.Tile => DesktopWallpaperPosition.Tile,
        WallpaperStyle.Center => DesktopWallpaperPosition.Center,
        WallpaperStyle.Span => DesktopWallpaperPosition.Span,
        _ => DesktopWallpaperPosition.Fill,
    };
}
