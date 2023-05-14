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

#nullable enable
using System;
using System.Collections.Concurrent;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Reactive.Linq;
using System.Runtime.Serialization;
using Avalonia.Media;
using DynamicData;
using HLab.Sys.Windows.API;
using Microsoft.Win32;
using ReactiveUI;

namespace HLab.Sys.Windows.Monitors
{
    [DataContract]
    public class MonitorsService : IMonitorsSet
    {
        public MonitorsService()
        {
        }

        public IEnumerable<PhysicalAdapter> Adapters => _adapters.Values;
        readonly ConcurrentDictionary<string,PhysicalAdapter> _adapters = new();


        public IEnumerable<DisplayDevice> Devices => _devices.Values;
        readonly ConcurrentDictionary<string,DisplayDevice> _devices = new();

        public IEnumerable<MonitorDevice> Monitors => _monitors.Values;
        readonly ConcurrentDictionary<string,MonitorDevice> _monitors = new();

        [DataMember] public Color Background { get; set; }

        [DataMember] public DesktopWallpaperPosition WallpaperPosition { get; set; }

        //IObservableCache<MonitorDevice, string> _attachedMonitors;
        //public IObservableCache<MonitorDevice,string> AttachedMonitors => _attachedMonitors;    
        public static IMonitorsSet Design => new MonitorsService();

        public DisplayDevice GetOrAddDevice(string deviceId, Func<string,DisplayDevice> get) 
            => _devices.GetOrAdd(deviceId, get);

        public DisplayDevice? RemoveDevice(string deviceId)
        {
            return _devices.TryRemove(deviceId, out var device) ? device : null;
        }

        public MonitorDevice GetOrAddMonitor(string deviceId, Func<string, MonitorDevice> get)
            => _monitors.GetOrAdd(deviceId, get);

        public MonitorDevice? RemoveMonitor(string deviceId)
        {
            return _monitors.TryRemove(deviceId, out var monitor) ? monitor : null;
        }

        public PhysicalAdapter GetOrAddAdapter(string deviceId, Func<string, PhysicalAdapter> get)
            => _adapters.GetOrAdd(deviceId, get);

        public PhysicalAdapter? RemoveAdapter(string deviceId)
        {
            return _adapters.TryRemove(deviceId, out var adapter) ? adapter : null;
        }

        public string AppDataPath(bool create)
        {
            var path = Path.Combine(Environment.GetFolderPath(
                Environment.SpecialFolder.LocalApplicationData), "LittleBigMouse");

            if (create) Directory.CreateDirectory(path);

            return path;
        }

        bool ParseWindowsConfig()
        {
            using var configurationKey = GetConfigurationKey();
            if(configurationKey?.GetValue("SetId") is not string setId) return false;

            setId= setId.Trim('\0');
            var monitorNo = 1;

            var monitors = Monitors.ToList();
            var idDisplays = setId.Split('+').Reverse();
            foreach (var idDisplay in idDisplays)
            {
                var idMonitors = idDisplay.Split('*').Reverse();
                DisplayDevice display = null;
                foreach(var idMonitor in idMonitors)
                {
                    var monitor = monitors.FirstOrDefault(m => m.IdMonitor == idMonitor);
                    if (monitor == null) return false;

                    if(display!=null)
                    {
                        if(!ReferenceEquals(display,monitor.AttachedDisplay)) return false;
                    }
                    else display = monitor.AttachedDisplay;
                    
                    monitor.MonitorNumber = monitorNo++;

                    monitors.Remove(monitor);
                }
            }
            return !monitors.Any();
        }

        RegistryKey GetConfigurationKey()
        {
            using var key = Registry.LocalMachine.OpenSubKey(@"SYSTEM\CurrentControlSet\Control\GraphicsDrivers\Configuration");
            foreach(var configurationKeyName in key.GetSubKeyNames())
            {
                var configurationKey = key.OpenSubKey(configurationKeyName);
                if (configurationKey?.GetValue("SetId") is string setId && MatchConfig(setId.Trim('\0')))
                {
                    return configurationKey;
                }
            }
            return null;
        }

        bool MatchConfig(string setId)
        {
            var devices = new List<DisplayDevice>();

            var monitors = Monitors.ToList();
            var idDisplays = setId.Split('+');
            foreach (var idDisplay in idDisplays)
            {
                var idMonitors = idDisplay.Split('*');
                DisplayDevice display = null;
                foreach(var idMonitor in idMonitors)
                {
                    var monitor = monitors.FirstOrDefault(m => m.IdMonitor == idMonitor);
                    if (monitor == null) return false;

                    if(display!=null)
                    {
                        if(!ReferenceEquals(display,monitor.AttachedDisplay)) return false;
                    }
                    else 
                    {
                        display = monitor.AttachedDisplay; 
                        if(devices.Contains(display)) return false;
                        devices.Add(display);
                    }
                    
                    monitors.Remove(monitor);
                }
            }
            return !monitors.Any();
        }


    }
}
