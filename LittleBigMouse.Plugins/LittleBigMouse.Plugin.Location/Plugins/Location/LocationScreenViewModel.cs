/*
  LittleBigMouse.Plugin.Location
  Copyright (c) 2021 Mathieu GRENET.  All right reserved.

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
using HLab.Notify.Annotations;
using HLab.Notify.PropertyChanged;

using LittleBigMouse.DisplayLayout;
using LittleBigMouse.Plugin.Location.Plugins.Location.Rulers;
using LittleBigMouse.Plugins;

using RulerPanelView = LittleBigMouse.Plugin.Location.Plugins.Location.Rulers.RulerPanelView;
using RulerView = LittleBigMouse.Plugin.Location.Plugins.Location.Rulers.RulerView;

namespace LittleBigMouse.Plugin.Location.Plugins.Location;

using H = H<LocationScreenViewModel>;

class LocationScreenViewModel : ViewModel<Monitor>, IScreenContentViewModel
{
    public LocationScreenViewModel()
    {
        H.Initialize(this);
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

        foreach (var source in Model.Layout.AllSources.Items)
        {
            var area = source.Device.MonitorArea;
            var s = source.InDip;

            var panel = new RulerPanelView
            {
                Left = s.Bounds.Left + s.Bounds.Width / 4, //area.Left, //+area.Width/10, // // if <=Left or >=Left+Width/2  panel maximize to wrong screen
                Top = s.Bounds.Top + s.Bounds.Height / 4, //area.Top, //+area.Height/10, //
                Width = 0, //area.Width*1,//, //s.Bounds.Width/* - 20*/,
                Height = 0, //area.Height*1,//, //s.Bounds.Height/* - 20*/,
                DataContext = new RulerPanelViewModel(Model, source)
            };
            panel.Show();

            panel.WindowState = WindowState.Maximized;

            _panels.Add(panel);
        }
    }

    public double RatioX
    {
        get => _ratioX.Get();
        set
        {
            Model.PhysicalRatio.X = value / 100;
            Model.Layout.Compact();
        }
    }
    private readonly IProperty<double> _ratioX = H.Property<double>(c => c
        .Set(e => e.Model.PhysicalRatio.X * 100)
        .On(e => e.Model.PhysicalRatio.X)
        .Update()
    );

    public double RatioY
    {
        get => _ratioY.Get();
        set
        {
            Model.PhysicalRatio.Y = value / 100;
            Model.Layout.Compact();
        }
    }
    private readonly IProperty<double> _ratioY = H.Property<double>(c => c
        .Set(e => e.Model.PhysicalRatio.Y * 100)
        .On(e => e.Model.PhysicalRatio.Y)
        .Update()
    );

    public VerticalAlignment DpiVerticalAlignment => _dpiVerticalAlignment.Get();
    private readonly IProperty<VerticalAlignment> _dpiVerticalAlignment = H.Property<VerticalAlignment>(c => c
        .Set(e => e.Model.Orientation == 3 ? VerticalAlignment.Bottom : VerticalAlignment.Top)
        .On(e => e.Model.Orientation)
        .Update()
    );

    public VerticalAlignment PnpNameVerticalAlignment => _pnpNameVerticalAlignment.Get();
    private readonly IProperty<VerticalAlignment> _pnpNameVerticalAlignment = H.Property<VerticalAlignment>(c => c
        .Set(e => e.Model.Orientation == 2 ? VerticalAlignment.Bottom : VerticalAlignment.Top)
        .On(e => e.Model.Orientation)
        .Update()
    );

    public IScreenFrameViewModel ScreenFrameViewModel
    {
        get => _screenFrameViewModel.Get();
        set => _screenFrameViewModel.Set(value);
    }
    private readonly IProperty<IScreenFrameViewModel> _screenFrameViewModel = H.Property<IScreenFrameViewModel>();

}
