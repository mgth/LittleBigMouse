#nullable enable
using LittleBigMouse.Plugins;

namespace LittleBigMouse.Platform.Linux;

/// <summary>
/// Whether this is a Plasma session whose wallpaper can be driven — asked by running
/// the smallest possible script through plasmashell, which is also the answer to "is
/// plasmashell there at all".
/// <para>
/// Setting the wallpaper is the agent's (v6): it owns the settings file, cuts the span
/// slices and paints the desktop, so that a display change puts the background back
/// whether or not a window is open.
/// </para>
/// </summary>
public class PlasmaWallpaperService : IWallpaperService
{
    bool? _isSupported;

    public bool IsSupported => _isSupported ??=
        LinuxDesktopEnvironment.Current.IsKde
        && PlasmaWallpaper.EvaluateScript("print(\"ok\")") == "ok";
}
