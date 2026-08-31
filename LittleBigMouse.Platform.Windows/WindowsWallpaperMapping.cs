#nullable enable
using System.Linq;
using HLab.ColorTools;
using HLab.Sys.Monitors;
using HLab.Sys.Windows.API;
using HLab.Sys.Windows.Monitors;
using HLab.Sys.Windows.Monitors.Factory;
using LittleBigMouse.DisplayLayout.Monitors;

namespace LittleBigMouse.Platform.Windows;

/// <summary>
/// Copies the desktop wallpaper (path, style, background color) from the Win32 device tree
/// into the reactive model. Split out from the geometry mapping because it is the one part
/// refreshed on its own — a live wallpaper change must not rebuild the layout.
/// <para>
/// The registry/file detail (the fade-window fallback path and the poll signature) lives in
/// <see cref="WindowsWallpaperReader"/>; this class only turns the COM adapter fields (plus that
/// fallback) into the reactive <see cref="DisplaySource"/> the UI repaints from.
/// </para>
/// </summary>
internal static class WindowsWallpaperMapping
{
    /// <summary>
    /// Re-read the desktop wallpaper (only) into the sources of an already-built layout, in
    /// place: geometry, DPI and monitor identity are left untouched, so any in-progress layout
    /// edits are preserved. The reactive <see cref="DisplaySource"/> properties drive the repaint.
    /// </summary>
    public static void UpdateWallpaper(this MonitorsLayout layout, ISystemMonitorsService service)
    {
        var root = service.Root;
        if (root is null) return;

        // Re-read the live wallpaper (COM IDesktopWallpaper) into the cached Win32 adapters.
        root.UpdateWallpaper();

        // IDesktopWallpaper.GetWallpaper returns "" while Windows plays the wallpaper fade, so an
        // image->image change would momentarily read empty. The registry holds the real current
        // image path (empty for a solid color) and is written immediately, so use it as a fallback
        // when the per-monitor COM path is still empty. Per-monitor wallpapers keep working via the
        // COM path when present.
        var registryPath = WindowsWallpaperReader.ReadImagePath();

        foreach (var monitor in root.AllMonitorDevices())
        {
            if (monitor.IsSpecialized) continue;
            layout.PhysicalSources
                .FirstOrDefault(s => s.DeviceId == monitor.Id)
                ?.Source.UpdateWallpaperFrom(monitor, registryPath);
        }
    }

    /// <summary>
    /// Copy only the wallpaper (path, style, background color) from the Win32 adapter into the
    /// reactive model. Shared by the full source mapping and the in-place wallpaper refresh
    /// (live wallpaper change, no layout rebuild).
    /// <para>
    /// When the freshly-read per-monitor COM path is empty (the transient returned during the
    /// wallpaper fade), the <paramref name="fallbackPath"/> is used instead, so the monitor is
    /// not blanked. Style and background color are always applied.
    /// </para>
    /// </summary>
    public static DisplaySource UpdateWallpaperFrom(this DisplaySource source, MonitorDevice monitor, string? fallbackPath = null)
    {
        if (monitor.ActiveConnection is not { } device) return source;
        if (device.Parent == null) return source;

        // Per-monitor COM path is empty during the wallpaper fade; fall back to the registry path
        // (the real current image, written immediately) so a live change reflects at once.
        var comPath = device.Parent.WallpaperPath;
        source.WallpaperPath = !string.IsNullOrEmpty(comPath) ? comPath : (fallbackPath ?? comPath);

        source.WallpaperStyle = device.Parent.WallpaperPosition switch
        {
            DesktopWallpaperPosition.Fill => WallpaperStyle.Fill,
            DesktopWallpaperPosition.Fit => WallpaperStyle.Fit,
            DesktopWallpaperPosition.Center => WallpaperStyle.Center,
            DesktopWallpaperPosition.Tile => WallpaperStyle.Tile,
            DesktopWallpaperPosition.Span => WallpaperStyle.Span,
            _ => WallpaperStyle.Stretch
        };

        var color = device.Parent.Background;
        source.BackgroundColor = HLabColors.RGB<double>((byte)(color & 0xFF), (byte)((color >> 8) & 0xFF), (byte)((color >> 16) & 0xFF));

        return source;
    }
}
