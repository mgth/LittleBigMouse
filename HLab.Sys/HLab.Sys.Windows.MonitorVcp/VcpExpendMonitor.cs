using System.Runtime.CompilerServices;
using HLab.Sys.Windows.Monitors;

namespace HLab.Sys.Windows.MonitorVcp;

public static class VcpExpendMonitor
{
    static readonly LevelParser LevelParser = new();

    static readonly ConditionalWeakTable<MonitorDevice, VcpControl> AllVcp = new();
    public static VcpControl Vcp(this MonitorDevice monitor) 
        => AllVcp.GetValue(monitor, m => new VcpControl(monitor, LevelParser));

    public static void Stop() => LevelParser.Stop();

}