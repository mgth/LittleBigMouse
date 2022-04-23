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

using HLab.Core.Annotations;
using HLab.Mvvm.Annotations;
using HLab.Notify.PropertyChanged;
using LittleBigMouse.Plugins;

namespace LittleBigMouse.Plugin.Vcp;

class ViewModeScreenVcp : ViewMode { }

public class VcpPlugin : IBootloader
{
    private readonly IMainService _mainService;

    public VcpPlugin(IMainService mainService)
    {
        _mainService = mainService;
    }

    public void Load(IBootContext bootstrapper)
    {
        _mainService.AddControlPlugin( c =>

        c.AddButton(new NCommand<bool>(b =>
            {
                if (b)
                    c.SetViewMode<ViewModeScreenVcp>();
                else
                    c.SetViewMode<ViewModeDefault>();
            })
        {
            IconPath = "Icon/MonitorVcp",
            ToolTipText = "Vcp control"
        }
        ));
    }

}
