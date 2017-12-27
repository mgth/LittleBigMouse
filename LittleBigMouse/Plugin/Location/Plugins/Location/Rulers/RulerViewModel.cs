/*
  LittleBigMouse.Plugin.Location
  Copyright (c) 2017 Mathieu GRENET.  All right reserved.

  This file is part of LittleBigMouse.Plugin.Location.

    LittleBigMouse.Plugin.Location is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    LittleBigMouse.Plugin.Location is distributed in the hope that it will be useful,
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
using System.Windows.Media;
using HLab.Mvvm;
using HLab.Notify;
using LittleBigMouse.ScreenConfigs;

namespace LittleBigMouse.LocationPlugin.Plugins.Location.Rulers
{
    public class RulerViewModel : ViewModel
    {
        public RulerViewModel(Screen screen, Screen drawOn, RulerSide side)
        {
            this.SubscribeNotifier();
            using (this.Suspend())
            {
                Side = side;
                Screen = screen;
                DrawOn = drawOn;                
            }
        }
        public enum RulerSide
        {
            Top,
            Bottom,
            Left,
            Right
        }
        public RulerSide Side
        {
            get => this.Get<RulerSide>(); set => this.Set(value);
        }
        public bool Enabled
        {
            get => this.Get<bool>();
            set => this.Set(value);
        }
        public Screen DrawOn
        {
            get => this.Get<Screen>();
            set => this.Set(value);
        }
        public Screen Screen
        {
            get => this.Get<Screen>();
            set => this.Set(value);
        }

        [TriggedOn(nameof(RatioX))]
        [TriggedOn(nameof(DrawOn), "XMoving")]
        [TriggedOn(nameof(Screen), "XMoving")]
        public double ZeroX => this.Get(() //=> 0);
          => RatioX * (Screen.XMoving - DrawOn.XMoving));

        [TriggedOn(nameof(RatioY))]
        [TriggedOn(nameof(DrawOn), "YMoving")]
        [TriggedOn(nameof(Screen), "YMoving")]
        public double ZeroY => this.Get(()//=> 0);
             => RatioY * (Screen.YMoving - DrawOn.YMoving));



        [TriggedOn(nameof(ZeroY))]
        [TriggedOn(nameof(ZeroX))]
        public Thickness Margin => this.Get<Thickness>(()
                
            => Vertical ? new Thickness( 0, ZeroY, 0, 0 ) : new Thickness( ZeroX, 0, 0, 0 ));


        public Brush GetBrush(double x1, double y1, double x2, double y2, Color c1)
        {
            Color c2 = Color.FromScRgb(0, c1.ScR/3, c1.ScG/3, c1.ScB/3);

            c2.A = 0;

            return new LinearGradientBrush
            {
                StartPoint = new Point(x1,y1),
                EndPoint = new Point(x2,y2),
                GradientStops =
                {
                    new GradientStop(c1, 0),
                    new GradientStop(c1, 0.3),
                    new GradientStop(c2, 1),
                }
            };
        }


        [TriggedOn(nameof(Side))]
        public Brush Background => this.Get(() =>
        {
            var c = ReferenceEquals(DrawOn, Screen) ? Colors.DarkGreen : Colors.DarkBlue;

            c = Color.Multiply(c,0.7f);

            switch (Side)
            {
                case RulerSide.Top: return GetBrush(0.5, 0, 0.5, 1, c);
                case RulerSide.Bottom: return GetBrush(0.5, 1, 0.5, 0, c);
                case RulerSide.Left: return GetBrush(0, 0.5, 1, 0.5, c);
                case RulerSide.Right: return GetBrush(1, 0.5, 0, 0.5, c);
                default:
                    throw new ArgumentOutOfRangeException();
            }
        });

        [TriggedOn(nameof(Side))]
        public Brush BackgroundOut => this.Get(() =>
        {
            var c = Color.Multiply(Colors.Black,0.7f);

            switch (Side)
            {
                case RulerSide.Top: return GetBrush(0.5, 0, 0.5, 1, c);
                case RulerSide.Bottom: return GetBrush(0.5, 1, 0.5, 0, c);
                case RulerSide.Left: return GetBrush(0, 0.5, 1, 0.5, c);
                case RulerSide.Right: return GetBrush(1, 0.5, 0, 0.5, c);
                default:
                    throw new ArgumentOutOfRangeException();
            }
        });

        [TriggedOn(nameof(Side))]
        public bool Vertical => this.Get(()=>Side == RulerSide.Left) || (Side == RulerSide.Right);

        [TriggedOn(nameof(Vertical))]
        public bool Horizontal => this.Get(()=>!Vertical);

        [TriggedOn(nameof(Side))]
        public bool Revert => this.Get(() => Side == RulerSide.Right || Side == RulerSide.Bottom);


        [TriggedOn(nameof(DrawOn), "InDip", "Height")]
        [TriggedOn(nameof(DrawOn), "InMm", "Height")]
        public double RatioY => this.Get(() => DrawOn.InDip.Height / DrawOn.InMm.Height);

        [TriggedOn(nameof(DrawOn), "InDip", "Height")]
        [TriggedOn(nameof(DrawOn), "InMm", "Height")]
        public double RatioX => this.Get(() => DrawOn.InDip.Width / DrawOn.InMm.Width);

        [TriggedOn(nameof(Vertical))]
        [TriggedOn(nameof(RatioY))]
        [TriggedOn(nameof(Screen), "InMm", "Height")]
        //public double RulerHeight => this.Get(() => DrawOn.MmToDipRatio.Y * (Vertical ? Screen.InMm.Height : double.NaN));
        public double RulerHeight => this.Get(() => Vertical ? (Screen.InMm.Height * RatioY) : double.NaN);


        [TriggedOn(nameof(Horizontal))]
        [TriggedOn(nameof(RatioX))]
        [TriggedOn(nameof(Screen),"InMm","Width")]
        public double RulerWidth => this.Get(() => Horizontal ? Screen.InMm.Width * RatioX : double.NaN);

        [TriggedOn(nameof(Vertical))]
        [TriggedOn(nameof(Screen),"InMm","Width")]
        [TriggedOn(nameof(Screen),"InMm","Height")]
        public double RulerLength => this.Get(()=> Vertical ? Screen.InMm.Height : Screen.InMm.Width);

        [TriggedOn(nameof(Vertical))]
        [TriggedOn(nameof(DrawOn),"InMm","Height")]
        [TriggedOn(nameof(DrawOn),"InMm","Width")]
        [TriggedOn(nameof(RulerStart))]
        public double RulerEnd => this.Get(()=> RulerStart + (Vertical ? DrawOn.InMm.Height : DrawOn.InMm.Width));

        [TriggedOn(nameof(Vertical))]
        [TriggedOn(nameof(DrawOn),"XMoving")]
        [TriggedOn(nameof(DrawOn),"YMoving")]
        [TriggedOn(nameof(Screen),"XMoving")]
        [TriggedOn(nameof(Screen),"YMoving")]
        public double RulerStart => this.Get(()=> Vertical? DrawOn.YMoving - Screen.YMoving : DrawOn.XMoving - Screen.XMoving);

        [TriggedOn(nameof(Vertical))]
        [TriggedOn(nameof(RatioX))]
        [TriggedOn(nameof(RatioY))]
        public double LengthRatio => this.Get(() => Vertical ? RatioY : RatioX);

        [TriggedOn(nameof(Vertical))]
        [TriggedOn(nameof(RatioX))]
        [TriggedOn(nameof(RatioY))]
        public double SizeRatio => this.Get(() => Vertical ? RatioX : RatioY);

        [TriggedOn(nameof(Vertical))]
        [TriggedOn(nameof(ZeroX))]
        [TriggedOn(nameof(ZeroY))]
        public double Zero => this.Get(() =>  Vertical ? ZeroY : ZeroX);

    }
}
