/*
  LittleBigMouse.Plugin.Vcp
  Copyright (c) 2017 Mathieu GRENET.  All right reserved.

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

using System.Xml.Serialization;
using HLab.Core.Annotations;
using HLab.DependencyInjection.Annotations;
using HLab.Mvvm;
using HLab.Mvvm.Annotations;
using HLab.Mvvm.Wpf.Icons;
using LittleBigMouse.Control.Core;

namespace LittleBigMouse.Plugin.Vcp
{
    class ViewModeScreenVcp : ViewMode { }

    class VcpPlugin : IPreBootloader
    {
        private readonly IIconService _iconService;
        private readonly MainService _mainService;

        [Import] public VcpPlugin(IIconService iconService, MainService mainService)
        {
            _iconService = iconService;
            _mainService = mainService;
        }

        public void Load()
        {
            _mainService.MainViewModel.AddButton(_iconService.GetIcon("Icons/IconVcp"),"Vcp control",
                () => _mainService.MainViewModel.Presenter.ViewMode = typeof(ViewModeScreenVcp),
                () => _mainService.MainViewModel.Presenter.ViewMode = typeof(ViewModeDefault));
        }
    }
}
