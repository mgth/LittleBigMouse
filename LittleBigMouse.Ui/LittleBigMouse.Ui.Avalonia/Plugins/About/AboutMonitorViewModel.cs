/*
  LittleBigMouse.Control.Core
  Copyright (c) 2021 Mathieu GRENET.  All right reserved.

  This file is part of LittleBigMouse.Control.Core.

    LittleBigMouse.Control.Core is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    LittleBigMouse.Control.Core is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with MouseControl.  If not, see <http://www.gnu.org/licenses/>.

	  mailto:mathieu@mgth.fr
	  http://www.mgth.fr
*/

using System;
using System.Reflection;
using HLab.Mvvm.ReactiveUI;
using LittleBigMouse.DisplayLayout.Monitors;

namespace LittleBigMouse.Ui.Avalonia.Plugins.About;
public class AboutMonitorViewModel : ViewModel<PhysicalMonitor>
{
    public string Version
    {
        get
        {
            var assembly = Assembly.GetEntryAssembly();
            var info = assembly?.GetCustomAttribute<AssemblyInformationalVersionAttribute>()?.InformationalVersion;
            if (string.IsNullOrEmpty(info)) return assembly?.GetName().Version?.ToString() ?? "";

            // SourceLink appends "+<full commit sha>" — show it as a short hash.
            var parts = info.Split('+', 2);
            return parts.Length == 1 ? info : $"{parts[0]} ({parts[1][..Math.Min(7, parts[1].Length)]})";
        }
    }
}