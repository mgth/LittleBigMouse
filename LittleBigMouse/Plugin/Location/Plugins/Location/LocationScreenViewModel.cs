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
using LittleBigMouse.LocationPlugin.Plugins.Location.Rulers;
using LittleBigMouse.Plugin.Location.Plugins.Location.Rulers;
using LittleBigMouse.ScreenConfigs;
using RulerView = LittleBigMouse.Plugin.Location.Plugins.Location.Rulers.RulerView;

namespace LittleBigMouse.Plugin.Location.Plugins.Location
{
    class LocationScreenViewModel : ViewModel<LocationScreenViewModel,Screen>
    {
        public LocationScreenViewModel()
        {
            Initialize();
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

        private readonly IProperty<Brush> _rulerForegroundColor = H.Property<Brush>();
        public Brush RulerForegroundColor => _rulerForegroundColor.Get();
        [TriggerOn(nameof(Ruler))]
        [TriggerOn(nameof(RulerMouseOver))]
        private void _setRulerForegroundColor() => _rulerForegroundColor.Set(Ruler ? new SolidColorBrush(Colors.White): RulerMouseOver?new SolidColorBrush(Colors.Black) : new SolidColorBrush(Colors.Black));

        private readonly IProperty<Brush> _rulerBackgroundColor = H.Property<Brush>();
        public Brush RulerBackgroundColor => _rulerBackgroundColor.Get();
        [TriggerOn(nameof(Ruler))]
        [TriggerOn(nameof(RulerMouseOver))]
        private void _set_RulerBackgroundColor() => _rulerBackgroundColor.Set(RulerMouseOver ? new SolidColorBrush(Colors.DarkBlue) : Ruler ? new SolidColorBrush(Colors.Black) : new SolidColorBrush(Colors.White));


        [TriggerOn(nameof(Model),"PhysicalRatio","X")]
        public double RatioX
        {
            get => Model.PhysicalRatio.X * 100;
            set
            {
                Model.PhysicalRatio.X = value/100;
                Model.Config.Compact();
            }
        }

        [TriggerOn(nameof(Model), "PhysicalRatio","Y")]
        public double RatioY
        {
            get => Model.PhysicalRatio.Y * 100;
            set
            {
                Model.PhysicalRatio.Y = value / 100;
                Model.Config.Compact();
            }
        }

        [TriggerOn(nameof(Model), "Orientation")]
        public VerticalAlignment DpiVerticalAlignment
            => Model.Orientation == 3 ? VerticalAlignment.Bottom : VerticalAlignment.Top;

        [TriggerOn(nameof(Model), "Orientation")]
        public VerticalAlignment PnpNameVerticalAlignment
            => Model.Orientation == 2 ? VerticalAlignment.Bottom : VerticalAlignment.Top;






    }
}
