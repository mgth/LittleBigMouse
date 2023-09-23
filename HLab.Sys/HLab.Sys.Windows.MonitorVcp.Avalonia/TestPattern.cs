/*
  HLab.Windows.MonitorVcp
  Copyright (c) 2021 Mathieu GRENET.  All right reserved.

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

using Avalonia;
using Avalonia.Controls;
using Avalonia.Layout;
using Avalonia.Media;
using HLab.Base.Avalonia;

namespace HLab.Sys.Windows.MonitorVcp.Avalonia
{
    using H = DependencyHelper<TestPattern>;

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
        static readonly Color Red = new( 0xFF,0xFF,0x00,0x00);
        static readonly Color Green = new(0xFF,0x00,0xFF,0x00);
        static readonly Color Blue = new(0xFF,0x00,0x00,0xFF);
        static readonly Color White = new(0xFF,0xFF,0xFF,0xFF);
        static readonly Color Black = new(0xFF,0x00,0x00,0x00);

        public Window GetWindow(Rect location)
        {
            return new Window
            {

                //WindowStyle = WindowStyle.None,
                //ResizeMode = ResizeMode.NoResize,
                //Left = location.Left,
                //Top = location.Top,
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

            panel.PointerPressed += (s, e) =>
            {
                if(e.ClickCount>1)
                    panel.Close();
            };

            return panel;
        }

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

        public static AvaloniaProperty PatternColorAProperty = H.Property<Color>().OnChangeBeforeNotification((c) =>
        {
            c.InvalidateVisual();
        }).Register();

        public Color PatternColorB
        {
            get => (Color)GetValue(PatternColorBProperty);
            set => SetValue(PatternColorBProperty, value);
        }
        public static AvaloniaProperty PatternColorBProperty = H.Property<Color>().OnChangeBeforeNotification((c) =>
         {
             c.InvalidateVisual();
         }).Register();

        public static AvaloniaProperty PatternTypeProperty = H.Property<TestPatternType>().OnChangeBeforeNotification((c) =>
        {
            c._onRender = c.PatternType switch
            {
                TestPatternType.Solid => (dc, rect, colorA, colorB, orient) =>
                    dc.DrawRectangle(new SolidColorBrush(colorA), null, rect),
                TestPatternType.Gradient => (dc, rect, colorA, colorB, orient) =>
                    dc.DrawGradient(colorA, colorB, rect, orient, 1.0 / 2.2 /*2.124*/),
                TestPatternType.Circle => (dc, rect, colorA, colorB, orient) => dc.RenderCircle(colorA, colorB, rect),
                TestPatternType.Circles => (dc, rect, colorA, colorB, orient) =>
                    dc.DrawContrast(colorA, colorB, rect, 5, orient),
                TestPatternType.Grid => (dc, rect, colorA, colorB, orient) => dc.DrawHomeCinemaPattern(rect),
                TestPatternType.Gamma => (dc, rect, colorA, colorB, orient) =>
                    dc.DrawGamma(colorA, colorB, rect, orient),
                _ => throw new ArgumentOutOfRangeException()
            };

            c.InvalidateVisual();
        }).Register();


        public bool Rgb
        {
            get => (bool)GetValue(RgbProperty);
            set => SetValue(RgbProperty, value);
        }
        public static AvaloniaProperty RgbProperty = H.Property<bool>().OnChangeBeforeNotification((c) =>
        {
            c.InvalidateVisual();
        }).Register();

        public Orientation Orientation
        {
            get => (Orientation)GetValue(OrientationProperty);
            set => SetValue(OrientationProperty, value);
        }
        public static AvaloniaProperty OrientationProperty = H.Property<Orientation>().OnChangeBeforeNotification((c) =>
        {
            c.InvalidateVisual();
        }).Register();

        Action<DrawingContext, Rect, Color, Color, Orientation> _onRender = null;

        // TODO : Avalonia
        protected void OnRender(DrawingContext dc)
        {
            if (Height > 0 && Width > 0)
            {
                if (Rgb)
                {
                    if (Orientation == Orientation.Horizontal)
                    {
                        var height = Height / 5;

                        _onRender?.Invoke(dc, new Rect(0, 0,          Width, height), PatternColorA, PatternColorB, Orientation);
                        _onRender?.Invoke(dc, new Rect(0, height,     Width, height), Red,           PatternColorB, Orientation);
                        _onRender?.Invoke(dc, new Rect(0, height * 2, Width, height), Green,         PatternColorB, Orientation);
                        _onRender?.Invoke(dc, new Rect(0, height * 3, Width, height), Blue,          PatternColorB, Orientation);
                        _onRender?.Invoke(dc, new Rect(0, height * 4, Width, height), PatternColorB, PatternColorA, Orientation);
                    }
                    else
                    {
                        var width = Width / 5;

                        _onRender?.Invoke(dc, new Rect(0, 0, width,         Height), PatternColorA, PatternColorB, Orientation);
                        _onRender?.Invoke(dc, new Rect(width, 0, width,     Height), Red,           PatternColorB, Orientation);
                        _onRender?.Invoke(dc, new Rect(width * 2, 0, width, Height), Green,         PatternColorB, Orientation);
                        _onRender?.Invoke(dc, new Rect(width * 3, 0, width, Height), Blue,          PatternColorB, Orientation);
                        _onRender?.Invoke(dc, new Rect(width * 4, 0, width, Height), PatternColorB, PatternColorA, Orientation);

                    }

                }
                else
                    _onRender?.Invoke(dc, new Rect(0, 0, Width, Height), PatternColorA, PatternColorB, Orientation);

            }

            //base.OnRender(dc);
        }

    }
}
