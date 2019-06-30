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

using System;
using System.Windows;
using HLab.Notify.Annotations;
using HLab.Notify.PropertyChanged;
using LittleBigMouse.ScreenConfigs;

namespace LittleBigMouse.Plugin.Location.Plugins.Location.Rulers
{
    public class RulerPanelViewModel : N<RulerPanelViewModel>
    {
        public RulerPanelViewModel(Screen screen, Screen drawOn)
        {
            Screen = screen;
            DrawOn = drawOn;
            TopRuler = new RulerViewModel(Screen, DrawOn, RulerViewModel.RulerSide.Top);
            RightRuler = new RulerViewModel(Screen, DrawOn, RulerViewModel.RulerSide.Right);
            BottomRuler = new RulerViewModel(Screen, DrawOn, RulerViewModel.RulerSide.Bottom);
            LeftRuler = new RulerViewModel(Screen, DrawOn, RulerViewModel.RulerSide.Left);
            Initialize();
        }
        public Screen Screen { get; }
        public Screen DrawOn { get; }
        public RulerViewModel TopRuler { get; }
        public RulerViewModel RightRuler { get; }
        public RulerViewModel BottomRuler { get; }
        public RulerViewModel LeftRuler { get; }


        private readonly IProperty<bool> _enabled =H.Property<bool>();
        public bool Enabled
        {
            get => _enabled.Get();
            set => _enabled.Set(value);
        }

        public Visibility Visibility
        {
            get => _visibility.Get();
            set => _visibility.Set(value);
        }
        private readonly IProperty<Visibility> _visibility 
            = H.Property<Visibility>(nameof(Visibility));

        
        public double RulerWidth => _rulerWidth.Get();
        private readonly IProperty<double> _rulerWidth = H.Property<double>(c => c
            .On(e => e.DrawOn.MmToDipRatio.X)
            .Set(e => 30 * e.DrawOn.MmToDipRatio.X)
        );

        public double RulerHeight => _rulerHeight.Get();
        private readonly IProperty<double> _rulerHeight = H.Property<double>(c => c
            .On(e => e.DrawOn.MmToDipRatio.Y)
            .Set(e => 30 * e.DrawOn.MmToDipRatio.Y)
        );
    }
}
