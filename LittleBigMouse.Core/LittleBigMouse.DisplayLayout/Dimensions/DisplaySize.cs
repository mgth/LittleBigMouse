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

using System;
using System.Reactive.Concurrency;
using System.Runtime.Serialization;
using System.Text;
using System.Text.Json.Serialization;
using Avalonia;
using HLab.Base.Avalonia;
using ReactiveUI;

namespace LittleBigMouse.DisplayLayout.Dimensions;

[DataContract]
public abstract class DisplaySize : ReactiveModel, IDisplaySize
{
    protected DisplaySize(IDisplaySize source)
    {
        Source = source;
    }

    protected void Init()
    {
        _borders = this.WhenAnyValue(
                e => e.LeftBorder,
                e => e.TopBorder,
                e => e.RightBorder,
                e => e.BottomBorder,(left,top,right,bottom) => new Thickness(left, top, right, bottom))
            .Log(this).ToProperty(this, e => e.Borders, scheduler: Scheduler.Immediate);

        _location = this.WhenAnyValue(
                e => e.X,
                e => e.Y,
                (x,y) => new Point(x, y))
            .Log(this).ToProperty(this, e => e.Location, scheduler: Scheduler.Immediate);

        _size = this.WhenAnyValue(
                e => e.Width,
                e => e.Height,
                (x,y) => new Size(x, y))
            .Log(this).ToProperty(this, e => e.Size, scheduler: Scheduler.Immediate);

        _center = this.WhenAnyValue(
                e => e.Location,
                e => e.Size,
                (location,size) => location + new Vector(size.Width / 2, size.Height / 2))
            .Log(this).ToProperty(this, e => e.Center, scheduler: Scheduler.Immediate);

        _bounds = this.WhenAnyValue(
                e => e.Location,
                e => e.Size,
                (location,size) => new Rect(location, size))
            .Log(this).ToProperty(this, e => e.Bounds, scheduler: Scheduler.Immediate);

        _outsideX = this.WhenAnyValue(
                e => e.X,
                e => e.LeftBorder,
                (x,leftBorder) => x - leftBorder)
            .Log(this).ToProperty(this, e => e.OutsideX, scheduler: Scheduler.Immediate);

        _outsideY = this.WhenAnyValue(
                e => e.Y,
                e => e.TopBorder,
                (y,topBorder) => y - topBorder)
            .Log(this).ToProperty(this, e => e.OutsideY, scheduler: Scheduler.Immediate);

        _outsideWidth = this.WhenAnyValue(
                e => e.LeftBorder,
                e => e.Width,
                e => e.RightBorder,
                (leftBorder,width,rightBorder) => leftBorder + width + rightBorder)
            .Log(this).ToProperty(this, e => e.OutsideWidth, scheduler: Scheduler.Immediate);

        _outsideHeight = this.WhenAnyValue(
                e => e.TopBorder,
                e => e.Height,
                e => e.BottomBorder,
                (topBorder,height,bottomBorder) => topBorder + height + bottomBorder)
            .Log(this).ToProperty(this, e => e.OutsideHeight, scheduler: Scheduler.Immediate);

        _outsideBounds = this.WhenAnyValue(
                e => e.OutsideX,
                e => e.OutsideY,
                e => e.OutsideWidth,
                e => e.OutsideHeight,
                (x,y,width,height) => new Rect(new Point(x, y), new Size(width, height)))
            .Log(this).ToProperty(this, e => e.OutsideBounds, scheduler: Scheduler.Immediate);
    }

    [JsonIgnore]
    public IDisplaySize Source { get; }

    //[DataMember]
    public abstract double Width { get; set; }
    //[DataMember]
    public abstract double Height { get; set; }
    //[DataMember]
    public abstract double X { get; set; }
    //[DataMember]
    public abstract double Y { get; set; }
    //[DataMember]
    public abstract double TopBorder { get; set; }
    //[DataMember]
    public abstract double BottomBorder { get; set; }
    //[DataMember]
    public abstract double LeftBorder { get; set; }

    //[DataMember]
    public abstract double RightBorder { get; set; }


    [DataMember]
    public Thickness Borders => _borders.Value;
    ObservableAsPropertyHelper<Thickness> _borders;

    //[DataMember]
    public Point Location
    {
        get => _location.Value;
        set
        {
            X = value.X;
            Y = value.Y;
        }
    }
    ObservableAsPropertyHelper<Point> _location;

    public Size Size
    {
        get => _size.Value;
        set
        {
            Height = value.Height;
            Width = value.Width;
        }
    }
    ObservableAsPropertyHelper<Size> _size;

    public Point Center => _center.Value;
    ObservableAsPropertyHelper<Point> _center;

    [DataMember] public Rect Bounds => _bounds.Value;
    ObservableAsPropertyHelper<Rect> _bounds;

    //[DataMember]
    public double OutsideX
    {
        get => _outsideX.Value;
        set => X = value + LeftBorder;
    }
    ObservableAsPropertyHelper<double> _outsideX;

    //[DataMember]
    public double OutsideY
    {
        get => _outsideY.Value;
        set => Y = value + TopBorder;
    }
    ObservableAsPropertyHelper<double> _outsideY;

    public double OutsideWidth
    {
        get => _outsideWidth.Value;
        set => throw new NotImplementedException();
    }
    ObservableAsPropertyHelper<double> _outsideWidth;

    public double OutsideHeight
    {
        get => _outsideHeight.Value;
        set => throw new NotImplementedException();
    }
    ObservableAsPropertyHelper<double> _outsideHeight;

    [DataMember]
    public Rect OutsideBounds => _outsideBounds.Value;
    ObservableAsPropertyHelper<Rect> _outsideBounds;

    public bool Equals(IDisplaySize other)
    {
        if (other == null)
            return false;
        
        return X == other.X && Y == other.Y && Width == other.Width && Height == other.Height &&
               TopBorder == other.TopBorder && BottomBorder == other.BottomBorder &&
               LeftBorder == other.LeftBorder && RightBorder == other.RightBorder;
    }

    public virtual string TransformToString => string.Empty;

    public override string ToString()
    {
        var b = new StringBuilder();

        b.Append($"{Source?.ToString() ?? "()"} -> ");
        if (!string.IsNullOrEmpty(TransformToString)) b.Append($"{TransformToString} -> ");
        b.Append($"[{X},{Y}-({Width}x{Height}) B:{Borders}]");

        return b.ToString();
    }
}
