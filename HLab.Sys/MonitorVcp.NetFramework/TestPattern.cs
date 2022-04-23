/*
  HLab.Windows.MonitorVcp
  Copyright (c) 2017 Mathieu GRENET.  All right reserved.

  This file is part of HLab.Windows.MonitorVcp.

    HLab.Windows.MonitorVcp is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    HLab.Windows.MonitorVcp is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with MouseControl.  If not, see <http://www.gnu.org/licenses/>.

	  mailto:mathieu@mgth.fr
	  http://www.mgth.fr
*/
using HLab.Base.Wpf;
using System;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Media;
using System.Xaml.Schema;

namespace HLab.Windows.MonitorVcp
{
    public enum TestPatternType
    {
        Solid,
        Gradient,
        Circle,
        Circles,
        Grid,
        Gamma
    }

    public class TestPattern : Grid
    {

        private static readonly Color Red = new Color{A=0xFF,R=0xFF,G=0x00,B=0x00};
        private static readonly Color Green = new Color{A=0xFF,R=0x00,G=0xFF,B=0x00};
        private static readonly Color Blue = new Color{A=0xFF,R=0x00,G=0x00,B=0xFF};
        private static readonly Color White = new Color{A=0xFF,R=0xFF,G=0xFF,B=0xFF};
        private static readonly Color Black = new Color{A=0xFF,R=0x00,G=0x00,B=0x00};

        public Window GetWindow(Rect location)
        {
            return new Window
            {
                WindowStyle = WindowStyle.None,
                ResizeMode = ResizeMode.NoResize,
                Left = location.Left,
                Top = location.Top,
                Height = location.Height,
                Width = location.Width,
                Content = this
            };
        }

        public TestPattern Clone()
        {
            return new TestPattern
            {
                PatternColorA = PatternColorA,
                PatternColorB = PatternColorB,
                PatternType = PatternType,
                Orientation = Orientation,
                Rgb = Rgb
            };
        }

        public Window Show(Rect location)
        {
            var panel = Clone().GetWindow(location);

            panel.Show();
            panel.WindowState = WindowState.Maximized;

            return panel;
        }

        private class H : DependencyHelper<TestPattern> { }

        public TestPatternType PatternType
        {
            get => (TestPatternType)GetValue(PatternTypeProperty);
            set => SetValue(PatternTypeProperty, value);
        }

        public Color PatternColorA
        {
            get => (Color)GetValue(PatternColorAProperty);
            set => SetValue(PatternColorAProperty, value);
        }

        public static DependencyProperty PatternColorAProperty = H.Property<Color>().OnChange((c, e) =>
        {
            c.InvalidateVisual();
        }).Register();

        public Color PatternColorB
        {
            get => (Color)GetValue(PatternColorBProperty);
            set => SetValue(PatternColorBProperty, value);
        }
        public static DependencyProperty PatternColorBProperty = H.Property<Color>().OnChange((c, e) =>
         {
             c.InvalidateVisual();
         }).Register();

        public static DependencyProperty PatternTypeProperty = H.Property<TestPatternType>().OnChange((c, e) =>
        {
            switch (e.NewValue)
            {
                case TestPatternType.Solid:
                    c._onRender = (dc, rect, colorA, colorB, orient) => dc.DrawRectangle(new SolidColorBrush(colorA), null, rect);
                    break;
                case TestPatternType.Gradient:
                    c._onRender = (dc, rect, colorA, colorB, orient) => dc.DrawGradient(colorA, colorB, rect, orient, 1.0 / 2.2/*2.124*/);
                    break;
                case TestPatternType.Circle:
                    c._onRender = (dc, rect, colorA, colorB, orient) => dc.RenderCircle(colorA, colorB, rect);
                    break;
                case TestPatternType.Circles:
                    c._onRender = (dc, rect, colorA, colorB, orient) => dc.DrawContrast(colorA,colorB,rect,5,orient);
                    break;
                case TestPatternType.Grid:
                    c._onRender = (dc, rect, colorA, colorB, orient) => dc.DrawHomeCinemaPattern(rect);
                    break;
                case TestPatternType.Gamma:
                    c._onRender = (dc, rect, colorA, colorB, orient) => dc.DrawGamma(colorA, colorB, rect, orient);
                    break;
                default:
                    throw new ArgumentOutOfRangeException();
            }

            c.InvalidateVisual();
        }).Register();


        public bool Rgb
        {
            get => (bool)GetValue(RgbProperty);
            set => SetValue(RgbProperty, value);
        }
        public static DependencyProperty RgbProperty = H.Property<bool>().OnChange((c, e) =>
        {
            c.InvalidateVisual();
        }).Register();

        public Orientation Orientation
        {
            get => (Orientation)GetValue(OrientationProperty);
            set => SetValue(OrientationProperty, value);
        }
        public static DependencyProperty OrientationProperty = H.Property<Orientation>().OnChange((c, e) =>
        {
            c.InvalidateVisual();
        }).Register();

        private Action<DrawingContext, Rect, Color, Color, Orientation> _onRender = null;

        protected override void OnRender(DrawingContext dc)
        {
            if (ActualHeight > 0 && ActualWidth > 0)
            {
                if (Rgb)
                {
                    if (Orientation == Orientation.Horizontal)
                    {
                        var height = ActualHeight / 5;

                        _onRender?.Invoke(dc, new Rect(0, 0,          ActualWidth, height), PatternColorA, PatternColorB, Orientation);
                        _onRender?.Invoke(dc, new Rect(0, height,     ActualWidth, height), Red,           PatternColorB, Orientation);
                        _onRender?.Invoke(dc, new Rect(0, height * 2, ActualWidth, height), Green,         PatternColorB, Orientation);
                        _onRender?.Invoke(dc, new Rect(0, height * 3, ActualWidth, height), Blue,          PatternColorB, Orientation);
                        _onRender?.Invoke(dc, new Rect(0, height * 4, ActualWidth, height), PatternColorB, PatternColorA, Orientation);
                    }
                    else
                    {
                        var width = ActualWidth / 5;

                        _onRender?.Invoke(dc, new Rect(0, 0, width,         ActualHeight), PatternColorA, PatternColorB, Orientation);
                        _onRender?.Invoke(dc, new Rect(width, 0, width,     ActualHeight), Red,           PatternColorB, Orientation);
                        _onRender?.Invoke(dc, new Rect(width * 2, 0, width, ActualHeight), Green,         PatternColorB, Orientation);
                        _onRender?.Invoke(dc, new Rect(width * 3, 0, width, ActualHeight), Blue,          PatternColorB, Orientation);
                        _onRender?.Invoke(dc, new Rect(width * 4, 0, width, ActualHeight), PatternColorB, PatternColorA, Orientation);

                    }

                }
                else
                    _onRender?.Invoke(dc, new Rect(0, 0, ActualWidth, ActualHeight), PatternColorA, PatternColorB, Orientation);

            }

            base.OnRender(dc);
        }

    }
}
