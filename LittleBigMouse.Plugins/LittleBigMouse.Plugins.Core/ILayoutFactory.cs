using System;
using LittleBigMouse.DisplayLayout.Monitors;

namespace LittleBigMouse.Plugins;

/// <summary>
/// Platform seam: builds the neutral <see cref="MonitorsLayout"/> model from the current
/// system. Each platform provides its own implementation (Windows reads the Win32 display
/// tree; a future Linux one reads RandR/DRM). The UI depends only on this interface and on
/// the model it returns — the model IS the abstraction, there is no intermediate data layer.
/// </summary>
public interface ILayoutFactory
{
    /// <summary>Enumerate the system and build a fresh layout (caller owns/disposes it).</summary>
    MonitorsLayout Create();

    /// <summary>
    /// A cheap fingerprint of the current display configuration (attached monitors + geometry +
    /// DPI + primary), read without the full <see cref="Create"/> enumeration. Two identical
    /// consecutive reads mean the OS has finished reconfiguring — used to react to a display
    /// change once it has settled, instead of waiting a fixed delay. Empty string if unavailable.
    /// </summary>
    string DisplaySignature();

    /// <summary>
    /// Raised when the OS desktop wallpaper changes. May fire on a background thread — the handler
    /// must marshal to the UI thread before touching the model (e.g. via <see cref="UpdateWallpaper"/>).
    /// </summary>
    event EventHandler? WallpaperChanged;

    /// <summary>
    /// Re-read the current desktop wallpaper into an already-built layout, in place (no
    /// teardown): only the wallpaper path/style/background of the live sources are refreshed,
    /// preserving any in-progress layout edits. Called when <see cref="WallpaperChanged"/> fires.
    /// </summary>
    void UpdateWallpaper(MonitorsLayout layout);
}
