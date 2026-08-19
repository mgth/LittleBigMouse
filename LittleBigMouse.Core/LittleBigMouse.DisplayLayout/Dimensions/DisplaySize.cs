/*
  LittleBigMouse.DisplayLayout
  Copyright (c) 2021 Mathieu GRENET.  All right reserved.

  This file is part of LittleBigMouse.DisplayLayout.

    LittleBigMouse.DisplayLayout is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    LittleBigMouse.DisplayLayout is distributed in the hope that it will be useful,
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
using HLab.Base.ReactiveUI;
using HLab.Geo;
using ReactiveUI;

namespace LittleBigMouse.DisplayLayout.Dimensions;

[DataContract]
public abstract class DisplaySize(IDisplaySize source) : SavableReactiveModel, IDisplaySize
{
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
                (x,y,width,height) => new Rect(x, y, width, height))
//                (x,y,width,height) => new Rect(new Point(x,y), new Size(width, height)))
            .Log(this).ToProperty(this, e => e.OutsideBounds, scheduler: Scheduler.Immediate);
    }

    [JsonIgnore]
    public IDisplaySize Source { get; } = source;

    //[DataMember]
    public double Width => WidthValue;
    protected abstract double WidthValue { get; }
    //[DataMember]
    public double Height => HeightValue;
    protected abstract double HeightValue { get; }
    //[DataMember]
    public double X => XValue;
    protected abstract double XValue { get; }
    //[DataMember]
    public double Y => YValue;
    protected abstract double YValue { get; }
    //[DataMember]
    public double TopBorder => TopBorderValue;
    protected abstract double TopBorderValue { get; }
    //[DataMember]
    public double BottomBorder => BottomBorderValue;
    protected abstract double BottomBorderValue { get; }
    //[DataMember]
    public double LeftBorder => LeftBorderValue;
    protected abstract double LeftBorderValue { get; }

    //[DataMember]
    public double RightBorder => RightBorderValue;
    protected abstract double RightBorderValue { get; }


    [DataMember]
    public Thickness Borders => _borders.Value;
    ObservableAsPropertyHelper<Thickness> _borders;

    //[DataMember]
    public Point Location => _location.Value;
    ObservableAsPropertyHelper<Point> _location;

    public Size Size => _size.Value;
    ObservableAsPropertyHelper<Size> _size;

    public Point Center => _center.Value;
    ObservableAsPropertyHelper<Point> _center;

    [DataMember] public Rect Bounds => _bounds.Value;
    ObservableAsPropertyHelper<Rect> _bounds;

    //[DataMember]
    public double OutsideX => _outsideX.Value;
    ObservableAsPropertyHelper<double> _outsideX;

    //[DataMember]
    public double OutsideY => _outsideY.Value;
    ObservableAsPropertyHelper<double> _outsideY;

    public double OutsideWidth => _outsideWidth.Value;
    ObservableAsPropertyHelper<double> _outsideWidth;

    public double OutsideHeight => _outsideHeight.Value;
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

/// <summary>
/// Base class for dimensions that can propagate every edit, including borders, to
/// their source. Read-only computed dimensions derive directly from <see cref="DisplaySize"/>.
/// </summary>
public abstract class MutableDisplayBounds(IMutableDisplayBounds source)
    : DisplaySize(source), IMutableDisplayBounds
{
    [JsonIgnore]
    public new IMutableDisplayBounds Source => source;

    public new abstract double Width { get; set; }
    public new abstract double Height { get; set; }
    public new abstract double X { get; set; }
    public new abstract double Y { get; set; }

    public new Point Location
    {
        get => base.Location;
        set
        {
            X = value.X;
            Y = value.Y;
        }
    }

    public new Size Size
    {
        get => base.Size;
        set
        {
            Width = value.Width;
            Height = value.Height;
        }
    }

    public new double OutsideX
    {
        get => base.OutsideX;
        set => X = value + LeftBorder;
    }

    public new double OutsideY
    {
        get => base.OutsideY;
        set => Y = value + TopBorder;
    }

    protected sealed override double WidthValue => Width;
    protected sealed override double HeightValue => Height;
    protected sealed override double XValue => X;
    protected sealed override double YValue => Y;
}

/// <summary>
/// Base class for dimensions that can propagate every edit, including borders, to
/// their source.
/// </summary>
public abstract class MutableDisplaySize(IMutableDisplaySize source)
    : MutableDisplayBounds(source), IMutableDisplaySize
{
    [JsonIgnore]
    public new IMutableDisplaySize Source => source;

    public new abstract double TopBorder { get; set; }
    public new abstract double BottomBorder { get; set; }
    public new abstract double LeftBorder { get; set; }
    public new abstract double RightBorder { get; set; }

    protected sealed override double TopBorderValue => TopBorder;
    protected sealed override double BottomBorderValue => BottomBorder;
    protected sealed override double LeftBorderValue => LeftBorder;
    protected sealed override double RightBorderValue => RightBorder;
}
