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
using HLab.Mvvm.Annotations;
using HLab.Mvvm.Legacy;
using HLab.Notify.PropertyChanged;
using LittleBigMouse.Control.Main;
using LittleBigMouse.Control.ScreenFrame;
using LittleBigMouse.DisplayLayout;
using LittleBigMouse.DisplayLayout.Dimensions;
using LittleBigMouse.Plugins;

namespace LittleBigMouse.Control.Wpf
{
    using H = H<MultiScreensViewModel>;

    public class MultiScreensViewModel : ViewModel, IPresenterViewModel, IMvvmContextProvider, IMultiScreensViewModel
    {
        public MainControlViewModel MainViewModel { get; }

        public MultiScreensViewModel(IMonitorsLayout config, MainControlViewModel mainViewModel)
        {
            MainViewModel = mainViewModel;

            H.Initialize(this);
            Layout = config;
        }

        public Type ViewMode
        {
            get => _viewMode.Get();
            set => _viewMode.Set(value);
        }

        readonly IProperty<Type> _viewMode 
            = H.Property<Type>(c=>c.Default(typeof(ViewModeDefault)));

        public IMonitorsLayout Layout
        {
            get => _layout.Get();
            set => _layout.Set(value);
        }

        readonly IProperty<IMonitorsLayout> _layout = H.Property<IMonitorsLayout>();

        public double Width => _width.Get();

        readonly IProperty<double> _width = H.Property<double>(c => c
            .NotNull(e => e.Layout)
            .Set(e => e.Layout.InMmOutsideBounds.Width * e.VisualRatio.X)
            .On(e => e.Layout.InMmOutsideBounds)
            .On(e => e.VisualRatio.X)
            .Update()
        );

        public double Height => _height.Get();

        readonly IProperty<double> _height = H.Property<double>(c => c
            .NotNull(e => e.Layout)
            .Set(e => e.Layout.InMmOutsideBounds.Height * e.VisualRatio.Y)
            .On(e => e.Layout.InMmOutsideBounds)
            .On(e => e.VisualRatio.Y)
            .Update()
        );

        //public IScreenRatio VisualRatio { get; } = new ScreenRatioValue(1.0);
        public IDisplayRatio VisualRatio => _visualRatio.Get();

        public IProperty<IDisplayRatio> _visualRatio = H.Property<IDisplayRatio>(c => c.Default((IDisplayRatio)new DisplayRatioValue(1.0)));

        public void ConfigureMvvmContext(IMvvmContext ctx)
        {
            ctx.AddCreator<ScreenFrameViewModel>(vm => vm.Presenter = this);
        }
    }
}
