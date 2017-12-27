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
using System.ComponentModel;
using System.Globalization;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Media;

namespace LittleBigMouse.LocationPlugin.Plugins.Location.Rulers
{
    /// <summary>
    /// Logique d'interaction pour Sizer.xaml
    /// </summary>
    /// 


 
    public partial class RulerViewTop : UserControl
    {
        public RulerViewTop()
        {
            InitializeComponent();
            DataContextChanged += Ruler_DataContextChanged;
        }

        private void Ruler_DataContextChanged(object sender, DependencyPropertyChangedEventArgs e)
        {
            if (e.OldValue is INotifyPropertyChanged oldVm) oldVm.PropertyChanged -= ViewModel_PropertyChanged;
            if (e.NewValue is INotifyPropertyChanged newVm)
            {
                newVm.PropertyChanged += ViewModel_PropertyChanged;
                InvalidateVisual();
            }
        }


        private void ViewModel_PropertyChanged(object sender, PropertyChangedEventArgs e)
        {
            switch (e.PropertyName)
            {
                case "LengthRatio":
                case "SizeRatio":
                case "RulerLength":
                case "RulerStart":
                case "RulerEnd":
                case "Revert":
                case "Zero":
                        InvalidateVisual();
                     break;
                default:
                    return;
            }
        }

        public RulerViewModel ViewModel => DataContext as RulerViewModel;


        protected override void OnRender(DrawingContext dx)
        {
            base.OnRender(dx);

            if (ViewModel == null) return;

            var vertical = ViewModel.Vertical;

            var lengthRatio = ViewModel.LengthRatio;
            var sizeRatio = ViewModel.SizeRatio;
            var rulerInLength = ViewModel.RulerLength;

            var rulerStart = ViewModel.RulerStart;
            var rulerEnd = ViewModel.RulerEnd;

            var revert = ViewModel.Revert;
            var zero = ViewModel.Zero;

            var pixelsPerDip = 96 / ViewModel.Screen.RealDpiAvg;

            var rwidth = (vertical?ViewModel.RatioX:ViewModel.RatioY) * 30;
            var penIn = new Pen(Brushes.WhiteSmoke,1);
            var penOut = new Pen(new SolidColorBrush(Color.FromScRgb(0.7f, 0.7f, 0.7f, 0.7f)),1);


            //   neg     0 actual ruler    L    outside positive
            // |---------|-----------------|----------|
            // r0        r1                r2         r3

            var r0 = 0;
            var r1 = zero;
            var r2 = zero + rulerInLength * lengthRatio;
            var r3 = zero + rulerEnd * lengthRatio;
            if (vertical)
            {
                if(r0<r3 && r1>r0)
                    dx.DrawRectangle(ViewModel.BackgroundOut, null,new Rect(new Point(0,0),new Point(rwidth, Math.Min(r1,r3))));

                if(r2<r3)
                    dx.DrawRectangle(ViewModel.BackgroundOut, null,new Rect(new Point(0, r2),new Point(rwidth, r3)));

                if(r1<r3 && r2>r0)
                    dx.DrawRectangle(ViewModel.Background, null,new Rect(new Point(0,Math.Max(r1,r0)),new Point(rwidth,Math.Min(r2,r3))));
            }
            else
            {
                if (r0 < r3 && r1 > r0)
                    dx.DrawRectangle(ViewModel.BackgroundOut, null, new Rect(new Point(0, 0), new Point(Math.Min(r1, r3), rwidth)));

                if (r2 < r3)
                    dx.DrawRectangle(ViewModel.BackgroundOut, null, new Rect(new Point(r2, 0), new Point(r3, rwidth)));

                if (r1 < r3 && r2 > r0)
                    dx.DrawRectangle(ViewModel.Background, null,new Rect(new Point(zero,0),new Point(Math.Min(r2, r3), rwidth)));
            }

            var mm = (int)rulerStart;
            while (mm < (int)rulerEnd)
            {
                var pos = zero + mm * lengthRatio;

                var size = sizeRatio;
                string text = null;

                var pen = (mm < 0 || mm > rulerInLength) ? penOut : penIn;

                 //if (mm >= rulerLength + 1)
                //{
                //    pos = rulerLength * lengthRatio;
                //    size *= 30.0;
                //    pen = penRed;
                //}
                //else if (mm >= rulerLength)
                //{
                //    pos = rulerLength * lengthRatio;
                //    size *= 30.0;
                //    pen = penRed;
                //}
                //else if (mm == 0)
                //{
                //    size *= 30.0;
                //    pen = penRed;
                //}
                //else 
                if (mm % 100 == 0)
                {
                    size *= 20.0;
                    text = (mm / 100).ToString();
                }
                else if (mm % 50 == 0)
                {
                    size *= 15.0;
                    text = "5";
                }
                else if (mm % 10 == 0)
                {
                    size *= 10.0;
                    text = ((mm % 100) / 10).ToString();
                }
                else if (mm % 5 == 0)
                {
                    size *= 5.0;
                }
                else
                {
                    size *= 2.5;
                }

                if (text != null)
                {
                    var t = new FormattedText(text,CultureInfo.CurrentCulture, FlowDirection.LeftToRight, new Typeface("Segoe UI"), size / 3, pen.Brush ,pixelsPerDip);
                        

                    var p2 = (!revert) ? (size - size / 3) : (rwidth - size);

                    dx.DrawText(t, vertical ? new Point(p2, pos) : new Point(pos, p2));
                }

                var x0 = /*x +*/ (vertical ? ((revert) ? rwidth - size : 0) : pos);
                var x1 = /*x +*/ (vertical ? ((revert) ? rwidth : size) : pos);
                var y0 = /*y + */(vertical ? pos : (revert ? rwidth - size : 0));
                var y1 = /*y +*/ (vertical ? pos : ((revert) ? rwidth : size));

                pen.Thickness = 0.25 * lengthRatio;

                dx.DrawLine(pen, new Point(x0, y0), new Point(x1, y1));
                mm++;
            }
        }
    }
}
