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

using HLab.Notify.PropertyChanged;

using LittleBigMouse.DisplayLayout;

namespace LittleBigMouse.Plugin.Location.Plugins.Location.Rulers;

using H = H<RulerViewModel>;

public class RulerViewModel : NotifierBase
{
    public Monitor Screen { get; }
    public int Orientation { get; }
    public MonitorSource DrawOn { get; }

    public RulerViewModel(Monitor screen, MonitorSource drawOn, int orientation)
    {
        Orientation = orientation;
        switch (Orientation)
        {
            case 0:
                Vertical = false;
                Horizontal = true;
                Revert = false;
                break;
            case 1:
                Vertical = true;
                Horizontal = false;
                Revert = true;
                break;
            case 2:
                Vertical = false;
                Horizontal = true;
                Revert = true;
                break;
            case 3:
                Vertical = true;
                Horizontal = false;
                Revert = false;
                break;
            default:
                throw new ArgumentOutOfRangeException(nameof(Orientation), orientation, null);
        }
        Screen = screen;
        DrawOn = drawOn;
        H.Initialize(this);
        Selected = ReferenceEquals(DrawOn, Screen);
    }

    public bool Enabled
    {
        get => _enable.Get();
        set => _enable.Set(value);
    }
    private readonly IProperty<bool> _enable = H.Property<bool>();
    public bool Selected
    {
        get => _selected.Get();
        set => _selected.Set(value);
    }
    private readonly IProperty<bool> _selected = H.Property<bool>();

    public double ZeroX => _zeroX.Get();
    private readonly IProperty<double> _zeroX = H.Property<double>(c => c
        .Set(e => e.Screen.XMoving - e.DrawOn.Monitor.XMoving)
        .On(e => e.DrawOn.Monitor.XMoving)
        .On(e => e.Screen.XMoving)
        .Update()
    );

    public double ZeroY => _zeroY.Get();
    private readonly IProperty<double> _zeroY = H.Property<double>(c => c
        .Set(e => e.Screen.YMoving - e.DrawOn.Monitor.YMoving)
        .On(e => e.DrawOn.Monitor.YMoving)
        .On(e => e.Screen.YMoving)
        .Update()
    );

    //public Thickness Margin => _margin.Get();
    //private readonly IProperty<Thickness> _margin = H.Property<Thickness>( c => c
    //    .On(e => e.RatioY)
    //    .On(e => e.DrawOn.YMoving)
    //    .On(e => e.Screen.YMoving)
    //    .Set(e => new Thickness((!e.Horizontal) ? e.ZeroX : 0, e.Vertical ? e.ZeroY : 0, 0, 0))
    //);



    public bool Vertical { get; }
    public bool Horizontal { get; }
    public bool Revert { get; }

    public double RatioX => _ratioX.Get();
    private readonly IProperty<double> _ratioX = H.Property<double>(c => c
       .Set(e => e.DrawOn.InDip.Width / e.DrawOn.Monitor.InMm.Width)
       .On(e => e.DrawOn.InDip.Width)
       .On(e => e.DrawOn.Monitor.InMm.Width)
       .Update()
    );

    public double RatioY => _ratioY.Get();
    private readonly IProperty<double> _ratioY = H.Property<double>(c => c
       .Set(e => e.DrawOn.InDip.Height / e.DrawOn.Monitor.InMm.Height)
       .On(e => e.DrawOn.InDip.Height)
       .On(e => e.DrawOn.Monitor.InMm.Height)
       .Update()
    );

    //public double RulerHeight => _rulerHeight.Get();
    //private readonly IProperty<double> _rulerHeight = H.Property<double>(c => c
    //        .On(e => e.RatioY)
    //        .On(e => e.Screen.InMm.Height)
    //        .Set(e => e.Vertical ? (e.Screen.InMm.Height * e.RatioY) : double.NaN)
    //);

    //public double RulerWidth => _rulerWidth.Get();
    //private readonly IProperty<double> _rulerWidth = H.Property<double>( c => c
    //        .On(e => e.RatioX)
    //        .On(e => e.Screen.InMm.Width)
    //        .Set(e => e.Horizontal ? (e.Screen.InMm.Width * e.RatioX) : double.NaN)
    //    );

    public double RulerLength => _rulerLength.Get();
    private readonly IProperty<double> _rulerLength = H.Property<double>(c => c
            .Set(e => e.Vertical ? e.Screen.InMm.Height : e.Screen.InMm.Width)
            .On(e => e.Screen.InMm.Width)
            .On(e => e.Screen.InMm.Height)
            .Update()
        );

    public double RulerEnd => _rulerEnd.Get();
    private readonly IProperty<double> _rulerEnd = H.Property<double>(c => c
        .Set(e => e.RulerStart + e.RulerLength)
        .On(e => e.RulerLength)
        .On(e => e.RulerStart)
        .Update()
    );

    /// <summary>
    /// Calculate the first tick value of the ruler
    /// </summary>
    public double RulerStart => _rulerStart.Get();
    private readonly IProperty<double> _rulerStart = H.Property<double>(c => c
        .Set(e => e.Vertical ? e.DrawOn.Monitor.YMoving - e.Screen.YMoving : e.DrawOn.Monitor.XMoving - e.Screen.XMoving)
        .On(e => e.DrawOn.Monitor.XMoving)
        .On(e => e.DrawOn.Monitor.YMoving)
        .On(e => e.Screen.XMoving)
        .On(e => e.Screen.YMoving)
        .Update()
    );

    public double Length => _length.Get();
    private readonly IProperty<double> _length = H.Property<double>(c => c
        .Set(e => e.Vertical ? e.DrawOn.Monitor.InMm.Height : e.DrawOn.Monitor.InMm.Width)
        .On(e => e.DrawOn.Monitor.InMm.Width)
        .On(e => e.DrawOn.Monitor.InMm.Height)
        .Update()
    );

    public double Size => _size.Get();
    private readonly IProperty<double> _size = H.Property<double>(c => c
        .Set(e => 30.0)
    //.On(e => e.RatioX)
    //.On(e => e.RatioY)
    //.Update()
    );

    public double Zero => _zero.Get();
    private readonly IProperty<double> _zero = H.Property<double>(c => c
        .Set(e => e.Vertical ? e.ZeroY : e.ZeroX)
        .On(e => e.ZeroX)
        .On(e => e.ZeroY)
        .Update()
    );
}
