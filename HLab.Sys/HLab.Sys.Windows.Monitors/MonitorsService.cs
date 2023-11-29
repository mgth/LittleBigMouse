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
#pragma warning disable CA1416 // Valider la compatibilité de la plateforme

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
using OneOf;
using OneOf.Types;

namespace HLab.Sys.Windows.Monitors;

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
    public static IMonitorsSet MonitorsSetDesign => new MonitorsService();

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
}