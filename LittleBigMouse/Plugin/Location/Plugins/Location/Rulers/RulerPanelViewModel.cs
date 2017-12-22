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
using System.Windows;
using Hlab.Mvvm;
using Hlab.Notify;
using LittleBigMouse.ScreenConfigs;

namespace LittleBigMouse.LocationPlugin.Plugins.Location.Rulers
{
    public class RulerPanelViewModel : ViewModel
    {
        public RulerPanelViewModel(Screen screen, Screen drawOn)
        {
            using (this.Suspend())
            {
                Screen = screen;
                DrawOn = drawOn;                
            }
        }
        public bool Enabled
        {
            get => this.Get<bool>(); set => this.Set(value);
        }
        public Screen DrawOn
        {
            get => this.Get<Screen>();
            set => this.Set(value);
        }
        public Screen Screen
        {
            get => this.Get<Screen>();
            set => this.Set(value);
        }
        public Visibility Visibility
        {
            get => this.Get<Visibility>(); set => this.Set(value);
        }

        public RulerViewModel TopRuler => this.Get(()=>new RulerViewModel(Screen,DrawOn,RulerViewModel.RulerSide.Top));
        public RulerViewModel RightRuler => this.Get(()=>new RulerViewModel(Screen,DrawOn,RulerViewModel.RulerSide.Right));
        public RulerViewModel BottomRuler => this.Get(()=>new RulerViewModel(Screen,DrawOn,RulerViewModel.RulerSide.Bottom));
        public RulerViewModel LeftRuler => this.Get(()=>new RulerViewModel(Screen,DrawOn,RulerViewModel.RulerSide.Left));

        [TriggedOn(nameof(DrawOn), "MmToDipRatio", "X")]
        public double RulerWidth => this.Get(() //=> 0);
            => 30 * DrawOn.MmToDipRatio.X);

        [TriggedOn(nameof(DrawOn), "MmToDipRatio", "Y")]
        public double RulerHeight => this.Get(()
            => 30 * DrawOn.MmToDipRatio.Y);
    }
}
