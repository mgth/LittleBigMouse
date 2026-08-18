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

using System.Reactive.Concurrency;
using ReactiveUI;

namespace LittleBigMouse.DisplayLayout.Dimensions;

public static class ScreenSizeWpfExt
{
    public static IMutableDisplayBounds Wpf(this DisplaySizeInPixels source, IDisplayRatio ratio) => new DisplaySizeWpf(source, ratio);
}
public class DisplaySizeWpf : MutableDisplayBounds
{
    public DisplaySizeWpf(IMutableDisplayBounds source, IDisplayRatio ratio) : base(source)
    {
        MutableSource = source;
        Ratio = ratio;

        _width = this.WhenAnyValue(e => e.Source.Width, e => e.Ratio.X, (value, scale) => value * scale)
            .ToProperty(this, e => e.Width, scheduler: Scheduler.Immediate);
        _height = this.WhenAnyValue(e => e.Source.Height, e => e.Ratio.Y, (value, scale) => value * scale)
            .ToProperty(this, e => e.Height, scheduler: Scheduler.Immediate);
        _x = this.WhenAnyValue(e => e.Source.X)
            .ToProperty(this, e => e.X, scheduler: Scheduler.Immediate);
        _y = this.WhenAnyValue(e => e.Source.Y)
            .ToProperty(this, e => e.Y, scheduler: Scheduler.Immediate);
        _topBorder = this.WhenAnyValue(e => e.Source.TopBorder, e => e.Ratio.Y, (value, scale) => value * scale)
            .ToProperty(this, e => e.TopBorder, scheduler: Scheduler.Immediate);
        _bottomBorder = this.WhenAnyValue(e => e.Source.BottomBorder, e => e.Ratio.Y, (value, scale) => value * scale)
            .ToProperty(this, e => e.BottomBorder, scheduler: Scheduler.Immediate);
        _leftBorder = this.WhenAnyValue(e => e.Source.LeftBorder, e => e.Ratio.X, (value, scale) => value * scale)
            .ToProperty(this, e => e.LeftBorder, scheduler: Scheduler.Immediate);
        _rightBorder = this.WhenAnyValue(e => e.Source.RightBorder, e => e.Ratio.X, (value, scale) => value * scale)
            .ToProperty(this, e => e.RightBorder, scheduler: Scheduler.Immediate);

        Init();
    }

    IMutableDisplayBounds MutableSource { get; }

    public IDisplayRatio Ratio
    {
       get;
       private set => field = value;
    }

    public override double Width
    {
        get => _width.Value;
        set => MutableSource.Width = value / Ratio.X;
    }
    readonly ObservableAsPropertyHelper<double> _width;

    public override double Height
    {
        get => _height.Value;
        set => MutableSource.Height = value / Ratio.Y;
    }
    readonly ObservableAsPropertyHelper<double> _height;

    public override double X
    {
        get => _x.Value;
        set => MutableSource.X = value;
    }
    readonly ObservableAsPropertyHelper<double> _x;

    public override double Y
    {
        get => _y.Value;
        set => MutableSource.Y = value;
    }
    readonly ObservableAsPropertyHelper<double> _y;

    protected override double TopBorderValue => _topBorder.Value;
    readonly ObservableAsPropertyHelper<double> _topBorder;

    protected override double BottomBorderValue => _bottomBorder.Value;
    readonly ObservableAsPropertyHelper<double> _bottomBorder;

    protected override double LeftBorderValue => _leftBorder.Value;
    readonly ObservableAsPropertyHelper<double> _leftBorder;

    protected override double RightBorderValue => _rightBorder.Value;
    readonly ObservableAsPropertyHelper<double> _rightBorder;
}
