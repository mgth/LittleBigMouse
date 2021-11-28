/*
  LittleBigMouse.Control.Core
  Copyright (c) 2017 Mathieu GRENET.  All right reserved.

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
using System.Windows.Input;
using HLab.DependencyInjection.Annotations;
using HLab.Mvvm.Annotations;
using LittleBigMouse.Plugins;

namespace LittleBigMouse.Control.Main
{
    [Export(typeof(IMainService)),Singleton]
    public class MainService : IMainService
    {
        public ScreenConfig.ScreenConfig Config {get;}
        public MainViewModel MainViewModel { get; }

        [Import]
        public MainService(MainViewModel mainViewModel, ScreenConfig.ScreenConfig config)
        {
           MainViewModel = mainViewModel;
           Config = config;
        }

        public void AddButton(ICommand cmd) =>
            MainViewModel.AddButton(cmd);

        public void SetViewMode(Type viewMode)
        {
            MainViewModel.Presenter.ViewMode = viewMode;
        }

        public void SetViewMode<T>() where T:ViewMode => SetViewMode(typeof(T));
    }
}
