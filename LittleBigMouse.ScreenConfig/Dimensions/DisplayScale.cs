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

namespace LittleBigMouse.DisplayLayout.Dimensions;

public class DisplayScale : DisplaySize
{
    public DisplayScale(IDisplaySize source, IDisplayRatio ratio) : base(source)
    {
        Ratio = ratio;

        this.WhenAnyValue(e => e.Source.X)
            .ToProperty(this, e => e.X,out _x);

        this.WhenAnyValue(e => e.Source.Y)
            .ToProperty(this, e => e.Y,out _y);

        this.WhenAnyValue(
                e => e.Source.Width,
                e => e.Ratio.X,

                (width,r) => width*r
            )
            .ToProperty(this, e => e.Width,out _width);

        this.WhenAnyValue(
                e => e.Source.LeftBorder,
                e => e.Ratio.X,

                (width,r) => width*r
            )
            .ToProperty(this, e => e.LeftBorder,out _leftBorder);

        this.WhenAnyValue(
                e => e.Source.RightBorder,
                e => e.Ratio.X,

                (width,r) => width*r
            )
            .ToProperty(this, e => e.RightBorder,out _rightBorder);

        this.WhenAnyValue(
                e => e.Source.Height,
                e => e.Ratio.Y,

                (height,r) => height*r
            )
            .ToProperty(this, e => e.Height,out _height);

        this.WhenAnyValue(
                e => e.Source.TopBorder,
                e => e.Ratio.Y,

                (height,r) => height*r
            )
            .ToProperty(this, e => e.TopBorder,out _topBorder);

        this.WhenAnyValue(
                e => e.Source.BottomBorder,
                e => e.Ratio.Y,

                (height,r) => height*r
            )
            .ToProperty(this, e => e.BottomBorder,out _bottomBorder);
    }

    public IDisplayRatio Ratio { get; }

    public override double Width
    {
        get => _width.Get();
        set => Source.Width = value / Ratio.X;
    }
    readonly ObservableAsPropertyHelper<double> _width;

    public override double Height
    {
        get => _height.Get();
        set => Source.Height = value / Ratio.Y;
    }
    readonly ObservableAsPropertyHelper<double> _height;

    public override double X
    {
        get => _x.Get();
        set => Source.X = value;
    }

    readonly ObservableAsPropertyHelper<double> _x;

    public override double Y
    {
        get => _y.Get();
        set => Source.Y = value;
    }
    readonly ObservableAsPropertyHelper<double> _y;

    public override double TopBorder
    {
        get => _topBorder.Get();
        set => Source.TopBorder = value / Ratio.Y;
    }
    readonly ObservableAsPropertyHelper<double> _topBorder;

    public override double BottomBorder
    {
        get => _bottomBorder.Get();
        set => Source.BottomBorder = value / Ratio.Y;
    }
    readonly ObservableAsPropertyHelper<double> _bottomBorder;

    public override double LeftBorder
    {
        get => _leftBorder.Get();
        set => Source.LeftBorder = value / Ratio.X;
    }
    readonly ObservableAsPropertyHelper<double> _leftBorder;

    public override double RightBorder
    {
        get => _rightBorder.Get();
        set => Source.RightBorder = value / Ratio.X;
    }
    readonly ObservableAsPropertyHelper<double> _rightBorder;

}
