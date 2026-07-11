#nullable enable
using HLab.Sys.Windows.Monitors;
using HLab.Sys.Windows.MonitorVcp;
using LittleBigMouse.DisplayLayout.Monitors;

namespace LittleBigMouse.Plugin.Vcp.Avalonia;

/// <summary>Windows: match the layout monitor back to its Win32 device and use dxva2.</summary>
public class WindowsVcpService(ISystemMonitorsService monitorsService) : IVcpService
{
    public VcpControl? GetControl(PhysicalMonitor monitor)
        => monitor.MonitorDevice(monitorsService)?.Vcp();
}
