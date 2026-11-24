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
/// an order that follows how the shell paints those slots; see <see cref="WriteImages"/>.
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

        if (!WriteImages(desktop, screens, WallpaperSlotOrder.ActiveMonitorCount(), TimeSpan.FromMilliseconds(50)))
            return;

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
    /// Files every screen's image (an empty one shows the background colour) and returns whether
    /// at least one write succeeded.
    /// <para>
    /// The slot pass comes first, using the pairing of <see cref="WallpaperSlotOrder"/>: for slot
    /// <c>k</c>, file the image of the monitor the shell paints there, under the path that files
    /// slot <c>k</c>. A screen whose pair was attempted stays tied to that filing path — a failed
    /// write is retried there once, never re-routed through the screen's own device path, which
    /// files another monitor's slot and would overwrite the image that slot just received.
    /// </para>
    /// <para>
    /// The device-path pass then takes the screens no slot was found for, and only through a
    /// path that files no slot of the snapshot: a collision would change another monitor. When
    /// the snapshot has holes, the paths of the holes are unknown, so it writes nothing. When
    /// the shell cannot enumerate at all (observed as RPC_X_NULL_REF_POINTER, 0x800706F4, from a
    /// non-elevated caller), every screen goes through its own device path.
    /// </para>
    /// </summary>
    internal static bool WriteImages(IDesktopWallpaper desktop, IReadOnlyList<ScreenWallpaper> screens,
        int activeMonitors, TimeSpan retryDelay)
    {
        var plan = WallpaperSlotOrder.ReadPlan(desktop, activeMonitors, attempts: 3, out var enumerated, retryDelay);

        if (plan is null && enumerated)
        {
            // Enumerated, but the snapshot does not add up: a write could land anywhere.
            Console.Error.WriteLine(
                $"IDesktopWallpaper monitor snapshot is inconsistent; {screens.Count} screen(s) left unchanged.");
            return false;
        }

        if (plan is null)
            Console.Error.WriteLine(
                $"IDesktopWallpaper cannot enumerate monitors; {screens.Count} screen(s) fall back to " +
                "their device path.");

        var applied = false;
        var claimed = new HashSet<ScreenWallpaper>(ReferenceEqualityComparer.Instance);

        foreach (var (filed, painted) in plan?.Pairs ?? [])
        {
            // The screen that must appear on the monitor painted at this slot.
            var match = screens.FirstOrDefault(s =>
                !claimed.Contains(s) && WallpaperSlotOrder.Covers(painted.Rect, s.LogicalBounds));
            if (match is null) continue;

            claimed.Add(match);
            var image = match.ImagePath ?? "";
            if (desktop.SetWallpaper(filed.DevicePath, image) == HResult.Ok
                || desktop.SetWallpaper(filed.DevicePath, image) == HResult.Ok)
                applied = true;
            else
                Console.Error.WriteLine($"IDesktopWallpaper could not file the image of {painted.Rect} under its slot.");
        }

        if (plan is { Complete: false }) return applied;

        foreach (var screen in screens)
        {
            if (claimed.Contains(screen)) continue;

            // SetWallpaper wants a display device interface path; anything else (the monitor
            // hardware id, an empty id) cannot be written this way.
            if (DisplayDevicePath(screen.DeviceId) is not { } deviceId) continue;
            if (plan?.Files(deviceId) == true) continue;

            if (desktop.SetWallpaper(deviceId, screen.ImagePath ?? "") == HResult.Ok)
                applied = true;
        }

        return applied;
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
