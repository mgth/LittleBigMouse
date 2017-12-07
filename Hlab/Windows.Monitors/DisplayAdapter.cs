/*
  HLab.Windows.Monitors
  Copyright (c) 2017 Mathieu GRENET.  All right reserved.

  This file is part of HLab.Windows.Monitors.

    HLab.Windows.Monitors is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    HLab.Windows.Monitors is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with MouseControl.  If not, see <http://www.gnu.org/licenses/>.

	  mailto:mathieu@mgth.fr
	  http://www.mgth.fr
*/
using System.Collections.Generic;
using System.Collections.ObjectModel;
using System.Diagnostics;
using System.Linq;
using Hlab.Notify;
using WinAPI;

namespace HLab.Windows.Monitors
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
