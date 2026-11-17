#nullable enable
using System.Collections.Generic;
using System.Threading.Tasks;
using LittleBigMouse.Plugins;

namespace LittleBigMouse.Platform.MacOS;

/// <summary>
/// Spanning a wallpaper across displays needs AppKit (NSWorkspace
/// setDesktopImageURL:forScreen:), which is not wired yet. Reporting
/// unsupported makes the wallpaper plugin hide itself, as it does on GNOME.
/// </summary>
public class MacWallpaperService : IWallpaperService
{
    public bool IsSupported => false;

    public Task ApplyAsync(IReadOnlyList<ScreenWallpaper> screens) => Task.CompletedTask;
}
