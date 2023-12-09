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
using System.IO;
using System.Runtime.Serialization;
using Avalonia.Media;
using HLab.Sys.Windows.API;
using HLab.Sys.Windows.Monitors.Factory;

namespace HLab.Sys.Windows.Monitors;

[DataContract]
public class SystemMonitorsService : ISystemMonitorsService
{
    WeakReference<DisplayDevice>? _root; 
    public DisplayDevice? Root
    {
        get
        {
            if (_root != null && _root.TryGetTarget(out var root)) return root;

            root = MonitorDeviceHelper.GetDisplayDevices();
            _root = new WeakReference<DisplayDevice>(root);

            return root;
        }
    }

    public void UpdateDevices()
    {
        _root = null;
    }

    [DataMember] public Color Background { get; set; }

    [DataMember] public DesktopWallpaperPosition WallpaperPosition { get; set; }

    //IObservableCache<MonitorDevice, string> _attachedMonitors;
    //public IObservableCache<MonitorDevice,string> AttachedMonitors => _attachedMonitors;    
    public static ISystemMonitorsService MonitorsSetDesign => new SystemMonitorsService();

    public string AppDataPath(bool create)
    {
        var path = Path.Combine(Environment.GetFolderPath(
            Environment.SpecialFolder.LocalApplicationData), "LittleBigMouse");

        if (create) Directory.CreateDirectory(path);

        return path;
    }
}