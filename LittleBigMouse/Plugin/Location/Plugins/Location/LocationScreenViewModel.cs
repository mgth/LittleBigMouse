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
using System.Collections.Generic;
using System.ComponentModel;
using System.Windows;
using System.Windows.Media;
using HLab.Mvvm;
using HLab.Notify;
using LittleBigMouse.LocationPlugin.Plugins.Location.Rulers;
using LittleBigMouse.ScreenConfigs;
using RulerView = LittleBigMouse.LocationPlugin.Plugins.Location.Rulers.RulerView;

namespace LittleBigMouse.LocationPlugin.Plugins.Location
{
    class LocationScreenViewModel : IViewModel<Screen>
    {
        public Screen Model => this.GetModel();
        public event PropertyChangedEventHandler PropertyChanged
        {
            add => this.Add(value);
            remove => this.Remove(value);
        }


        public ScreenLocationPlugin Plugin
        {
            get => this.Get<ScreenLocationPlugin>(); set => this.Set(value);
        }

        public bool Ruler
        {
            get => this.Get(() => false);
            set => this.Set(value);
        }
        public bool RulerMouseOver
        {
            get => this.Get(() => false);
            set => this.Set(value);
        }


        private readonly List<RulerView> _rulers = new List<RulerView>();

        public LocationScreenViewModel()
        {
            this.SubscribeNotifier();
        }



        private readonly List<RulerPanelView> _panels = new List<RulerPanelView>();

        [TriggedOn(nameof(Model), "Selected")]
        public void UpdateSelected()
        {
            if (Model == null) return;
            if (Model.Selected) return;

            Ruler = false;
        }

        [TriggedOn(nameof(Ruler))]
        public void UpdateRulers()
        {
            foreach (var panel in _panels)
            {
                panel.Close();
            }
            _panels.Clear();

            if (!Ruler) return;

            Model.Selected = true;

            foreach (var screen in Model.Config.AllScreens)
            {
                var s = screen.InDip;

                var panel = new RulerPanelView
                {
                    Left = s.Bounds.Left,
                    Top = s.Bounds.Top,
                    Width = 0, //s.Bounds.Width/* - 20*/,
                    Height = 0, //s.Bounds.Height/* - 20*/,
                    DataContext = new RulerPanelViewModel(Model,screen)
                };
                panel .Show();

                panel.Height = s.Bounds.Height/* - 20*/;
                panel.Width = s.Bounds.Width/* - 20*/;

                panel.WindowState = WindowState.Maximized;

                _panels.Add(panel);
            }
        }


        [TriggedOn(nameof(Ruler))]
        [TriggedOn(nameof(RulerMouseOver))]
        public Brush RulerForegroundColor => this.Get(() => Ruler ? new SolidColorBrush(Colors.White): RulerMouseOver?new SolidColorBrush(Colors.Black) : new SolidColorBrush(Colors.Black));

        [TriggedOn(nameof(Ruler))]
        [TriggedOn(nameof(RulerMouseOver))]
        public Brush RulerBackgroundColor => this.Get(() => RulerMouseOver ? new SolidColorBrush(Colors.DarkBlue) : Ruler ? new SolidColorBrush(Colors.Black) : new SolidColorBrush(Colors.White));


        [TriggedOn(nameof(Model),"PhysicalRatio","X")]
        public double RatioX
        {
            get => this.Get(()=> Model.PhysicalRatio.X * 100);
            set
            {
                Model.PhysicalRatio.X = value/100;
                Model.Config.Compact();
            }
        }

        [TriggedOn(nameof(Model), "PhysicalRatio","Y")]
        public double RatioY
        {
            get => this.Get(()=> Model.PhysicalRatio.Y * 100);
            set
            {
                Model.PhysicalRatio.Y = value / 100;
                Model.Config.Compact();
            }
        }

        [TriggedOn(nameof(Model), "Orientation")]
        public VerticalAlignment DpiVerticalAlignment
            => this.Get(()=> Model.Orientation == 3 ? VerticalAlignment.Bottom : VerticalAlignment.Top);

        [TriggedOn(nameof(Model), "Orientation")]
        public VerticalAlignment PnpNameVerticalAlignment
            => this.Get(()=> Model.Orientation == 2 ? VerticalAlignment.Bottom : VerticalAlignment.Top);






    }
}
