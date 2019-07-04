using System;
using System.Collections.Generic;
using System.ComponentModel;
using System.Globalization;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Media;

namespace LittleBigMouse.Plugin.Location.Plugins.Location.Rulers
{
    public class RulerVisual : DrawingVisual
    {
        public RulerVisual()
        {
            VisualEdgeMode = EdgeMode.Aliased;
        }
    }


    public class RulerViewTop : Grid
    {
        private  DrawingVisual _visual = null;
        public RulerViewTop()
        {
            DataContextChanged += Ruler_DataContextChanged;
            SizeChanged += RulerViewTop_SizeChanged;
            
        }

        private void RulerViewTop_SizeChanged(object sender, SizeChangedEventArgs e)
        {
            Render();
        }

        private void Ruler_DataContextChanged(object sender, DependencyPropertyChangedEventArgs e)
        {
            if (e.OldValue is INotifyPropertyChanged oldVm) oldVm.PropertyChanged -= ViewModel_PropertyChanged;
            if (e.NewValue is INotifyPropertyChanged newVm)
            {
                newVm.PropertyChanged += ViewModel_PropertyChanged;
                Render();
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
                    Render();
                    break;
                default:
                    return;
            }
        }


        protected void Render()
        {
            if (!IsLoaded) return;
            if (!(DataContext is RulerViewModel vm)) return;
            if (Math.Abs(ActualHeight) < double.Epsilon || Math.Abs(ActualWidth) < Double.Epsilon) return;


            var side = vm.Side;


            var vertical = side == RulerViewModel.RulerSide.Left || side == RulerViewModel.RulerSide.Right;
            var revert = side == RulerViewModel.RulerSide.Right || side == RulerViewModel.RulerSide.Bottom;

            var lengthRatio = vm.LengthRatio;
            var sizeRatio = vm.SizeRatio;

            var rulerInLength = vm.RulerLength;

            var rulerStart = vm.RulerStart;
//            var rulerEnd = vm.RulerEnd;

            var zero = vm.Zero;

            var pixelsPerDip = 96 / vm.Screen.RealDpiAvg;

            var rWidth = (vertical ? vm.RatioX : vm.RatioY) * 30;
            var penIn = new Pen(Brushes.WhiteSmoke, 1);
            var penOut = new Pen(new SolidColorBrush(Color.FromScRgb(0.7f, 0.7f, 0.7f, 0.7f)), 1);


            //   neg     0 actual ruler    L    outside positive
            // |---------|-----------------|----------|
            // r0        r1                r2         r3

            const double r0 = 1.0;
            var r1 = zero * lengthRatio;
            var r2 = (zero + rulerInLength) * lengthRatio;
            //var r3 = (zero + rulerEnd) * lengthRatio;
            var r3 = vertical ? ActualHeight : ActualWidth;

            var rulerEnd = 1 + (r3 - r1) / lengthRatio;

            var visual = new RulerVisual();
            using (var dx = visual.RenderOpen())
            {
                if (vertical)
                {
                    if (r0 < r3 && r1 > r0)
                        dx.DrawRectangle(vm.BackgroundOut, null,
                            new Rect(new Point(r0, r0), new Point(rWidth, Math.Min(r1, r3))));

                    if (r2 < r3)
                        dx.DrawRectangle(vm.BackgroundOut, null,
                            new Rect(new Point(0.0, Math.Max(r2, r0)), new Point(rWidth, r3)));

                    if (r1 < r3 && r2 > r0)
                        dx.DrawRectangle(vm.Background, null,
                            new Rect(new Point(0.0, Math.Max(r1, r0)), new Point(rWidth, Math.Min(r2, r3))));
                }
                else
                {
                    if (r0 < r3 && r1 > r0)
                        dx.DrawRectangle(vm.BackgroundOut, null,
                            new Rect(new Point(r0, r0), new Point(Math.Min(r1, r3), rWidth)));

                    if (r2 < r3)
                        dx.DrawRectangle(vm.BackgroundOut, null,
                            new Rect(new Point(Math.Max(r2, r0),0.0), new Point(r3, rWidth)));

                    if (r1 < r3 && r2 > r0)
                        dx.DrawRectangle(vm.Background, null,
                            new Rect(new Point(Math.Max(r1, r0), 0.0), new Point(Math.Min(r2, r3), rWidth)));
                }

                var mm = (int) rulerStart;
                while (mm < (int) rulerEnd)
                {
                    var pos = (zero + mm) * lengthRatio;

                    var size = sizeRatio;
                    string text = null;

                    var pen = (mm < 0 || mm > rulerInLength) ? penOut : penIn;

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
                        var t = new FormattedText(text, CultureInfo.CurrentCulture, FlowDirection.LeftToRight,
                            new Typeface("Segoe UI"), size / 3, pen.Brush, pixelsPerDip);


                        var p2 = (!revert) ? (size - size / 3) : (rWidth - size);

                        dx.DrawText(t, vertical ? new Point(p2, pos) : new Point(pos, p2));
                    }

                    var x0 = /*x +*/ (vertical ? ((revert) ? rWidth - size : 0) : pos);
                    var x1 = /*x +*/ (vertical ? ((revert) ? rWidth : size) : pos);
                    var y0 = /*y + */(vertical ? pos : (revert ? rWidth - size : 0));
                    var y1 = /*y +*/ (vertical ? pos : ((revert) ? rWidth : size));

                    pen.Thickness = 1.0;//0.25 * lengthRatio;

                    dx.DrawLine(pen, new Point(x0, y0), new Point(x1, y1));
                    mm++;
                }
            }
            RemoveVisualChild(_visual);
            AddVisualChild(visual);
            _visual = visual;
        }

        protected override int VisualChildrenCount => 1;

        protected override Visual GetVisualChild(int index) => _visual;
    }
}
