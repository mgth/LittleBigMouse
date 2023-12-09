using HLab.Sys.Windows.Monitors;
using LittleBigMouse.DisplayLayout.Monitors;

namespace LittleBigMouse.Plugin.Vcp.Avalonia;

internal static class PhysicalMonitorsExtensions
{
    public static MonitorDeviceConnection FromPhysical(this ISystemMonitorsService service, PhysicalMonitor monitor) 
        => service.Root
        .AllChildren<MonitorDeviceConnection>()
        .FirstOrDefault(e => e.Monitor.PhysicalId == monitor.ActiveSource.Source.Id);

    public static MonitorDeviceConnection MonitorDevice(this PhysicalMonitor monitor, ISystemMonitorsService service) 
        => service.FromPhysical(monitor);
}