using HLab.Core.Annotations;
using HLab.Mvvm.Annotations;
using LittleBigMouse.Plugins;
using LittleBigMouse.Plugins.Avalonia;

namespace LittleBigMouse.Plugin.Wallpaper.Avalonia;

public class WallpaperViewMode : ViewMode { }

public class WallpaperPlugin(IMainService mainService, IWallpaperService wallpaperService, WallpaperManager manager) : Bootloader
{
    public override Task<BootState> LoadAsync()
    {
        // Injecting the manager instantiates the singleton at boot: it must be
        // listening before the first layout lands to re-slice span wallpapers.
        _ = manager;

        // No supported desktop environment (GNOME, bare X11…): stay out of the UI.
        if (wallpaperService.IsSupported)
            mainService.AddControlPlugin(c =>
                c.AddViewModeButton<WallpaperViewMode>(
                    "wallpaper",
                    "Icon/Wallpaper",
                    "Wallpaper"
                )
            );
        return Task.FromResult(BootState.Completed);
    }
}
