/*
  MouseControl - LbmMouse Managment in multi DPI monitors environment
  Copyright (c) 2015 Mathieu GRENET.  All right reserved.

  This file is part of MouseControl.

    ArduixPL is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    ArduixPL is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with MouseControl.  If not, see <http://www.gnu.org/licenses/>.

	  mailto:mathieu@mgth.fr
	  http://www.mgth.fr
*/

using System.ComponentModel;
using System.Globalization;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;
using System.Windows.Interop;
using System.Windows.Media;
using System.Windows.Shapes;
using Erp.Mvvm;
using Microsoft.Win32.SafeHandles;
using WinAPI;

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

            //int mm = (int)rulerLengthOut - (int)ViewModel.DrawOn.InMm.Height;
            var mm = (int)rulerStart;

            if (vertical)
            {
                dx.DrawRectangle(ViewModel.BackgroundOut, null,new Rect(new Point(0,0),new Point(rwidth,zero)));
                dx.DrawRectangle(ViewModel.BackgroundOut, null,new Rect(new Point(0, zero + rulerInLength * lengthRatio),new Point(rwidth, zero + rulerEnd * lengthRatio)));
                dx.DrawRectangle(ViewModel.Background, null,new Rect(new Point(0,zero),new Point(rwidth,zero + rulerInLength * lengthRatio)));
            }
            else
            {
                dx.DrawRectangle(ViewModel.BackgroundOut, null, new Rect(new Point(0, 0), new Point(zero, rwidth)));
                dx.DrawRectangle(ViewModel.BackgroundOut, null, new Rect(new Point(zero + rulerInLength * lengthRatio,0), new Point(zero + rulerEnd * lengthRatio, rwidth)));
                dx.DrawRectangle(ViewModel.Background, null,new Rect(new Point(zero,0),new Point(zero + rulerInLength * lengthRatio,rwidth)));
            }

            while (mm < (int)rulerEnd + 1)
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

                pen.Thickness = 0.1 * lengthRatio;

                dx.DrawLine(pen, new Point(x0, y0), new Point(x1, y1));
                mm++;
            }
        }
    }
}
