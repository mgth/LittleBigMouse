/*
  HLab.Windows.Monitors
  Copyright (c) 2021 Mathieu GRENET.  All right reserved.

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
using System.Runtime.Serialization;
using HLab.Notify.PropertyChanged;
using Newtonsoft.Json;
using NativeMethods = HLab.Sys.Windows.API.NativeMethods;

namespace HLab.Sys.Windows.Monitors
{
    using H = H<DisplayDevice>;

    [DataContract]
    public class DisplayDevice : NotifierBase
    {
        public DisplayDevice(IMonitorsService service)
        {
            MonitorsService = service;

            H.Initialize(this);
        }

        [JsonIgnore]
        public IMonitorsService MonitorsService { get; }

        public void Init(DisplayDevice parent, NativeMethods.DISPLAY_DEVICE dev, IList<DisplayDevice> oldDevices, IList<MonitorDevice> oldMonitors)
        {
            Parent = parent;

            DeviceId = dev.DeviceID;
            DeviceString = dev.DeviceString;

            DeviceKey = dev.DeviceKey;
            DeviceName = dev.DeviceName;
            State = dev.StateFlags;

            if(MonitorsService is not MonitorsService service) return;

            switch (DeviceId.Split('\\')[0])
            {
                case "ROOT":
                    break;
                case "MONITOR":

                    var monitor = service.GetOrAddMonitor(DeviceId, () =>
                    {
                        var m =  new MonitorDevice(DeviceId, MonitorsService)/*{MonitorNo = n++}*/;
                        return m;
                    });

                    monitor.DeviceKey = DeviceKey;
                    monitor.DeviceString = DeviceString;
                    if (AttachedToDesktop)
                    {
                        monitor.AttachedDisplay = parent;
                        monitor.AttachedDevice = this;
                    }
                    else
                    {
                        monitor.AttachedDisplay = null;
                        monitor.AttachedDevice = null;
                    }
                    monitor.AttachedToDesktop = AttachedToDesktop;

                    var idx = oldMonitors.IndexOf(monitor);
                    if (idx>=0) oldMonitors.RemoveAt(idx);
                    break;

                case "PCI":
                case "RdpIdd_IndirectDisplay":
                case string s when s.StartsWith("VID_DATRONICSOFT_PID_SPACEDESK_VIRTUAL_DISPLAY_"):

                    var adapter = service.GetOrAddAdapter(DeviceId, () => new PhysicalAdapter(DeviceId, MonitorsService)
                    {
                        DeviceString = DeviceString
                    });

                    CheckCurrentMode();

                    break;
                default:
                        break;
            }
            




            uint i = 0;
            var child = new NativeMethods.DISPLAY_DEVICE(true);

            while (NativeMethods.EnumDisplayDevices(DeviceName, i++, ref child, 0))
            {
                var c = child;
                var device = MonitorsService.Devices.AddOrUpdate(m => m.DeviceName == c.DeviceName, 
                    d =>oldDevices.Remove(d), 
                    () =>new DisplayDevice(service));

                device.Init(this, c, oldDevices, oldMonitors);
                child = new NativeMethods.DISPLAY_DEVICE(true);
            }
        }

        [DataMember]
        public bool AttachedToDesktop => _attachedToDesktop.Get();
        private readonly IProperty<bool> _attachedToDesktop = H.Property<bool>(c => c
            .Set(e => (e.State & NativeMethods.DisplayDeviceStateFlags.AttachedToDesktop) != 0)
            .On(e => e.State)
            .Update()
        );

        [JsonProperty]
        public ObservableCollectionSafe<DisplayMode> DisplayModes { get; } = new ObservableCollectionSafe<DisplayMode>();

        [DataMember]
        public DeviceCaps DeviceCaps => _deviceCaps.Get();
        private readonly IProperty<DeviceCaps> _deviceCaps = H.Property<DeviceCaps>(c => c
                 .Set(s => new DeviceCaps(s.DeviceName))
            );



       [DataMember]
        public DisplayMode CurrentMode
        {
            get => _currentMode.Get();
            set => _currentMode.Set(value);
        }
        private readonly IProperty<DisplayMode> _currentMode = H.Property<DisplayMode>();

        private void CheckCurrentMode()
        {
            var devMode = new NativeMethods.DEVMODE(true);

            if (NativeMethods.EnumDisplaySettingsEx(DeviceName, -1, ref devMode, 0))
            {
                CurrentMode = new DisplayMode(devMode);
            }
        }
 
        public void UpdateDevModes()
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

        [DataMember] 
        public string DeviceKey
        {
            get => _deviceKey.Get();
            protected set => _deviceKey.Set(value);
        }
        private readonly IProperty<string> _deviceKey = H.Property<string>();

        public override string ToString() => DeviceId;
    }
}

