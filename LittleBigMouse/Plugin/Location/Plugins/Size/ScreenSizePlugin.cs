/*
  LittleBigMouse.Plugin.Location
  Copyright (c) 2017 Mathieu GRENET.  All right reserved.

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
using HLab.DependencyInjection.Annotations;
using HLab.Mvvm.Annotations;
using HLab.Mvvm.Wpf.Icons;
using LittleBigMouse.Control.Core;

namespace LittleBigMouse.Plugin.Location.Plugins.Size
{
    public class ViewModeScreenSize : ViewMode { }
    internal class ScreenSizePlugin : IPreBootloader
    {
        private readonly MainService _mainService;
        private readonly IIconService _iconService;

        [Import]
        public ScreenSizePlugin(MainService mainService, IIconService iconService)
        {
            _mainService = mainService;
            _iconService = iconService;
        }

        public void Load()
        {
            _mainService.MainViewModel.AddButton(_iconService.GetIcon("Icons/IconSize"),"Size",
                () => _mainService.MainViewModel.Presenter.ViewMode = typeof(ViewModeScreenSize),
                () => _mainService.MainViewModel.Presenter.ViewMode = typeof(ViewModeDefault));
        }
    }

}
