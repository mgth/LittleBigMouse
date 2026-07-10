#nullable enable
using System;
using System.Linq;
using HLab.Sys.Windows.Monitors;
using LittleBigMouse.DisplayLayout.Monitors;
using LittleBigMouse.Plugins;

namespace LittleBigMouse.Platform.Windows;

/// <summary>
/// Windows implementation of <see cref="IMonitorInfoService"/>: locates the monitor's
/// connection in the Win32 device tree and dumps its values — the code that used to
/// live in the Info view-model, now behind the seam.
/// </summary>
public class WindowsMonitorInfoService : IMonitorInfoService
{
    readonly ISystemMonitorsService _monitors;

    public WindowsMonitorInfoService(ISystemMonitorsService monitors)
    {
        _monitors = monitors;
    }

    public void DisplayValues(PhysicalMonitor monitor, Action<string, string, Action?, bool> addValue)
    {
        var device = _monitors.Root.AllChildren<MonitorDeviceConnection>()
            .FirstOrDefault(d => d.Id == monitor.DeviceId);

        device?.DisplayValues(addValue);
    }
}
