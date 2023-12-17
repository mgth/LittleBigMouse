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
using System.Linq;
using System.Text.Json.Serialization;
using DynamicData;

namespace HLab.Sys.Windows.Monitors;

public class DisplayDevice
{
    public override string ToString() => $"{GetType().Name} {DeviceName} {DeviceString}";

    readonly List<DisplayDevice> _children = [];

    internal void AddChild(DisplayDevice device)
    {
        _children.Add(device);
    }

    public IEnumerable<T> AllChildren<T>() where T : DisplayDevice
    {
        foreach (var child in Children)
        {
            if(child is T t) yield return t;
            foreach (var c in child.AllChildren<T>()) yield return c;
        }
    }

    public IEnumerable<MonitorDevice> AllMonitorDevices() 
        => AllChildren<MonitorDeviceConnection>()
            .Select(e => e.Monitor)
            .GroupBy(e => e.Id)
            .Select(e => e.First())
            .OrderBy(e => e.PhysicalId);

    [JsonIgnore]
    public DisplayDevice Parent { get; set; }

    public IEnumerable<DisplayDevice> Children  => _children; 

    /// <summary>
    /// Device name as returned by EnumDisplayDevices :
    /// "ROOT", "\\\\.\\DISPLAY1", "\\\\.\\DISPLAY1\monitor0" 
    /// </summary>
    public string DeviceName { get; init; }

    /// <summary>
    /// Device name in human readable format :
    /// "NVIDIA GeForce RTX 3080 Ti"
    /// </summary>
    public string DeviceString { get; init; }

    /// <summary>
    /// Device id as returned by EnumDisplayDevices :
    /// "PCI\\VEN_10DE&DEV_2206&SUBSYS_3A3C1458&REV_A1"
    /// </summary>
    public string Id { get; init; }

    /// <summary>
    /// Path to the device registry key :
    /// "\\Registry\\Machine\\System\\CurrentControlSet\\Control\\Video\\{AC0F00F9-3A6E-11ED-84B1-EBFE3BE9690A}\\0000"
    /// </summary>
    public string DeviceKey { get; init; }

    public SourceList<DisplayMode> DisplayModes { get; } = new ();

    /// <summary>
    /// Device mode as returned by EnumDisplaySettingsEx :
    /// 
    /// </summary>
    public DisplayMode CurrentMode { get; init; }

    public DeviceCaps Capabilities { get; init; }
    public DeviceState State { get; init; }

    public DisplayMode GetBestDisplayMode()
    {
        var best = DisplayModes.Items.FirstOrDefault();
        foreach (var mode in DisplayModes.Items)
        {
            if (mode.BitsPerPixel < best.BitsPerPixel) continue;
            if (mode.DisplayFrequency < best.DisplayFrequency) continue;
            if(mode.Pels.Width < best.Pels.Width) continue; 
            if(mode.Pels.Height < best.Pels.Height) continue; 
            best = mode;
        }
        return best;
    }

}