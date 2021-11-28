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
using HLab.DependencyInjection.Annotations;
using HLab.Mvvm;
using HLab.Mvvm.Annotations;
using HLab.Notify.PropertyChanged;
using LittleBigMouse.Control.Main;
using LittleBigMouse.Control.ScreenFrame;
using LittleBigMouse.Plugins;
using LittleBigMouse.ScreenConfig.Dimensions;

namespace LittleBigMouse.Control
{
    using H = H<MultiScreensViewModel>;

    public class MultiScreensViewModel : ViewModel, IPresenterViewModel, IMvvmContextProvider, IMultiScreensViewModel
    {
        [Import]
        public MainViewModel MainViewModel { get; }

        public MultiScreensViewModel(ScreenConfig.ScreenConfig config)
        {
            H.Initialize(this);
            Config = config;
        }

        public Type ViewMode
        {
            get => _viewMode.Get();
            set => _viewMode.Set(value);
        }
        private readonly IProperty<Type> _viewMode 
            = H.Property<Type>(c=>c.Default(typeof(ViewModeDefault)));

        public ScreenConfig.ScreenConfig Config
        {
            get => _config.Get();
            set => _config.Set(value);
        }
        private readonly IProperty<ScreenConfig.ScreenConfig> _config = H.Property<ScreenConfig.ScreenConfig>();

        public double Width => _width.Get();
        private readonly IProperty<double> _width = H.Property<double>(c => c
            .NotNull(e => e.Config)
            .Set(e => e.Config.InMmOutsideBounds.Width * e.VisualRatio.X)
            .On(e => e.Config.InMmOutsideBounds)
            .On(e => e.VisualRatio.X)
            .Update()
        );

        public double Height => _height.Get();
        private readonly IProperty<double> _height = H.Property<double>(c => c
            .NotNull(e => e.Config)
            .Set(e => e.Config.InMmOutsideBounds.Height * e.VisualRatio.Y)
            .On(e => e.Config.InMmOutsideBounds)
            .On(e => e.VisualRatio.Y)
            .Update()
        );

        //public IScreenRatio VisualRatio { get; } = new ScreenRatioValue(1.0);
        public IScreenRatio VisualRatio => _visualRatio.Get();

        public IProperty<IScreenRatio> _visualRatio = H.Property<IScreenRatio>(c => c.Default((IScreenRatio)new ScreenRatioValue(1.0)));

        public void ConfigureMvvmContext(IMvvmContext ctx)
        {
            ctx.AddCreator<ScreenFrameViewModel>(vm => vm.Presenter = this);
        }
    }
}
