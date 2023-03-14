using System.Linq;
using HLab.Sys.Windows.Monitors;

namespace LittleBigMouse.DisplayLayout;

public static class MonitorDeviceExtension
{
    public static Monitor GetMonitor(this MonitorDevice device, Layout layout)
    {

        var monitor = layout.AllMonitors.Items.FirstOrDefault(m => m.Id == device.IdMonitor);
        if (monitor != null) return monitor;
        
        //monitor = new Monitor();
        //monitor.Id = $"{device.IdMonitor}_{device.AttachedDisplay.CurrentMode.DisplayOrientation}";
        //layout.AllMonitors.Add(monitor);
        return monitor;
    }
}