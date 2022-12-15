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
using System.Runtime.Serialization;
using Avalonia;
using Newtonsoft.Json;
using ReactiveUI;

namespace LittleBigMouse.DisplayLayout.Dimensions;

[DataContract]
public abstract class DisplaySize : ReactiveObject, IDisplaySize
{
    protected DisplaySize(IDisplaySize source)
    {
        Source = source;

        this.WhenAnyValue(
                e => e.LeftBorder,
                e => e.RightBorder,
                e => e.TopBorder,
                e => e.BottomBorder,(l,r,t,b) => new Thickness(l, r, t, b))
            .ToProperty(this, e => e.Borders,out _borders);

        this.WhenAnyValue(
                e => e.X,
                e => e.Y,
                (x,y) => new Point(x, y))
            .ToProperty(this, e => e.Location,out _location);

        this.WhenAnyValue(
                e => e.Width,
                e => e.Height,
                (x,y) => new Size(x, y))
            .ToProperty(this, e => e.Size,out _size);

        this.WhenAnyValue(
                e => e.Location,
                e => e.Size,
                (l,s) => l + new Vector(s.Width / 2, s.Height / 2))
            .ToProperty(this, e => e.Center,out _center);

        this.WhenAnyValue(
                e => e.Location,
                e => e.Size,
                (l,s) => new Rect(l, s))
            .ToProperty(this, e => e.Bounds,out _bounds);

        this.WhenAnyValue(
                e => e.X,
                e => e.LeftBorder,
                (x,leftBorder) => x - leftBorder)
            .ToProperty(this, e => e.OutsideX,out _outsideX);

        this.WhenAnyValue(
                e => e.Y,
                e => e.TopBorder,
                (y,topBorder) => y - topBorder)
            .ToProperty(this, e => e.OutsideY,out _outsideY);

        this.WhenAnyValue(
                e => e.LeftBorder,
                e => e.Width,
                e => e.RightBorder,
                (leftBorder,width,rightBorder) => leftBorder + width + rightBorder)
            .ToProperty(this, e => e.OutsideWidth,out _outsideWidth);

        this.WhenAnyValue(
                e => e.TopBorder,
                e => e.Height,
                e => e.BottomBorder,
                (topBorder,height,bottomBorder) => topBorder + height + bottomBorder)
            .ToProperty(this, e => e.OutsideHeight,out _outsideHeight);

        this.WhenAnyValue(
                e => e.OutsideX,
                e => e.OutsideY,
                e => e.OutsideWidth,
                e => e.OutsideHeight,
                (x,y,width,height) => new Rect(new Point(x, y), new Size(width, height)))
            .ToProperty(this, e => e.OutsideBounds,out _outsideBounds);
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
    public Thickness Borders => _borders.Get();
    readonly ObservableAsPropertyHelper<Thickness> _borders;

    //[DataMember]
    public Point Location
    {
        get => _location.Get();
        set
        {
            X = value.X;
            Y = value.Y;
        }
    }
    readonly ObservableAsPropertyHelper<Point> _location;

    public Size Size
    {
        get => _size.Get();
        set
        {
            Height = value.Height;
            Width = value.Width;
        }
    }
    readonly ObservableAsPropertyHelper<Size> _size;

    public Point Center => _center.Get();
    readonly ObservableAsPropertyHelper<Point> _center;

    [DataMember] public Rect Bounds => _bounds.Get();
    readonly ObservableAsPropertyHelper<Rect> _bounds;

    //[DataMember]
    public double OutsideX
    {
        get => _outsideX.Get();
        set => X = value + LeftBorder;
    }
    readonly ObservableAsPropertyHelper<double> _outsideX;

    //[DataMember]
    public double OutsideY
    {
        get => _outsideY.Value;
        set => Y = value + TopBorder;
    }
    readonly ObservableAsPropertyHelper<double> _outsideY;

    public double OutsideWidth
    {
        get => _outsideWidth.Get();
        set => throw new NotImplementedException();
    }
    readonly ObservableAsPropertyHelper<double> _outsideWidth;

    public double OutsideHeight
    {
        get => _outsideHeight.Get();
        set => throw new NotImplementedException();
    }
    readonly ObservableAsPropertyHelper<double> _outsideHeight;

    [DataMember]
    public Rect OutsideBounds => _outsideBounds.Get();
    readonly ObservableAsPropertyHelper<Rect> _outsideBounds;

    public bool Equals(IDisplaySize other)
    {
        throw new NotImplementedException();
    }
}
