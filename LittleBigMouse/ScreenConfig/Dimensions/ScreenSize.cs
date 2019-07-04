/*
  LittleBigMouse.Screen.Config
  Copyright (c) 2017 Mathieu GRENET.  All right reserved.

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
using System.Windows;
using HLab.Base;
using HLab.Notify.Annotations;
using HLab.Notify.PropertyChanged;
using Newtonsoft.Json;

namespace LittleBigMouse.ScreenConfigs
{
    public interface IScreenSize : IEquatable<IScreenSize>
    {
        double Width { get; set; }
        double Height { get; set; }
        double X { get; set; }
        double Y { get; set; }
        double TopBorder { get; set; }
        double BottomBorder { get; set; }
        double LeftBorder { get; set; }
        double RightBorder { get; set; }

        Rect Bounds { get; }
        Point Center { get; }

        Rect OutsideBounds { get; }
        double OutsideWidth { get; }
        double OutsideHeight { get; }
        double OutsideX { get; }
        double OutsideY { get; }

        Point Location { get; }
    }

    public static class ScreenSizeExtensions
    {
        public static Point GetPoint(this IScreenSize sz, IScreenSize source, Point point)
        {
            var x = (point.X - source.X) / source.Width;
            var y = (point.Y - source.Y) / source.Height;

            return new Point(sz.X + x * sz.Width, sz.Y + y * sz.Height);
        }
        public static Point Inside(this IScreenSize sz, Point p)
        {
            var x = p.X < sz.X ? sz.X : (p.X > sz.Bounds.Right - 1) ? (sz.Bounds.Right - 1) : p.X;
            var y = p.Y < sz.Y ? sz.Y : (p.Y > sz.Bounds.Bottom - 1) ? (sz.Bounds.Bottom - 1) : p.Y;

            return new Point(x, y);
        }
    }

    public abstract class ScreenSize<TClass> : N<TClass>, IScreenSize
    where TClass : ScreenSize<TClass>
    {
        protected ScreenSize(IScreenSize source)
        {
            Source = source;
        }
        public IScreenSize Source { get; }

        //[JsonProperty]
        public abstract double Width { get; set; }
        //[JsonProperty]
        public abstract double Height { get; set; }
        //[JsonProperty]
        public abstract double X { get; set; }
        //[JsonProperty]
        public abstract double Y { get; set; }
        //[JsonProperty]
        public abstract double TopBorder { get; set; }
        //[JsonProperty]
        public abstract double BottomBorder { get; set; }
        //[JsonProperty]
        public abstract double LeftBorder { get; set; }

        //[JsonProperty]
        public abstract double RightBorder { get; set; }


        [JsonProperty]
        public Thickness Borders => _borders.Get();
        private readonly IProperty<Thickness> _borders = H.Property<Thickness>(c => c
            .On(nameof(LeftBorder))
            .On(nameof(RightBorder))
            .On(nameof(TopBorder))
            .On(nameof(BottomBorder))
            .Set(e => new Thickness(e.LeftBorder, e.TopBorder, e.RightBorder, e.BottomBorder))
        );
        private readonly IProperty<Point> _location = H.Property<Point>(c=>c
            .On(nameof(X))
            .On(nameof(Y))
            .Set(e => new Point(e.X, e.Y))
        );
        //[JsonProperty]
        public Point Location
        {
            get => _location.Get();
            set
            {
                    X = value.X;
                    Y = value.Y;
            }
        }


        private readonly IProperty<Size> _size = H.Property<Size>(c => c
            .On(nameof(Width))
            .On(nameof(Height))
            .Set(e => new Size(e.Width,e.Height))            
        );
        public Size Size
        {
            get => _size.Get();
            set
            {
                    Height = value.Height;
                    Width = value.Width;
            }
        }

        [TriggerOn(nameof(Location))]
        [TriggerOn(nameof(Size))]
        public Point Center => Location + new Vector(Width / 2, Height / 2);


        [JsonProperty]
        [TriggerOn(nameof(Size))]
        [TriggerOn(nameof(Location))]
        public Rect Bounds => new Rect(
            Location,
            Size);

        //[JsonProperty]



        //[JsonProperty]
        [TriggerOn(nameof(X))]
        [TriggerOn(nameof(LeftBorder))]
        public double OutsideX
        {
            get => X - LeftBorder;
            set => X = value + LeftBorder;
        }

        //[JsonProperty]
        [TriggerOn(nameof(Y))]
        [TriggerOn(nameof(TopBorder))]
       public double OutsideY
        {
            get => Y - TopBorder;
            set => Y = value + TopBorder;
        }

       public double OutsideWidth
       {
           get => _outsideWidth.Get();
           set => throw new NotImplementedException();
       }
       private readonly IProperty<double> _outsideWidth = H.Property<double>(c => c
            //.TriggerOn(n => n.Width).TriggerOn(n => n.LeftBorder).TriggerOn(n => n.LeftBorder)
           .On(e => e.Width).On(e=>e.LeftBorder).On(e => e.LeftBorder)
           .Set(e => e.Width + e.LeftBorder + e.RightBorder));


       public double OutsideHeight
       {
           get => _outsideHeight.Get();
           set => throw new NotImplementedException();
       }
       private readonly IProperty<double> _outsideHeight = H.Property<double>(c => c
            //.TriggerOn(n => n.Height).TriggerOn(n => n.TopBorder).TriggerOn(n => n.BottomBorder)
            .On(e => e.Height).On(e => e.TopBorder).On(e => e.BottomBorder)
           .Set(e => e.Height + e.TopBorder + e.BottomBorder));


        public bool Equals(IScreenSize other)
        {
            throw new NotImplementedException();
        }

        [JsonProperty]
        [TriggerOn(nameof(OutsideX))]
        [TriggerOn(nameof(OutsideY))]
        [TriggerOn(nameof(OutsideWidth))]
        [TriggerOn(nameof(OutsideHeight))]
        public Rect OutsideBounds => new Rect(new Point(OutsideX, OutsideY), new Size(OutsideWidth, OutsideHeight));
    }
}
