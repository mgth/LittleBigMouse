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
using System.Linq;
using System.Runtime.Serialization;
using HLab.Base;
using HLab.DependencyInjection.Annotations;
using HLab.Mvvm;
using HLab.Notify.Annotations;
using HLab.Notify.PropertyChanged;
using HLab.Windows.API;
using Newtonsoft.Json;

namespace HLab.Windows.Monitors
{
    [DataContract]
    public class DisplayDevice : N<DisplayDevice>
    {
        public DisplayDevice(IMonitorsService service)
        {
            MonitorsService = service;

            Initialize();
        }

        [JsonIgnore]
        public IMonitorsService MonitorsService { get; }

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
                    var mon = ((MonitorsService)MonitorsService).GetOrAddMonitor(DeviceId, () => new Monitor(DeviceId, MonitorsService));
                    mon.DeviceKey = DeviceKey;
                    mon.DeviceString = DeviceString;
                    mon.AttachedToDesktop = AttachedToDesktop;
                    break;
                case "PCI":
                    ((MonitorsService)MonitorsService).GetOrAddAdapter(DeviceId, () => new PhysicalAdapter(DeviceId, MonitorsService)
                    {
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
                var device = MonitorsService.Devices.FirstOrDefault(m => m.DeviceName == child.DeviceName);
                if (device != null)
                {
                    oldDevices.Remove(device);
                    device.Init(this, child, oldDevices);
                }
                else
                {
                    device = new DisplayDevice(MonitorsService);
                    device.Init(this, child,oldDevices);
                    MonitorsService.Devices.Add(device);
                }
                child = new NativeMethods.DISPLAY_DEVICE(true);
            }
        }

        [DataMember]
        [TriggerOn(nameof(State))]
        public bool AttachedToDesktop 
            => (State & NativeMethods.DisplayDeviceStateFlags.AttachedToDesktop) != 0;


        [Import,JsonProperty]
        public ObservableCollectionSafe<DisplayMode> DisplayModes { get; } = new ObservableCollectionSafe<DisplayMode>();

        [DataMember]
        public DeviceCaps DeviceCaps => _deviceCaps.Get();
        private readonly IProperty<DeviceCaps> _deviceCaps = H.Property<DeviceCaps>(c => c
                 .Set(s => new DeviceCaps(s.DeviceName))
            );



       [DataMember]
        public DisplayMode CurrentMode => _currentMode.Get();
        private readonly IProperty<DisplayMode> _currentMode = H.Property<DisplayMode>(c => c
             .Set( e =>
                    {
                        NativeMethods.DEVMODE devmode = new NativeMethods.DEVMODE(true);

                        int i = 0;
                        if (NativeMethods.EnumDisplaySettingsEx(e.DeviceName, -1, ref devmode, 0))
                        {
                            return new DisplayMode(devmode);
                        }
                        return null;
                    }
                ));
 
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

        [DataMember]
        public string DeviceName
        {
            get => _deviceName.Get();
            internal set => _deviceName.Set(value);
        }
        private readonly IProperty<string> _deviceName = H.Property<string>();
                    //if (string.IsNullOrWhiteSpace(DeviceString))
                    //{
                    //    string[] s = DeviceName.Split('\\');
                    //    if (s.Length > 3) DeviceString = s[3];
                    //}

        public DisplayDevice Parent
        {
            get => _parent.Get();
            protected set => _parent.Set(value);
        }
        private readonly IProperty<DisplayDevice> _parent = H.Property<DisplayDevice>();

        [DataMember]
        public string DeviceString
        {
            get => _deviceString.Get();
            protected set => _deviceString.Set(value ?? "");
        }
        private readonly IProperty<string> _deviceString= H.Property<string>();

        [DataMember]
        public NativeMethods.DisplayDeviceStateFlags State
        {
            get => _state.Get();
            protected set => _state.Set(value);
        }
        private readonly IProperty<NativeMethods.DisplayDeviceStateFlags> _state 
            = H.Property<NativeMethods.DisplayDeviceStateFlags>();

        [DataMember]
        public string DeviceId
        {
            get => _deviceId.Get();
            protected set => _deviceId.Set(value);
        }
        private readonly IProperty<string> _deviceId = H.Property<string>();

        [DataMember] public string DeviceKey
        {
            get => _deviceKey.Get();
            protected set => _deviceKey.Set(value);
        }
        private readonly IProperty<string> _deviceKey = H.Property<string>();

        public override string ToString() => DeviceString;
    }
}

