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
using DynamicData;
using HLab.Sys.Windows.API;
using Newtonsoft.Json;
using ReactiveUI;

namespace HLab.Sys.Windows.Monitors
{
    [DataContract]
    public class DisplayDevice : ReactiveObject
    {
        public DisplayDevice(IMonitorsService service, string deviceName)
        {
            MonitorsService = service;
            DeviceName = deviceName;

            this.WhenAnyValue(
                    e => e.State,
                    s => (s & User32.DisplayDeviceStateFlags.AttachedToDesktop) != 0
                )
                .ToProperty(this, e => e.AttachedToDesktop,out _attachedToDesktop);

        }

        [JsonIgnore]
        public IMonitorsService MonitorsService { get; }

        public void Init(DisplayDevice parent, User32.DISPLAY_DEVICE dev, IList<DisplayDevice> oldDevices, IList<MonitorDevice> oldMonitors)
        {
            Parent = parent;

            DeviceId = dev.DeviceID;
            DeviceString = dev.DeviceString;

            DeviceKey = dev.DeviceKey;
            DeviceName = dev.DeviceName;
            State = dev.StateFlags;

            DeviceCaps = new DeviceCaps(DeviceName);

            if(MonitorsService is not MonitorsService service) return;

            switch (DeviceId.Split('\\')[0])
            {
                case "ROOT":
                    break;
                case "MONITOR":

                    var monitor = service.GetOrAddMonitor(DeviceId, (s,id) =>
                    {
                        var m =  new MonitorDevice(id, s);
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

                    var adapter = service.GetOrAddAdapter(DeviceId, (s,id) => new PhysicalAdapter(id, s)
                    {
                        DeviceString = DeviceString
                    });

                    CheckCurrentMode();

                    break;
                default:
                        break;
            }

            uint i = 0;
            var child = new User32.DISPLAY_DEVICE();

            while (User32.EnumDisplayDevices(DeviceName, i++, ref child, 0))
            {
                var c = child;
                var device = service.GetOrAddDevice(c.DeviceName, 
                    (s,id) =>new DisplayDevice(s,id));

                oldDevices.Remove(device);
                device.Init(this, c, oldDevices, oldMonitors);
                child = new User32.DISPLAY_DEVICE();
            }
        }

        [DataMember]
        public bool AttachedToDesktop => _attachedToDesktop.Value;
        readonly ObservableAsPropertyHelper<bool> _attachedToDesktop;


        [JsonProperty]
        public SourceList<DisplayMode> DisplayModes { get; } = new ();

        [DataMember]
        public DeviceCaps DeviceCaps
        {
            get => _deviceCaps;
            set => this.RaiseAndSetIfChanged(ref _deviceCaps, value);
        }
        DeviceCaps _deviceCaps;


       [DataMember]
        public DisplayMode CurrentMode
        {
            get => _currentMode;
            set => this.RaiseAndSetIfChanged(ref _currentMode, value);
        }
        DisplayMode _currentMode;

        void CheckCurrentMode()
        {
            var devMode = new User32.DEVMODE(true);

            if (User32.EnumDisplaySettingsEx(DeviceName, -1, ref devMode, 0))
            {
                CurrentMode = new DisplayMode(devMode);
            }
        }
 
        public void UpdateDevModes()
        {
            var devMode = new User32.DEVMODE(true);

            int i = 0;
            while (User32.EnumDisplaySettingsEx(DeviceName, i, ref devMode, 0))
            {
                DisplayModes.Add(new DisplayMode(devMode));
                i++;
            }
        }

        [DataMember]
        public string DeviceName
        {
            get => _deviceName;
            internal set => this.RaiseAndSetIfChanged(ref _deviceName, value);
        }
        string _deviceName;

        public DisplayDevice Parent
        {
            get => _parent;
            protected set => this.RaiseAndSetIfChanged(ref _parent, value);
        }
        DisplayDevice _parent;

        [DataMember]
        public string DeviceString
        {
            get => _deviceString;
            protected set => this.RaiseAndSetIfChanged(ref _deviceString, value ?? "");
        }
        string _deviceString;

        [DataMember]
        public User32.DisplayDeviceStateFlags State
        {
            get => _state;
            protected set => this.RaiseAndSetIfChanged(ref _state, value);
        }
        User32.DisplayDeviceStateFlags _state;

        [DataMember]
        public string DeviceId
        {
            get => _deviceId;
            protected set => this.RaiseAndSetIfChanged(ref _deviceId, value);
        }
        string _deviceId;

        [DataMember] 
        public string DeviceKey
        {
            get => _deviceKey;
            protected set => this.RaiseAndSetIfChanged(ref _deviceKey, value);
        }
        string _deviceKey;

        public override string ToString() => DeviceId;
    }
}

