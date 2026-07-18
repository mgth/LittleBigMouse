/*
  LittleBigMouse.Plugin.Vcp
  Copyright (c) 2021 Mathieu GRENET.  All right reserved.

  This file is part of LittleBigMouse.Plugin.Vcp.

    LittleBigMouse.Plugin.Vcp is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    LittleBigMouse.Plugin.Vcp is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with MouseControl.  If not, see <http://www.gnu.org/licenses/>.

	  mailto:mathieu@mgth.fr
	  http://www.mgth.fr
*/

using Avalonia;
using HLab.Core.Annotations;
using HLab.Mvvm.Annotations;
using LittleBigMouse.DisplayLayout.Monitors;
using LittleBigMouse.Plugins;
using LittleBigMouse.Plugins.Avalonia;
using ReactiveUI;

namespace LittleBigMouse.Plugin.Vcp.Avalonia;

internal class MonitorVcpViewMode : ViewMode { }

public class VcpPlugin(IMainService mainService, ILayoutOptions options) : Bootloader
{
    public override Task<BootState> LoadAsync()
    {
        mainService.AddControlPlugin(c =>
            c.AddViewModeButton<MonitorVcpViewMode>(
                "vcp",
                "Icon/MonitorVcp",
                "Vcp control",
                // Off by default: the panel only shows for users who opted in.
                options.WhenAnyValue(o => o.VcpControl))
        );
        return Task.FromResult(BootState.Completed);
    }

}
