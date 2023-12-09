using System.Runtime.CompilerServices;
using HLab.Sys.Windows.Monitors;

namespace HLab.Sys.Windows.MonitorVcp;

public static class VcpExpendMonitor
{
    static readonly LevelParser LevelParser = new LevelParser();

    static readonly ConditionalWeakTable<MonitorDeviceConnection, VcpControl> AllVcp = new ConditionalWeakTable<MonitorDeviceConnection, VcpControl>();
    public static VcpControl Vcp(this MonitorDeviceConnection monitor) => AllVcp.GetValue(monitor, m => new VcpControl(monitor, LevelParser));

}