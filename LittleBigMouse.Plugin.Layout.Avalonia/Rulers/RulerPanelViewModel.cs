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

using HLab.Mvvm.ReactiveUI;
using LittleBigMouse.DisplayLayout.Monitors;
using ReactiveUI;

namespace LittleBigMouse.Plugin.Layout.Avalonia.Rulers;

public class RulerPanelViewModel : ViewModel
{
    public RulerPanelViewModel(PhysicalMonitor monitor, PhysicalSource drawOn)
    {
        Monitor = monitor;
        DrawOn = drawOn;
        TopRuler = new RulerViewModel(Monitor, DrawOn, 0);
        RightRuler = new RulerViewModel(Monitor, DrawOn, 1);
        BottomRuler = new RulerViewModel(Monitor, DrawOn, 2);
        LeftRuler = new RulerViewModel(Monitor, DrawOn, 3);

        _rulerWidth = this.WhenAnyValue(
            e => e.DrawOn.MmToDipRatio.X, x => 30 * x)
            .ToProperty(this, e => e.RulerWidth);

        _rulerHeight = this.WhenAnyValue(
            e => e.DrawOn.MmToDipRatio.Y, y => 30 * y)
            .ToProperty(this, e => e.RulerHeight);
    }

    public PhysicalMonitor Monitor { get; }
    public PhysicalSource DrawOn { get; }
    public RulerViewModel TopRuler { get; }
    public RulerViewModel RightRuler { get; }
    public RulerViewModel BottomRuler { get; }
    public RulerViewModel LeftRuler { get; }


    public bool Enabled
    {
        get => _enabled;
        set => this.RaiseAndSetIfChanged(ref _enabled, value);
    }
    bool _enabled;

    public bool Visibility
    {
        get => _visibility;
        set => this.RaiseAndSetIfChanged(ref _visibility, value);
    }
    bool _visibility;

    public double RulerWidth => _rulerWidth.Value;
    readonly ObservableAsPropertyHelper<double> _rulerWidth;

    public double RulerHeight => _rulerHeight.Value;
    readonly ObservableAsPropertyHelper<double> _rulerHeight;


}
