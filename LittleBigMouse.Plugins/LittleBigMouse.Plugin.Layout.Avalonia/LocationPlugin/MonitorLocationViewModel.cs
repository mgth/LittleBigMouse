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
using Avalonia.Platform;
using DynamicData.Binding;
using HLab.Base.ReactiveUI;
using HLab.Mvvm.ReactiveUI;
using LittleBigMouse.DisplayLayout.Monitors;
using LittleBigMouse.Plugin.Layout.Avalonia.Rulers;
using LittleBigMouse.Plugins;
using LittleBigMouse.Plugins.Avalonia;
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
                if (Ruler && e != Model)
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

    readonly List<RulerWindow> _panels = new();

    // Ruler thickness in DIPs, like the 100-DIP bands of the former fullscreen
    // panel grid: the ruler drawing works in DIPs, so a thickness in any other
    // unit clips the 10cm graduations and their digits on scaled monitors.
    const double RulerThickness = 100.0;

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

            var layoutBounds = source.Source.InPixel.Bounds;
            var viewModel = new RulerPanelViewModel(Model, source);

            var top = new RulerWindow(viewModel.TopRuler);
            var bottom = new RulerWindow(viewModel.BottomRuler);
            var left = new RulerWindow(viewModel.LeftRuler);
            var right = new RulerWindow(viewModel.RightRuler);

            // The layout space and the windowing-system space may differ:
            // KWin maps every XWayland output with a single global factor
            // (the largest output scale), so the monitor's window-system
            // geometry has to come from the matching Avalonia screen.
            var screen = ScreenFinder.FromLayoutBounds(top.Screens, layoutBounds);
            var bounds = screen?.Bounds ?? new PixelRect(
                (int)layoutBounds.X, (int)layoutBounds.Y,
                (int)layoutBounds.Width, (int)layoutBounds.Height);
            var scaling = screen?.Scaling ?? source.Source.EffectiveDpi.Y / 96.0;

            var thickness = RulerThickness * scaling;

            // Horizontal rulers first, vertical ones last so they end up on
            // top in the corners, like in the former single-panel layout.
            ShowRulerWindow(top, new PixelPoint(bounds.X, bounds.Y), bounds.Width, thickness, scaling);
            ShowRulerWindow(bottom, new PixelPoint(bounds.X, (int)(bounds.Bottom - thickness)), bounds.Width, thickness, scaling);
            ShowRulerWindow(left, new PixelPoint(bounds.X, bounds.Y), thickness, bounds.Height, scaling);
            ShowRulerWindow(right, new PixelPoint((int)(bounds.Right - thickness), bounds.Y), thickness, bounds.Height, scaling);
        }
    }

    void ShowRulerWindow(RulerWindow panel, PixelPoint position, double pixelWidth, double pixelHeight, double scaling)
    {
        panel.ShowAt(position, pixelWidth, pixelHeight, scaling);
        _panels.Add(panel);
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
