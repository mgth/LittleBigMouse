using HLab.Sys.Windows.Monitors;
using LittleBigMouse.DisplayLayout.Monitors;

namespace LittleBigMouse.Plugin.Vcp.Avalonia;

internal static class PhysicalMonitorsExtensions
{
    public static MonitorDevice FromPhysical(this ISystemMonitorsService service, PhysicalMonitor monitor) 
        => service.Root
        .AllChildren<MonitorDevice>()
        .FirstOrDefault(e => e.IdMonitor == monitor.ActiveSource.Source.IdMonitorDevice);

    public static MonitorDevice MonitorDevice(this PhysicalMonitor monitor, ISystemMonitorsService service) 
        => service.FromPhysical(monitor);
}