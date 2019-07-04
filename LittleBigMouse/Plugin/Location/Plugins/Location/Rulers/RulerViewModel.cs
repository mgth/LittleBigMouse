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
using HLab.Notify.PropertyChanged;
using LittleBigMouse.ScreenConfigs;

namespace LittleBigMouse.Plugin.Location.Plugins.Location.Rulers
{
    public class RulerViewModel : N<RulerViewModel>
    {
        public Screen Screen { get; }
        public RulerSide Side { get; }
        public Screen DrawOn { get; }

        public RulerViewModel(Screen screen, Screen drawOn, RulerSide side)
        {
            Side = side;
            switch (side)
            {
                case RulerSide.Top:
                    Vertical = false;
                    Horizontal = true;
                    Revert = false;
                    break;
                case RulerSide.Bottom:
                    Vertical = false;
                    Horizontal = true;
                    Revert = true;
                    break;
                case RulerSide.Left:
                    Vertical = true;
                    Horizontal = false;
                    Revert = false;
                    break;
                case RulerSide.Right:
                    Vertical = true;
                    Horizontal = false;
                    Revert = true;
                    break;
                default:
                    throw new ArgumentOutOfRangeException(nameof(side), side, null);
            }
            Screen = screen;
            DrawOn = drawOn;
            Background = GetBackground(ReferenceEquals(DrawOn, Screen) ? Colors.DarkGreen : Colors.DarkBlue);
            BackgroundOut = GetBackground(Colors.Black);
            Initialize();
        }
        public enum RulerSide
        {
            Top,
            Bottom,
            Left,
            Right
        }

        public bool Enabled
        {
            get => _enable.Get();
            set => _enable.Set(value);
        }
        private readonly IProperty<bool> _enable = H.Property<bool>();

        public double ZeroX => _zeroX.Get();
        private readonly IProperty<double> _zeroX = H.Property<double>(c => c
            .On(e => e.DrawOn.XMoving)
            .On(e => e.Screen.XMoving)
            .Set(e => e.Screen.XMoving - e.DrawOn.XMoving)
        );

        public double ZeroY => _zeroY.Get();
        private readonly IProperty<double> _zeroY = H.Property<double>(c => c
            .On(e => e.DrawOn.YMoving)
            .On(e => e.Screen.YMoving)
            .Set(e => e.Screen.YMoving - e.DrawOn.YMoving)
        );

        //public Thickness Margin => _margin.Get();
        //private readonly IProperty<Thickness> _margin = H.Property<Thickness>( c => c
        //    .On(e => e.RatioY)
        //    .On(e => e.DrawOn.YMoving)
        //    .On(e => e.Screen.YMoving)
        //    .Set(e => new Thickness((!e.Horizontal) ? e.ZeroX : 0, e.Vertical ? e.ZeroY : 0, 0, 0))
        //);


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

        public Brush Background { get; }
        public Brush BackgroundOut { get; }
        private Brush GetBackground(Color color) 
        {

            var c = Color.Multiply(color,0.7f);

            switch (Side)
            {
                case RulerSide.Top:
                    return GetBrush(0.5, 0, 0.5, 1, c);
                case RulerSide.Bottom:
                    return GetBrush(0.5, 1, 0.5, 0, c);
                case RulerSide.Left:
                    return GetBrush(0, 0.5, 1, 0.5, c);
                case RulerSide.Right:
                    return GetBrush(1, 0.5, 0, 0.5, c);
                default:
                    throw new ArgumentOutOfRangeException();
            }
        }


        public bool Vertical { get; }
        public bool Horizontal { get; }
        public bool Revert { get; }

        public double RatioX => _ratioX.Get();
        private readonly IProperty<double> _ratioX = H.Property<double>( c => c
            .On(e => e.DrawOn.InDip.Width)
            .On(e => e.DrawOn.InMm.Width)
            .Set(e => e.DrawOn.InDip.Width / e.DrawOn.InMm.Width)
        );

        public double RatioY => _ratioY.Get();
        private readonly IProperty<double> _ratioY = H.Property<double>( c => c
            .On(e => e.DrawOn.InDip.Height)
            .On(e => e.DrawOn.InMm.Height)
            .Set(e => e.DrawOn.InDip.Height / e.DrawOn.InMm.Height)
        );

        //public double RulerHeight => _rulerHeight.Get();
        //private readonly IProperty<double> _rulerHeight = H.Property<double>(c => c
        //        .On(e => e.RatioY)
        //        .On(e => e.Screen.InMm.Height)
        //        .Set(e => e.Vertical ? (e.Screen.InMm.Height * e.RatioY) : double.NaN)
        //);

        //public double RulerWidth => _rulerWidth.Get();
        //private readonly IProperty<double> _rulerWidth = H.Property<double>( c => c
        //        .On(e => e.RatioX)
        //        .On(e => e.Screen.InMm.Width)
        //        .Set(e => e.Horizontal ? (e.Screen.InMm.Width * e.RatioX) : double.NaN)
        //    );

        public double RulerLength => _rulerLength.Get();
        private readonly IProperty<double> _rulerLength = H.Property<double>(c => c
                .On(e => e.Screen.InMm.Width)
                .On(e => e.Screen.InMm.Height)
                .Set(e => e.Vertical ? e.Screen.InMm.Height : e.Screen.InMm.Width)
            );

        public double RulerEnd => _rulerEnd.Get();
        private readonly IProperty<double> _rulerEnd = H.Property<double>(c => c
            .On(e => e.RulerLength)
            .On(e => e.RulerStart)
            .Set(e => e.RulerStart + e.RulerLength)
        );

        /// <summary>
        /// Calculate the first tick value of the ruler
        /// </summary>
        public double RulerStart => _rulerStart.Get();
        private readonly IProperty<double> _rulerStart = H.Property<double>(c => c
            .On(e => e.DrawOn.XMoving)
            .On(e => e.DrawOn.YMoving)
            .On(e => e.Screen.XMoving)
            .On(e => e.Screen.YMoving)
            .Set(e => e.Vertical ? e.DrawOn.YMoving - e.Screen.YMoving : e.DrawOn.XMoving - e.Screen.XMoving)
        );

        public double LengthRatio => _lengthRatio.Get();
        private readonly IProperty<double> _lengthRatio = H.Property<double>(c => c
            .On(e => e.RatioX)
            .On(e => e.RatioY)
            .Set(e => e.Vertical ? e.RatioY : e.RatioX)
        );

        public double SizeRatio => _sizeRatio.Get();
        private readonly IProperty<double> _sizeRatio = H.Property<double>(c => c
            .On(e => e.RatioX)
            .On(e => e.RatioY)
            .Set(e => e.Vertical ? e.RatioX : e.RatioY)
        );

        public double Zero => _zero.Get();
        private readonly IProperty<double> _zero = H.Property<double>(c => c
            .On(e => e.ZeroX)
            .On(e => e.ZeroY)
            .Set(e => e.Vertical ? e.ZeroY : e.ZeroX)
        );
    }
}
