#nullable enable
using System;
using System.Collections.Generic;
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

        RebindPaintedImages(layout, registryPath);
    }

    /// <summary>
    /// The Win32 tree binds a slot's image to the monitor its device path names, while the shell
    /// paints it on another monitor — see <see cref="WallpaperSlotOrder"/>. Rebind the preview
    /// paths to what is actually on screen; without it the editor preview shows the two screens
    /// swapped on exactly the layouts where the desktop itself is now correct.
    /// <para>
    /// Runs after every read of the wallpaper into the model: the in-place refresh above, and
    /// the layout construction (<see cref="WindowsLayoutFactory.Create"/>), which reads through
    /// the device tree too and would otherwise put the preview back in device-path order.
    /// </para>
    /// </summary>
    internal static void RebindPaintedImages(this MonitorsLayout layout, string registryPath)
    {
        if (!OperatingSystem.IsWindows()) return;

        IDesktopWallpaper desktop;
        try
        {
            desktop = (IDesktopWallpaper)new DesktopWallpaperClass();
        }
        catch (Exception)
        {
            return; // No COM: keep the plain read-back.
        }

        if (WallpaperSlotOrder.Read(desktop) is not { } reads) return;
        if (WallpaperSlotOrder.Plan(reads, WallpaperSlotOrder.ActiveMonitorCount()) is not { } plan) return;

        RebindPaintedImages(plan, layout.PhysicalSources.Select(s => s.Source), registryPath);
    }

    /// <summary>
    /// Gives each source the image of the slot painted on it, for the certain pairs of
    /// <paramref name="plan"/>. A source painted by a hole, or by a slot whose image could not be
    /// read, keeps the device-path read-back.
    /// <para>
    /// An empty image is a solid colour, or the fade transient during a change. The registry path
    /// stands in for the transient, as in <see cref="UpdateWallpaperFrom"/>, except when another
    /// slot already paints that very image: then the registry is only repeating it, and the empty
    /// slot really is a solid colour. Without that check a photo/solid-colour pair previews the
    /// photo twice.
    /// </para>
    /// </summary>
    internal static void RebindPaintedImages(WallpaperSlotOrder.SlotPlan plan, IEnumerable<DisplaySource> sources,
        string? registryPath)
    {
        var shown = plan.Slots
            .Where(s => s.ImagePath is { Length: > 0 })
            .Select(s => s.ImagePath!)
            .ToHashSet(StringComparer.OrdinalIgnoreCase);

        var fallback = registryPath is { Length: > 0 } path && !shown.Contains(path) ? path : "";

        var list = sources.ToList();
        foreach (var (filed, painted) in plan.Pairs)
        {
            if (filed.ImagePath is not { } image) continue;

            var source = list.FirstOrDefault(s => WallpaperSlotOrder.Covers(painted.Rect, s.InPixel.Bounds));
            if (source is null) continue;

            source.WallpaperPath = image.Length > 0 ? image : fallback;
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
