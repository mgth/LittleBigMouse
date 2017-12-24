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
using System;
using System.Collections.Generic;
using System.Collections.ObjectModel;
using System.ComponentModel;
using System.Linq;
using System.Runtime.InteropServices;
using HLab.Notify;
using HLab.Windows.API;
using Microsoft.Win32;

namespace HLab.Windows.Monitors
{
    public class DisplayDevice : NotifierObject
    {
        public DisplayDevice()
        {
            this.Subscribe();
        }

        public MonitorsService Service => this.Get(()=>MonitorsService.D);

        public void Init(DisplayDevice parent, NativeMethods.DISPLAY_DEVICE dev, IList<DisplayDevice> oldDevices)
        {
            Parent = parent;

            DeviceId = dev.DeviceID;
            DeviceString = dev.DeviceString;

            DeviceKey = dev.DeviceKey;
            DeviceName = dev.DeviceName;
            State = dev.StateFlags;

            switch (DeviceId.Split('\\')[0])
            {
                case "ROOT":
                    break;
                case "MONITOR":
                    var mon = Service.GetOrAddMonitor(DeviceId, () => new Monitor {DeviceId = DeviceId});
                    mon.DeviceKey = DeviceKey;
                    mon.DeviceString = DeviceString;
                    mon.AttachedToDesktop = AttachedToDesktop;
                    break;
                case "PCI":
                    Service.GetOrAddAdapter(DeviceId, () => new PhysicalAdapter
                    {
                        DeviceId = DeviceId,
                        DeviceString = DeviceString
                    });
                    break;
                default:
                        break;
            }
            



            uint i = 0;
            var child = new NativeMethods.DISPLAY_DEVICE(true);

            while (NativeMethods.EnumDisplayDevices(DeviceName, i++, ref child, 0))
            {
                var device = MonitorsService.D.Devices.FirstOrDefault(m => m.DeviceName == child.DeviceName);
                if (device != null)
                {
                    oldDevices.Remove(device);
                    device.Init(this, child, oldDevices);
                }
                else
                {
                    device = new DisplayDevice();
                    device.Init(this, child,oldDevices);
                    MonitorsService.D.Devices.Add(device);
                }
                child = new NativeMethods.DISPLAY_DEVICE(true);
            }
        }

        [TriggedOn(nameof(State))]
        public bool AttachedToDesktop =>
            this.Get(() => (State & NativeMethods.DisplayDeviceStateFlags.AttachedToDesktop) != 0);


        public ObservableCollection<DisplayMode> DisplayModes =>
            this.Get(() => new ObservableCollection<DisplayMode>());
        public DeviceCaps DeviceCaps =>
            this.Get(() => new DeviceCaps(DeviceName));


        public DisplayMode CurrentMode => this.Get(() =>
        {
            NativeMethods.DEVMODE devmode = new NativeMethods.DEVMODE(true);

            int i = 0;
            if (NativeMethods.EnumDisplaySettingsEx(DeviceName, -1, ref devmode, 0))
            {
                return new DisplayMode(devmode);
            }
            return null;
        });

        public void UpdateDevMode()
        {
            NativeMethods.DEVMODE devmode = new NativeMethods.DEVMODE(true);

            int i = 0;
            while (NativeMethods.EnumDisplaySettingsEx(DeviceName, i, ref devmode, 0))
            {
                DisplayModes.Add(new DisplayMode(devmode));
                i++;
            }
        }

        public string DeviceName
        {
            get => this.Get<string>();
            internal set {
                if (this.Set(value))
                {
                    //if (string.IsNullOrWhiteSpace(DeviceString))
                    //{
                    //    string[] s = DeviceName.Split('\\');
                    //    if (s.Length > 3) DeviceString = s[3];
                    //}
                }
            }
        }

        public DisplayDevice Parent
        {
            get => this.Get<DisplayDevice>();
            protected set => this.Set(value);
        }

        public string DeviceString
        {
            get => this.Get<string>();
            protected set => this.Set(value ?? "");
        }

        public NativeMethods.DisplayDeviceStateFlags State
        {
            get => this.Get<NativeMethods.DisplayDeviceStateFlags>();
            protected set => this.Set(value);
        }

        public string DeviceId
        {
            get => this.Get<string>();
            protected set => this.Set(value);
        }

        public string DeviceKey
        {
            get => this.Get<string>();
            protected set => this.Set(value);
        }
    }
}

