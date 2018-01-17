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

using System.Windows;
using HLab.Notify;
using Newtonsoft.Json;

namespace LittleBigMouse.ScreenConfigs
{
    public abstract class ScreenSize : NotifierObject
    {
        protected ScreenSize() : base(false) { }

        public ScreenSize Source
        {
            get => this.Get<ScreenSize>();
            protected set => this.Set(value);
        }

        public Point GetPoint(ScreenSize source, Point point)
        {
            var x = (point.X - source.X) / source.Width;
            var y = (point.Y - source.Y) / source.Height;

            return new Point(X + x*Width, Y+y*Height);
        }

        //[TriggedOn(nameof(Source),"Screen")]
        //public Screen Screen
        //{
        //    get => this.Get(()=>Source.Screen);
        //    protected set => this.Set(value);
        //}

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
        [TriggedOn(nameof(LeftBorder))]
        [TriggedOn(nameof(RightBorder))]
        [TriggedOn(nameof(TopBorder))]
        [TriggedOn(nameof(BottomBorder))]
        public Thickness Borders =>
            this.Get(() => new Thickness(LeftBorder, TopBorder, RightBorder, BottomBorder));

        //[JsonProperty]
        [TriggedOn(nameof(X))]
        [TriggedOn(nameof(Y))]
        public Point Location
        {
            get => this.Get(() => new Point(X, Y));
            set
            {
                using (this.Suspend())
                {
                    X = value.X;
                    Y = value.Y;
                }
            }
        }
        //[JsonProperty]
        [TriggedOn(nameof(Width))]
        [TriggedOn(nameof(Height))]
        public Size Size
        {
            get => this.Get(() => new Size(Width, Height));
            set
            {
                using (this.Suspend())
                {
                    Height = value.Height;
                    Width = value.Width;
                }
            }
        }

        [JsonProperty]
        [TriggedOn(nameof(Size))]
        [TriggedOn(nameof(Location))]
        public Rect Bounds => this.Get(() => new Rect(
            Location,
            Size));

        //[JsonProperty]
        [TriggedOn(nameof(LeftBorder))]
        [TriggedOn(nameof(RightBorder))]
        [TriggedOn(nameof(Width))]
        public double OutsideWidth => this.Get(() => Width + LeftBorder + RightBorder);

        //[JsonProperty]
        [TriggedOn(nameof(TopBorder))]
        [TriggedOn(nameof(BottomBorder))]
        [TriggedOn(nameof(Height))]
        public double OutsideHeight => this.Get(() => Height + TopBorder + BottomBorder);

        //[JsonProperty]
        [TriggedOn(nameof(X))]
        [TriggedOn(nameof(LeftBorder))]
        public double OutsideX
        {
            get => this.Get(() => X - LeftBorder);
            set => X = value + LeftBorder;
        }

        //[JsonProperty]
        [TriggedOn(nameof(Y))]
        [TriggedOn(nameof(TopBorder))]
        public double OutsideY
        {
            get => this.Get(() => Y - TopBorder);
            set => Y = value + TopBorder;
        }

        public Point Inside(Point p)
        {
            var x = p.X < X ? X : (p.X > Bounds.Right - 1) ? (Bounds.Right - 1) : p.X;
            var y = p.Y < Y ? Y : (p.Y > Bounds.Bottom - 1) ? (Bounds.Bottom - 1) : p.Y;

            return new Point(x,y);
        }

        [JsonProperty]
        [TriggedOn(nameof(OutsideX))]
        [TriggedOn(nameof(OutsideY))]
        [TriggedOn(nameof(OutsideWidth))]
        [TriggedOn(nameof(OutsideHeight))]
        public Rect OutsideBounds => this.Get(() => new Rect(new Point(OutsideX, OutsideY), new Size(OutsideWidth, OutsideHeight)));
    }
}
