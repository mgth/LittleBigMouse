using HLab.Sys.Windows.API;
using System.Collections.Generic;

namespace HLab.Sys.Windows.Monitors;

public interface IMonitorsSet
{
    public IEnumerable<PhysicalAdapter> Adapters { get; }

    public IEnumerable<DisplayDevice> Devices { get; }

    public IEnumerable<MonitorDevice> Monitors { get; }

    DesktopWallpaperPosition WallpaperPosition { get; }
}