using System;
using Avalonia.Controls;

namespace HLab.Sys.Windows.Monitors;

public class MonitorDeviceDesign : MonitorDevice
{
    MonitorDeviceDesign()
    {
        if(!Design.IsDesignMode) throw new InvalidOperationException("Only for design mode");

        PnpCode = "DEL0000";
        PhysicalId = "1";
    }
}