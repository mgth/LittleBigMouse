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
using System.Windows;
using HLab.Core.Annotations;
using HLab.DependencyInjection.Annotations;
using HLab.Mvvm;
using HLab.Mvvm.Annotations;
using HLab.Windows.Monitors;
using LittleBigMouse.ScreenConfigs;

namespace LittleBigMouse.Control.Core
{
    public class MainBootloader : IPreBootloader
    {
        [Import]
        public MainBootloader(MainService mainService, IMvvmService mvvmService)
        {
            _mainService = mainService;
            _mvvmService = mvvmService;
        }

        private readonly MainService _mainService;
        private readonly IMvvmService _mvvmService;

        [Import] private Func<ScreenConfig,MultiScreensViewModel> _getViewModel;

        public void Load()
        {
            var viewModel = _mainService.MainViewModel;

            viewModel.Config = _mainService.Config;

            viewModel.Presenter = _getViewModel(_mainService.Config);

            var view = (Window)_mvvmService.MainContext.GetView<ViewModeDefault>(viewModel, typeof(IViewClassDefault));

            view.Show();
        }

    }

    [Export(typeof(MainService)),Singleton]
    public class MainService
    {
        public ScreenConfig Config {get;}
        public MainViewModel MainViewModel { get; }

        [Import]
        public MainService(MainViewModel mainViewModel, IMvvmService mvvmService, IMonitorsService monitorsService, ScreenConfig config)
        {
           MainViewModel = mainViewModel;
           Config = config;
        }

    }
}
