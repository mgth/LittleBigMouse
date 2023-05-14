/*
  LittleBigMouse.Plugin.Location
  Copyright (c) 2021 Mathieu GRENET.  All right reserved.

  This file is part of LittleBigMouse.Plugin.Location.

    LittleBigMouse.Plugin.Location is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    LittleBigMouse.Plugin.Location is distributed in the hope that it will be useful,
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
using LittleBigMouse.Plugins;
using ReactiveUI;
using System.Reactive.Linq;

namespace LittleBigMouse.Plugin.Layout.Avalonia.SizePlugin;

public class ViewModeScreenSize : ViewMode { }
public class ScreenSizePlugin : IBootloader
{
    readonly IMainService _mainService;

    public ScreenSizePlugin(IMainService mainService)
    {
        _mainService = mainService;
    }

    public void Load(IBootContext bootstrapper)
    {
        _mainService.AddControlPlugin(c =>
            c.AddButton(
                "size",
                "Icon/MonitorSize",
                "Size",

                ReactiveCommand.Create<bool>(b => {
                        if (b)
                            c.SetMonitorFrameViewMode<ViewModeScreenSize>();
                        else
                            c.SetMonitorFrameViewMode<DefaultViewMode>();
                    }
                    , outputScheduler: RxApp.MainThreadScheduler
                    , canExecute: Observable.Return(true) ))
        );

    }
}
