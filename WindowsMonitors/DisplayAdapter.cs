using System.Collections;
using System.Collections.Generic;
using System.Collections.ObjectModel;
using System.Diagnostics;
using System.Linq;
using Erp.Notify;
using WinAPI;

namespace WindowsMonitors
{
    public class DisplayAdapter : DisplayDevice
    {
    
//        public ObservableCollection<DisplayMonitor> Monitors { get; } = new ObservableCollection<DisplayMonitor>();
        public DisplayAdapter()
        {
            this.Subscribe();
        }

        public void Init(NativeMethods.DISPLAY_DEVICE dev, IList<DisplayMonitor> oldMonitors )
        {
            DeviceId = dev.DeviceID;
            DeviceKey = dev.DeviceKey;
            DeviceName = dev.DeviceName;
            DeviceString = dev.DeviceString;
            State = dev.StateFlags;

            //if ((dev.StateFlags & NativeMethods.DisplayDeviceStateFlags.AttachedToDesktop) != NativeMethods.DisplayDeviceStateFlags.AttachedToDesktop) return;

            uint i = 0;
            var mon = new NativeMethods.DISPLAY_DEVICE(true);

            var w = new Stopwatch();

            w.Start();

            while (NativeMethods.EnumDisplayDevices(DeviceName, i++, ref mon, 0))
            {
                var monitor = MonitorsService.D.Monitors.FirstOrDefault(m => m.DeviceName == mon.DeviceName);
                if (monitor != null)
                {
                    oldMonitors.Remove(monitor);
                    monitor.Init(this,mon);
                }
                else
                {
                    monitor = new DisplayMonitor();
                    monitor.Init(this,mon);
                    MonitorsService.D.Monitors.Add(monitor);
                }

                monitor.Timing = w.ElapsedMilliseconds;
                w.Restart();
            }
        }

        public ObservableCollection<DisplayMode> DisplayModes =>
            this.Get(() => new ObservableCollection<DisplayMode>());
        public DeviceCaps DeviceCaps =>
            this.Get(() => new DeviceCaps(DeviceName));

        public DisplayMode CurrentMode => this.Get(() =>
        {
            NativeMethods.DEVMODE devmode = new NativeMethods.DEVMODE(true);

            int i = 0;
            if (NativeMethods.EnumDisplaySettingsEx(DeviceName, -1, ref devmode,0))
            {
                return new DisplayMode(devmode);
            }
            return null;
        });

        public void UpdateDevMode()
        {
            NativeMethods.DEVMODE devmode = new NativeMethods.DEVMODE(true);

            int i = 0;
            while (NativeMethods.EnumDisplaySettingsEx(DeviceName, i, ref devmode,0))
            {
                DisplayModes.Add(new DisplayMode(devmode));
                i++;
            }
        }


    }
}
