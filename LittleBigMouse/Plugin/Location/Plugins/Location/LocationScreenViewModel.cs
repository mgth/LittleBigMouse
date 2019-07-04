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
using System.Windows;
using System.Windows.Media;
using HLab.Mvvm;
using HLab.Notify;
using HLab.Notify.Annotations;
using HLab.Notify.PropertyChanged;
using LittleBigMouse.Plugin.Location.Plugins.Location.Rulers;
using LittleBigMouse.ScreenConfigs;
using RulerPanelView = LittleBigMouse.Plugin.Location.Plugins.Location.Rulers.RulerPanelView;
using RulerView = LittleBigMouse.Plugin.Location.Plugins.Location.Rulers.RulerView;

namespace LittleBigMouse.Plugin.Location.Plugins.Location
{
    class LocationScreenViewModel : ViewModel<LocationScreenViewModel,Screen>
    {
        public LocationScreenViewModel()
        {
        }

        public ScreenLocationPlugin Plugin
        {
            get => _plugin.Get();
            set => _plugin.Set(value);
        }
        private readonly IProperty<ScreenLocationPlugin> _plugin = H.Property<ScreenLocationPlugin>();

        public bool Ruler
        {
            get => _ruler.Get();
            set => _ruler.Set(value);
        }
        private readonly IProperty<bool> _ruler = H.Property<bool>();

        public bool RulerMouseOver
        {
            get => _rulerMouseOver.Get();
            set => _rulerMouseOver.Set(value);
        }
        private readonly IProperty<bool> _rulerMouseOver = H.Property<bool>();


        private readonly List<RulerView> _rulers = new List<RulerView>();

        private readonly List<RulerPanelView> _panels = new List<RulerPanelView>();

        [TriggerOn(nameof(Model), "Selected")]
        public void UpdateSelected()
        {
            if (Model == null) return;
            if (Model.Selected) return;

            Ruler = false;
        }

        [TriggerOn(nameof(Ruler))]
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
                    Left = s.Bounds.Left+s.Bounds.Width/4, // if <=Left or >=Left+Width/2  panel maximize to wrong screen
                    Top = s.Bounds.Top+s.Bounds.Height/4,
                    Width = 0, //s.Bounds.Width/* - 20*/,
                    Height = 0, //s.Bounds.Height/* - 20*/,
                    DataContext = new RulerPanelViewModel(Model,screen)
                };
                panel .Show();

                panel.WindowState = WindowState.Maximized;

                _panels.Add(panel);
            }
        }

        public Brush RulerForegroundColor => _rulerForegroundColor.Get();
        private readonly IProperty<Brush> _rulerForegroundColor = H.Property<Brush>(c => c
            .On(e => e.Ruler)
            .On(e => e.RulerMouseOver)
            .Set(e => e.Ruler ? new SolidColorBrush(Colors.White) : e.RulerMouseOver ? new SolidColorBrush(Colors.Black) : new SolidColorBrush(Colors.Black))
        );

        public Brush RulerBackgroundColor => _rulerBackgroundColor.Get();
        private readonly IProperty<Brush> _rulerBackgroundColor = H.Property<Brush>(c => c
            .On(e => e.Ruler)
            .On(e => e.RulerMouseOver)
            .Set(e => e.RulerMouseOver ? new SolidColorBrush(Colors.DarkBlue) : e.Ruler ? new SolidColorBrush(Colors.Black) : new SolidColorBrush(Colors.White))
        );


        public double RatioX
        {
            get => _ratioX.Get();
            set
            {
                Model.PhysicalRatio.X = value/100;
                Model.Config.Compact();
            }
        }
        private readonly IProperty<double> _ratioX = H.Property<double>(c => c
            .On(e => e.Model.PhysicalRatio.X)
            .Set(e => e.Model.PhysicalRatio.X * 100)
        );

        public double RatioY
        {
            get => _ratioY.Get();
            set
            {
                Model.PhysicalRatio.Y = value / 100;
                Model.Config.Compact();
            }
        }
        private readonly IProperty<double> _ratioY = H.Property<double>(c => c
            .On(e => e.Model.PhysicalRatio.Y)
            .Set(e => e.Model.PhysicalRatio.Y * 100)
        );

        public VerticalAlignment DpiVerticalAlignment => _dpiVerticalAlignment.Get();
        private readonly IProperty<VerticalAlignment> _dpiVerticalAlignment = H.Property<VerticalAlignment>(c => c
            .On(e => e.Model.Orientation)
            .Set(e => e.Model.Orientation == 3 ? VerticalAlignment.Bottom : VerticalAlignment.Top)
        );

        public VerticalAlignment PnpNameVerticalAlignment => _pnpNameVerticalAlignment.Get();
        private readonly IProperty<VerticalAlignment> _pnpNameVerticalAlignment = H.Property<VerticalAlignment>(c => c
            .On(e => e.Model.Orientation)
            .Set(e => e.Model.Orientation == 2 ? VerticalAlignment.Bottom : VerticalAlignment.Top)
        );

    }
}
