using System.Collections.Generic;
using HLab.Sys.Windows.API;

namespace HLab.Sys.Windows.Monitors;

public interface IMonitorsSet
{
    public IEnumerable<PhysicalAdapter> Adapters { get; }

    public IEnumerable<DisplayDevice> Devices { get; }

    public IEnumerable<MonitorDevice> Monitors { get; }

    DesktopWallpaperPosition WallpaperPosition { get; }
}