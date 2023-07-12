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

using Avalonia;
using ReactiveUI;
using System.Reactive.Concurrency;

namespace LittleBigMouse.DisplayLayout.Dimensions;

public class DisplayRotate : DisplaySize
{
    public int Rotation { get; }

    public DisplayRotate(IDisplaySize source, int rotation = 0) : base(source)
    {
        Rotation = rotation;

        _x = this.WhenAnyValue(e => e.Source.X)
            .ToProperty(this, e => e.X);

        _y = this.WhenAnyValue(e => e.Source.Y)
            .ToProperty(this, e => e.Y);

        _width = this.WhenAnyValue(
                e => e.Source.Width,
                e => e.Source.Height,
                e => e.Rotation,
                (width,height,r) => r % 2 == 0 ? width : height
                )
            .ToProperty(this, e => e.Width, scheduler: Scheduler.Immediate);

        _height = this.WhenAnyValue(
                e => e.Source.Width,
                e => e.Source.Height,
                e => e.Rotation,
                (width,height,r) => r % 2 == 1 ? width : height
                )
            .ToProperty(this, e => e.Height, scheduler: Scheduler.Immediate);

        //readonly IProperty<double> _topBorder = H.Property<double>(c => c
        //    .Set(e => e.GetBorder(0))
        //    .On(e => e.Rotation)
        //    .On(e => e.Source.TopBorder)
        //    .On(e => e.Source.RightBorder)
        //    .On(e => e.Source.BottomBorder)
        //    .On(e => e.Source.LeftBorder)
        //    .Update()
        //);


        _topBorder = this.WhenAnyValue(
                e => e.Rotation,
                e => e.Source.LeftBorder,
                e => e.Source.TopBorder,
                e => e.Source.RightBorder,
                e => e.Source.BottomBorder,

                (r,left,top,right,bottom) => GetBorder(0,r,left,top,right,bottom)
                )
            .ToProperty(this, e => e.TopBorder, scheduler: Scheduler.Immediate);

        _rightBorder = this.WhenAnyValue(
                e => e.Rotation,
                e => e.Source.LeftBorder,
                e => e.Source.TopBorder,
                e => e.Source.RightBorder,
                e => e.Source.BottomBorder,

                (r,left,top,right,bottom) => GetBorder(1,r,left,top,right,bottom)
                )
            .ToProperty(this, e => e.RightBorder, scheduler: Scheduler.Immediate);

        _bottomBorder = this.WhenAnyValue(
                e => e.Rotation,
                e => e.Source.LeftBorder,
                e => e.Source.TopBorder,
                e => e.Source.RightBorder,
                e => e.Source.BottomBorder,

                (r,left,top,right,bottom) => GetBorder(2,r,left,top,right,bottom)
                )
            .ToProperty(this, e => e.BottomBorder, scheduler: Scheduler.Immediate);

        _leftBorder = this.WhenAnyValue(
                e => e.Rotation,
                e => e.Source.LeftBorder,
                e => e.Source.TopBorder,
                e => e.Source.RightBorder,
                e => e.Source.BottomBorder,

                (r,left,top,right,bottom) => GetBorder(3,r,left,top,right,bottom)
            )
            .ToProperty(this, e => e.LeftBorder, scheduler: Scheduler.Immediate);

        base.Init();
    }

    public Vector Translation
    {
        get => _translation;
        set => this.RaiseAndSetIfChanged(ref _translation, value);
    }
    Vector _translation;

    public override double Width
    {
        get => _width?.Value ?? 0;
        set
        {
            switch (Rotation % 2)
            {
                case 0:
                    Source.Width = value;
                    break;
                case 1:
                    Source.Height = value;
                    break;
            }
        }
    }

    readonly ObservableAsPropertyHelper<double> _width;

    public override double Height
    {
        get => _height?.Value ?? 0;
        set
        {
            switch (Rotation % 2)
            {
                case 0:
                    Source.Height = value;
                    break;
                case 1:
                    Source.Width = value;
                    break;
            }
        }
    }

    ObservableAsPropertyHelper<double> _height;

    public override double X
    {
        get => _x.Value;
        set => Source.X = value;
    }

    readonly ObservableAsPropertyHelper<double> _x;

    public override double Y
    {
        get => _y.Value;
        set => Source.Y = value;
    }

    readonly ObservableAsPropertyHelper<double> _y;

    static double GetBorder(int border, int rotation, double left, double top, double right, double bottom)
    {
        return ((border + rotation) % 4) switch
        {
            0 => top,
            1 => right,
            2 => bottom,
            3 => left,
            _ => -1,
        };
    }

    void SetBorder(int border, double value)
    {
        using (DelayChangeNotifications())
        {
            switch ((border + Rotation) % 4)
            {
                case 0:
                    Source.TopBorder = value;
                    break;
                case 1:
                    Source.RightBorder = value;
                    break;
                case 2:
                    Source.BottomBorder = value;
                    break;
                case 3:
                    Source.LeftBorder = value;
                    break;
            }
        }
    }



    public override double TopBorder
    {
        get => _topBorder.Value;
        set => SetBorder(0, value);
    }

    readonly ObservableAsPropertyHelper<double> _topBorder;

    public override double RightBorder
    {
        get => _rightBorder.Value;
        set => SetBorder(1, value);
    }

    readonly ObservableAsPropertyHelper<double> _rightBorder;

    public override double BottomBorder
    {
        get => _bottomBorder.Value;
        set => SetBorder(2, value);
    }

    readonly ObservableAsPropertyHelper<double> _bottomBorder;

    public override double LeftBorder
    {
        get => _leftBorder.Value;
        set => SetBorder(3, value);
    }

    readonly ObservableAsPropertyHelper<double> _leftBorder;

    public override string TransformToString => $"Rotate:{Rotation}";

}
