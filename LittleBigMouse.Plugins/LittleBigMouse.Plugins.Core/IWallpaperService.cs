#nullable enable
using System.Collections.Generic;
using System.Threading.Tasks;
using HLab.ColorTools;
using HLab.Geo;
using LittleBigMouse.DisplayLayout.Monitors;

namespace LittleBigMouse.Plugins;

/// <summary>
/// One screen's target wallpaper. Screens are identified by their compositor
/// logical geometry (DisplaySource.InPixel.Bounds) — the same key PlasmaWallpaper
/// matches on when reading; a Windows implementation maps geometry to a monitor id
/// via IDesktopWallpaper.GetMonitorRECT.
/// </summary>
/// <param name="ImagePath">Image to display, or null for a solid color.</param>
public record ScreenWallpaper(
    Rect LogicalBounds,
    string? ImagePath,
    WallpaperStyle Style,
    ColorRGB<double>? Color);

/// <summary>
/// Platform seam: writes per-screen wallpapers to the desktop environment.
/// Linux/KDE drives the plasmashell scripting API; other environments report
/// unsupported and the wallpaper plugin stays out of the UI.
/// </summary>
public interface IWallpaperService
{
    bool IsSupported { get; }

    /// <summary>Apply all screens in one batch (one config rewrite).</summary>
    Task ApplyAsync(IReadOnlyList<ScreenWallpaper> screens);
}
