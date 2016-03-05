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
        public ObservableCollection<DisplayMonitor> Monitors { get; } = new ObservableCollection<DisplayMonitor>();
        public DisplayAdapter(NativeMethods.DISPLAY_DEVICE dev)
        {
            Init(dev);
            AllAdapters.Add(this);
        }

        public void Init(NativeMethods.DISPLAY_DEVICE dev)
        {
            DeviceId = dev.DeviceID;
            DeviceKey = dev.DeviceKey;
            DeviceName = dev.DeviceName;
            DeviceString = dev.DeviceString;
            State = dev.StateFlags;

            UpdateMonitors();
        }

        ~DisplayAdapter()
        {
            AllAdapters.Remove(this);
        }

        public void UpdateMonitors()
        {
            uint i = 0;
            NativeMethods.DISPLAY_DEVICE dev = new NativeMethods.DISPLAY_DEVICE(true);
            while (NativeMethods.EnumDisplayDevices(DeviceName, i++, ref dev, 0))
            {
                DisplayMonitor displayMonitor = Monitors.FirstOrDefault(d => d.DeviceId == dev.DeviceID);
                if (displayMonitor == null) displayMonitor = new DisplayMonitor(this, dev);
                else displayMonitor.Init(dev);
            }
        }
    }
}
