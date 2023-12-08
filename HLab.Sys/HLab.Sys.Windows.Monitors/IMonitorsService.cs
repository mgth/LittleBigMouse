using HLab.Sys.Windows.API;

namespace HLab.Sys.Windows.Monitors;

public interface ISystemMonitorsService
{
    public DisplayDevice Root {get; }

    DesktopWallpaperPosition WallpaperPosition { get; }
}