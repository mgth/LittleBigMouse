/*
  LittleBigMouse.Screen.Config
  Copyright (c) 2021 Mathieu GRENET.  All right reserved.

  This file is part of LittleBigMouse.Screen.Config.

    LittleBigMouse.Screen.Config is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    LittleBigMouse.Screen.Config is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with MouseControl.  If not, see <http://www.gnu.org/licenses/>.

	  mailto:mathieu@mgth.fr
	  http://www.mgth.fr
*/

using LittleBigMouse.DisplayLayout.Monitors;
using ReactiveUI;
using System.Reactive.Concurrency;

namespace LittleBigMouse.DisplayLayout.Dimensions;

public class DisplayScaleDip : DisplaySize
{
    public IDisplayRatio EffectiveDpi { get; }
    public IMonitorsLayout Layout { get; }

    public DisplayScaleDip(IDisplaySize source, IDisplayRatio effectiveDpi, IMonitorsLayout layout) : base(source)
    {
        EffectiveDpi = effectiveDpi;
        Layout = layout;

        _ratio = this.WhenAnyValue(
            e => e.EffectiveDpi.X,
            e => e.EffectiveDpi.Y,
            (x, y) => new DisplayRatioValue(96 / x, 96 / y))
            .ToProperty(this,e=>e.Ratio, scheduler: Scheduler.Immediate);

        _x = this.WhenAnyValue(
            e => e.Source.X,
            e => e.Ratio.X,
            (h, r) => h * r
            ).ToProperty(this,e=>e.X, scheduler: Scheduler.Immediate);

        _y = this.WhenAnyValue(
            e => e.Source.Y, 
            e => e.Ratio.Y, 
            (h, r) => h * r
            ).ToProperty(this, e => e.Y, scheduler: Scheduler.Immediate);

        _mainRatio = this.WhenAnyValue(
            e => e.Layout.PrimarySource.EffectiveDpi.X,
            e => e.Layout.PrimarySource.EffectiveDpi.Y,
            (x, y) => new DisplayRatioValue(96 / x, 96 / y))
            .ToProperty(this,e=>e.MainRatio, scheduler: Scheduler.Immediate);

        _width = this.WhenAnyValue(
            e => e.Source.Width,
            e => e.Ratio.X,
            (w, r) => w * r)
            .ToProperty(this,e=>e.Width, scheduler: Scheduler.Immediate);

        _height = this.WhenAnyValue(
            e => e.Source.Height,
            e => e.Ratio.Y,
            (h, r) => h * r)
            .ToProperty(this,e=>e.Height, scheduler: Scheduler.Immediate);

        _topBorder = this.WhenAnyValue(
            e => e.Source.TopBorder, 
            e => e.Ratio.Y, 
            (b, r) => b * r
            ).ToProperty(this, e => e.TopBorder, scheduler: Scheduler.Immediate);

        _bottomBorder = this.WhenAnyValue(
            e => e.Source.BottomBorder, 
            e => e.Ratio.Y, 
            (b, r) => b * r
            ).ToProperty(this, e => e.BottomBorder, scheduler: Scheduler.Immediate);

        _leftBorder = this.WhenAnyValue(
            e => e.Source.LeftBorder, 
            e => e.Ratio.X, 
            (b, r) => b * r
            ).ToProperty(this, e => e.LeftBorder, scheduler: Scheduler.Immediate);

        _rightBorder = this.WhenAnyValue(
            e => e.Source.RightBorder, 
            e => e.Ratio.X, 
            (b, r) => b * r
            ).ToProperty(this, e => e.RightBorder, scheduler: Scheduler.Immediate);

        Init();
    }

    public IDisplayRatio Ratio => _ratio.Value;
    readonly ObservableAsPropertyHelper<IDisplayRatio> _ratio;
 
    public IDisplayRatio MainRatio => _mainRatio.Value;
    readonly ObservableAsPropertyHelper<IDisplayRatio> _mainRatio;

    public override double Width
    {
        get => _width.Value;
        set => Source.Width = value / Ratio.X;
    }
    readonly ObservableAsPropertyHelper<double> _width;

    public override double Height
    {
        get => _height.Value;
        set => Source.Height = value / Ratio.Y;
    }
    readonly ObservableAsPropertyHelper<double> _height;

    public override double X
    {
        get => _x.Value;
        set => Source.X = value / MainRatio.X;
    }
    readonly ObservableAsPropertyHelper<double> _x;

    public override double Y
    {
        get => _y.Value;
        set => Source.Y = value / MainRatio.Y;
    }
    readonly ObservableAsPropertyHelper<double> _y;

    public override double TopBorder
    {
        get => _topBorder.Value;
        set => Source.TopBorder = value / Ratio.Y;
    }
    readonly ObservableAsPropertyHelper<double> _topBorder;

    public override double BottomBorder
    {
        get => _bottomBorder.Value;
        set => Source.BottomBorder = value / Ratio.Y;
    }
    readonly ObservableAsPropertyHelper<double> _bottomBorder;

    public override double LeftBorder
    {
        get => _leftBorder.Value;
        set => Source.LeftBorder = value / Ratio.X;
    }
    readonly ObservableAsPropertyHelper<double> _leftBorder;

    public override double RightBorder
    {
        get => _rightBorder.Value;
        set => Source.RightBorder = value / Ratio.X;
    }
    readonly ObservableAsPropertyHelper<double> _rightBorder;

    public override string TransformToString => $"DPI:{EffectiveDpi}";

}
