using System;
using Avalonia.Controls;

namespace HLab.Sys.Windows.Monitors;

public class MonitorDeviceDesign : MonitorDeviceConnection
{
    MonitorDeviceDesign()
    {
        if(!Design.IsDesignMode) throw new InvalidOperationException("Only for design mode");

        Monitor = new MonitorDevice{
            
            PnpCode = "DEL0000",
            PhysicalId = "1",
        };
    }
}