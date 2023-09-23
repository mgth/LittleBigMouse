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

using System;
using System.Windows;

using HLab.Notify.Annotations;
using HLab.Notify.PropertyChanged;

using LittleBigMouse.DisplayLayout;

namespace LittleBigMouse.Plugin.Location.Plugins.Location.Rulers;

using H = H<RulerPanelViewModel>;

public class RulerPanelViewModel : NotifierBase
{
    public RulerPanelViewModel(Monitor monitor, MonitorSource drawOn)
    {
        Monitor = monitor;
        DrawOn = drawOn;
        TopRuler = new RulerViewModel(Monitor, DrawOn, 0);
        RightRuler = new RulerViewModel(Monitor, DrawOn, 1);
        BottomRuler = new RulerViewModel(Monitor, DrawOn, 2);
        LeftRuler = new RulerViewModel(Monitor, DrawOn, 3);
        H.Initialize(this);
    }
    public Monitor Monitor { get; }
    public MonitorSource DrawOn { get; }
    public RulerViewModel TopRuler { get; }
    public RulerViewModel RightRuler { get; }
    public RulerViewModel BottomRuler { get; }
    public RulerViewModel LeftRuler { get; }


    private readonly IProperty<bool> _enabled = H.Property<bool>();
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
        = H.Property<Visibility>();


    public double RulerWidth => _rulerWidth.Get();
    private readonly IProperty<double> _rulerWidth = H.Property<double>(c => c
        .Set(e => 30 * e.DrawOn.MmToDipRatio.X)
        .On(e => e.DrawOn.MmToDipRatio.X)
        .Update()
    );

    public double RulerHeight => _rulerHeight.Get();
    private readonly IProperty<double> _rulerHeight = H.Property<double>(c => c
        .Set(e => 30 * e.DrawOn.MmToDipRatio.Y)
        .On(e => e.DrawOn.MmToDipRatio.Y)
        .Update()
    );
}
