using HLab.Sys.Windows.Monitors;
using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using LittleBigMouse.DisplayLayout.Monitors;

namespace LittleBigMouse.Plugin.Vcp.Avalonia
{
    internal static class PhysicalMonitorsExtensions
    {
        public static MonitorDevice FromPhysical(this IMonitorsSet service, PhysicalMonitor monitor) 
            => service.Monitors.FirstOrDefault(e => e.IdMonitor == monitor.ActiveSource.Source.IdMonitorDevice);
        public static MonitorDevice MonitorDevice(this PhysicalMonitor monitor, IMonitorsSet service) 
            => service.FromPhysical(monitor);
    }
}
