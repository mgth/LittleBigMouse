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
using System.Collections.ObjectModel;
using System.Windows;
using HLab.Mvvm;
using HLab.Mvvm.Annotations;
using HLab.Notify;
using HLab.Notify.PropertyChanged;
using LittleBigMouse.ScreenConfigs;

namespace LittleBigMouse.Control.Core
{
    public class MultiScreensViewModel : ViewModel<MultiScreensViewModel>, IPresenterViewModel, IMvvmContextProvider
    {
        public MultiScreensViewModel(ScreenConfig config)
        {
            Initialize();
            Config = config;
        }

        public MainViewModel MainViewModel
        {
            get => _mainViewModel.Get();
            set => _mainViewModel.Set(value);
        }
        private readonly IProperty<MainViewModel> _mainViewModel = H.Property<MainViewModel>();

        public Type ViewMode
        {
            get => _viewMode.Get();
            set => _viewMode.Set(value);
        }
        private readonly IProperty<Type> _viewMode 
            = H.Property<Type>(nameof(ViewMode), c=>c
                .Set(e => typeof(ViewModeDefault/*ViewModeScreenLocation*/)));

        // private readonly IProperty<Size> _size = H.Property<Size>(nameof(Size));
        // public Size Size { get => _size.Get(); set => _size.Set(value); }
        // public ObservableCollection<ScreenFrameViewModel> ScreenFrames = new ObservableCollection<ScreenFrameViewModel>();


        public ScreenConfig Config
        {
            get => _config.Get();
            set => _config.Set(value);
        }
        private readonly IProperty<ScreenConfig> _config = H.Property<ScreenConfig>(nameof(Config));

        public void ConfigureMvvmContext(IMvvmContext ctx)
        {
            ctx.AddCreator<ScreenFrameViewModel>(vm => vm.Presenter = this);
        }
    }
}
