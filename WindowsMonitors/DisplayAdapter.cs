using System.Linq;
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

            bool result = NativeMethods.EnumDisplayDevices(DeviceName, i++, ref mon, 0);

            if (result == false) // TODO
            {
                mon.DeviceID = DeviceName;
                mon.DeviceName = DeviceName + @"\Monitor0";
                mon.DeviceString = DeviceName.Split('\\').Last();
                mon.StateFlags = NativeMethods.DisplayDeviceStateFlags.MultiDriver & NativeMethods.DisplayDeviceStateFlags.AttachedToDesktop;
                result = true;
            }

            while (result)
            {
                if (TempMonitors.FirstOrDefault(d => d.DeviceId == mon.DeviceID) == null)
                {
                    DisplayMonitor displayMonitor = AllMonitors.FirstOrDefault(d => d.DeviceId == mon.DeviceID);
                    if (displayMonitor == null) displayMonitor = new DisplayMonitor(this, mon);
                    else
                        displayMonitor.Init(this, mon);

                    TempMonitors.Add(displayMonitor);                    
                }

                result = NativeMethods.EnumDisplayDevices(DeviceName, i++, ref mon, 0);
            }
        }
    }
}
