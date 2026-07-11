#nullable enable
using System.Runtime.CompilerServices;
using HLab.Sys.Windows.Monitors;

namespace HLab.Sys.Windows.MonitorVcp;

public static class VcpExpendMonitor
{
    static readonly CommandWorker LevelParser = new();

    /// <summary>The shared DDC worker — all VCP levels of all monitors are polled on this single queue.</summary>
    public static CommandWorker Worker => LevelParser;

    static readonly ConditionalWeakTable<MonitorDevice, VcpControl> AllVcp = new();

    /// <summary>Windows path: VCP channel from the Win32 device tree (dxva2 transport).</summary>
    public static VcpControl Vcp(this MonitorDevice monitor)
        => AllVcp.GetValue(monitor, m => new VcpControl(m.Id, m.Edid?.ManufacturerCode, new DxVa2VcpTransport(m), LevelParser));

    public static void Stop() => LevelParser.Stop();

}
