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

using System.Reactive.Linq;
using Avalonia;
using Avalonia.Controls;
using DynamicData.Binding;
using HLab.Base.Avalonia.Extensions;
using HLab.Mvvm.ReactiveUI;
using LittleBigMouse.DisplayLayout.Monitors;
using LittleBigMouse.Plugin.Layout.Avalonia.Rulers;
using LittleBigMouse.Plugins;
using ReactiveUI;

namespace LittleBigMouse.Plugin.Layout.Avalonia.LocationPlugin;

internal class MonitorLocationViewModel : ViewModel<PhysicalMonitor>, IScreenContentViewModel
{
    static double GetScale(double scale, params double[] values)
    {
        var v = values.Min();
        var result = v * scale;

        if (double.IsNaN(result) || double.IsInfinity(result)) return 0.1;

        return result switch
        {
            < 0.1 => 0.1,
            > 35791 => 35791,
            _ => result
        };
    }

    public MonitorLocationViewModel()
    {
        _ratioX = this.WhenAnyValue(
        e => e.Model.DepthRatio.X,
        x => x * 100
        ).ToProperty(this, e => e.RatioX);

        _ratioY = this.WhenAnyValue(
        e => e.Model.DepthRatio.Y,
        x => x * 100
        ).ToProperty(this, e => e.RatioY);

        this
            .WhenPropertyChanged(e => e.Ruler)
            .Do(e => UpdateRulers())
            .Subscribe().DisposeWith(this);

        this.WhenAnyValue(
                e => e.MonitorFrameViewModel.MonitorsPresenter.SelectedMonitor)
            .Do(e =>
            {
                if (Ruler && e?.Model != Model)
                {
                    Ruler = false;
                }
            }).Subscribe().DisposeWith(this);

        Disposer.OnDispose(()=>
        {
            Ruler = false;
            UpdateRulers();
        });
    }


    public MonitorLocationPlugin Plugin
    {
        get => _plugin;
        set => this.RaiseAndSetIfChanged(ref _plugin, value);
    }
    MonitorLocationPlugin _plugin;

    public bool Ruler
    {
        get => _ruler;
        set => this.RaiseAndSetIfChanged(ref _ruler, value);
    }
    bool _ruler;

    readonly List<RulerPanelView> _panels = new();

    public void UpdateRulers()
    {
        foreach (var panel in _panels)
        {
            panel.Close();
        }
        _panels.Clear();

        if (!Ruler) return;

        MonitorFrameViewModel.Select();

        foreach (var source in MonitorFrameViewModel.MonitorsPresenter.Model.PhysicalSources)
        {
            if(!source.Source.AttachedToDesktop) continue;
            //var area = source.Source.Device.MonitorArea;
            var s = source.Source.InPixel;

            var panel = new RulerPanelView
            {
                Position = new PixelPoint((int)(s.Bounds.Left + s.Bounds.Width / 4), (int)(s.Bounds.Top + s.Bounds.Height / 4)),
                Width = 10, //area.Width*1,//, //s.Bounds.Width/* - 20*/,
                Height = 10, //area.Height*1,//, //s.Bounds.Height/* - 20*/,
                DataContext = new RulerPanelViewModel(Model, source)
            };
            panel.Show();
            panel.WindowState = WindowState.FullScreen;
            _panels.Add(panel);
        }
    }

    public double RatioX
    {
        get => _ratioX.Value;
        set
        {
            if (Model is null) return;
            Model.DepthRatio.X = value / 100;
            Model.Layout.Compact();
        }
    }

    readonly ObservableAsPropertyHelper<double> _ratioX;


    public double RatioY
    {
        get => _ratioY.Value;
        set
        {
            if (Model is null) return;
            Model.DepthRatio.Y = value / 100;
            Model.Layout.Compact();
        }
    }

    readonly ObservableAsPropertyHelper<double> _ratioY;

    public IMonitorFrameViewModel MonitorFrameViewModel
    {
        get => _monitorFrameViewModel;
        set => this.RaiseAndSetIfChanged(ref _monitorFrameViewModel, value);
    }
    IMonitorFrameViewModel _monitorFrameViewModel;

}
