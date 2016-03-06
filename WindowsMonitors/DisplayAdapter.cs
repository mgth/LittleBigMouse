using System;
using System.Collections.Generic;
using System.Collections.ObjectModel;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using WinAPI;

namespace WindowsMonitors
{
    public class DisplayAdapter : DisplayDevice
    {
//        public ObservableCollection<DisplayMonitor> Monitors { get; } = new ObservableCollection<DisplayMonitor>();
        public DisplayAdapter(NativeMethods.DISPLAY_DEVICE dev)
        {
            DeviceId = dev.DeviceID;
            DeviceKey = dev.DeviceKey;
            DeviceName = dev.DeviceName;
            DeviceString = dev.DeviceString;
            State = dev.StateFlags;

            uint i = 0;
            NativeMethods.DISPLAY_DEVICE mon = new NativeMethods.DISPLAY_DEVICE(true);
            while (NativeMethods.EnumDisplayDevices(DeviceName, i++, ref mon, 0))
            {
                DisplayMonitor displayMonitor = AllMonitors.FirstOrDefault(d => d.DeviceId == mon.DeviceID);
                if (displayMonitor == null) displayMonitor = new DisplayMonitor(this, mon);
                else
                    displayMonitor.Init(this, mon);

                if(displayMonitor.AttachedToDesktop)
                    TempMonitors.Add(displayMonitor);
            }
        }
    }
}
