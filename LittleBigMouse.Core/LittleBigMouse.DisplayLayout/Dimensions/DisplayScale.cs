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


using ReactiveUI;
using System.Reactive.Concurrency;

namespace LittleBigMouse.DisplayLayout.Dimensions;

public class DisplayScale : DisplaySize
{
    public DisplayScale(IDisplaySize source, IDisplayRatio ratio) : base(source)
    {
        Ratio = ratio;

        _x = this.WhenAnyValue(e => e.Source.X)
            .ToProperty(this, e => e.X);

        _y = this.WhenAnyValue(e => e.Source.Y)
            .ToProperty(this, e => e.Y);

        _width = this.WhenAnyValue(
                e => e.Source.Width,
                e => e.Ratio.X,

                (width,r) => width*r
            )
            .ToProperty(this, e => e.Width, scheduler: Scheduler.Immediate);

        _leftBorder = this.WhenAnyValue(
            e => e.Source.LeftBorder, 
            e => e.Ratio.X, 
            (width, r) => width * r)
            .ToProperty(this, e => e.LeftBorder, scheduler: Scheduler.Immediate);

        _rightBorder = this.WhenAnyValue(
                e => e.Source.RightBorder,
                e => e.Ratio.X,

                (width,r) => width*r
            )
            .ToProperty(this, e => e.RightBorder, scheduler: Scheduler.Immediate);

       _height = this.WhenAnyValue(
                e => e.Source.Height,
                e => e.Ratio.Y,

                (height,r) => height*r
            )
            .ToProperty(this, e => e.Height, scheduler: Scheduler.Immediate);

        _topBorder = this.WhenAnyValue(
                e => e.Source.TopBorder,
                e => e.Ratio.Y,

                (height,r) => height*r
            )
            .ToProperty(this, e => e.TopBorder, scheduler: Scheduler.Immediate);

        _bottomBorder = this.WhenAnyValue(
                e => e.Source.BottomBorder,
                e => e.Ratio.Y,

                (height,r) => height*r
            )
            .ToProperty(this, e => e.BottomBorder, scheduler: Scheduler.Immediate);

        base.Init();
    }

    public IDisplayRatio Ratio { get; }

    public override double Width
    {
        get => _width?.Value ?? 0;
        set => Source.Width = value / Ratio.X;
    }
    readonly ObservableAsPropertyHelper<double> _width;

    public override double Height
    {
        get => _height?.Value ?? 0;
        set => Source.Height = value / Ratio.Y;
    }
    readonly ObservableAsPropertyHelper<double> _height;

    public override double X
    {
        get => _x?.Value ?? 0;
        set => Source.X = value;
    }
    readonly ObservableAsPropertyHelper<double> _x;

    public override double Y
    {
        get => _y?.Value ?? 0;
        set => Source.Y = value;
    }
    readonly ObservableAsPropertyHelper<double> _y;

    public override double TopBorder
    {
        get => _topBorder?.Value??0;
        set => Source.TopBorder = value / Ratio.Y;
    }
    readonly ObservableAsPropertyHelper<double> _topBorder;

    public override double BottomBorder
    {
        get => _bottomBorder?.Value??0;
        set => Source.BottomBorder = value / Ratio.Y;
    }
    readonly ObservableAsPropertyHelper<double> _bottomBorder;

    public override double LeftBorder
    {
        get => _leftBorder?.Value ?? 0;
        set => Source.LeftBorder = value / Ratio.X;
    }
    readonly ObservableAsPropertyHelper<double> _leftBorder;

    public override double RightBorder
    {
        get => _rightBorder?.Value??0;
        set => Source.RightBorder = value / Ratio.X;
    }
    readonly ObservableAsPropertyHelper<double> _rightBorder;

    public override string TransformToString => $"Scale:{Ratio}";

}
