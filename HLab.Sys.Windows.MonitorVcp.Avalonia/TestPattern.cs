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
using HLab.Base.Avalonia.DependencyHelpers;
using HLab.ColorTools.Avalonia;

namespace HLab.Sys.Windows.MonitorVcp.Avalonia;

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

public class TestPattern : Control
{
    static readonly Color Red = new(0xFF,0xFF,0x00,0x00);
    static readonly Color Green = new(0xFF,0x00,0xFF,0x00);
    static readonly Color Blue = new(0xFF, 0x00, 0x00, 0xFF );
    static readonly Color White = new(0xFF, 0xFF, 0xFF, 0xFF );
    static readonly Color Black = new(0xFF, 0x00, 0x00, 0x00 );

    public Window GetWindow(PixelPoint location, double width, double height)
    {
        return new Window
        {
            SystemDecorations = SystemDecorations.None,
            CanResize = false,
            Position = location,
            Height = height,
            Width = width,
            ShowInTaskbar = false,
            //Topmost = true, bug in avalonia, if set there won't be applied on show()
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

    public Window Show(PixelPoint location)
    {
        var panel = Clone().GetWindow(location,1,1);

        panel.Show();
        panel.Topmost = true;
        panel.WindowState = WindowState.FullScreen;

        // TODO : better with double click
        panel.PointerPressed += (s, e) => { panel.Close(); };

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

    public static StyledProperty<Color> PatternColorAProperty = H.Property<Color>().OnChanged((c, e) =>
    {
        c.InvalidateVisual();
    }).Register();

    public Color PatternColorB
    {
        get => (Color)GetValue(PatternColorBProperty);
        set => SetValue(PatternColorBProperty, value);
    }
    public static StyledProperty<Color> PatternColorBProperty = H.Property<Color>().OnChanged((c, e) =>
    {
        c.InvalidateVisual();
    }).Register();

    public static StyledProperty<TestPatternType> PatternTypeProperty = H.Property<TestPatternType>().OnChanged((c, e) =>
    {
        c._onRender = e.NewValue.Value switch
        {
            TestPatternType.Solid => (dc, rect, colorA, colorB, orient) 
                => dc.DrawRectangle(new SolidColorBrush(colorA), null, rect),

            TestPatternType.Gradient => (dc, rect, colorA, colorB, orient) 
                => dc.DrawGradient(colorA.ToColor<double>(), colorB.ToColor<double>(), rect, orient, 1.0 / 2.2 /*2.124*/),

            TestPatternType.Circle => (dc, rect, colorA, colorB, orient) 
                => dc.RenderCircle(colorA, colorB, rect),

            TestPatternType.Circles => (dc, rect, colorA, colorB, orient) 
                => dc.DrawContrast(colorA, colorB, rect, 5, orient),

            TestPatternType.Grid => (dc, rect, colorA, colorB, orient) 
                => dc.DrawHomeCinemaPattern(rect),

            TestPatternType.Gamma => (dc, rect, colorA, colorB, orient) 
                => dc.DrawGamma(colorA.ToColor<double>(), colorB.ToColor<double>(), rect, orient),

            _ => throw new ArgumentOutOfRangeException()
        };

        c.InvalidateVisual();
    }).Register();


    public bool Rgb
    {
        get => (bool)GetValue(RgbProperty);
        set => SetValue(RgbProperty, value);
    }
    public static StyledProperty<bool> RgbProperty = H.Property<bool>().OnChanged((c, e) =>
    {
        c.InvalidateVisual();
    }).Register();

    public Orientation Orientation
    {
        get => GetValue(OrientationProperty);
        set => SetValue(OrientationProperty, value);
    }
    public static StyledProperty<Orientation> OrientationProperty = H.Property<Orientation>().OnChanged((c, e) =>
    {
        c.InvalidateVisual();
    }).Register();

    Action<DrawingContext, Rect, Color, Color, Orientation> _onRender = null;

    public override void Render(DrawingContext dc)
    {
        if (Bounds.Height > 0 && Bounds.Width > 0)
        {
            if (Rgb)
            {
                if (Orientation == Orientation.Horizontal)
                {
                    var height = Bounds.Height / 5;

                    _onRender?.Invoke(dc, new Rect(0, 0,          Bounds.Width, height), PatternColorA, PatternColorB, Orientation);
                    _onRender?.Invoke(dc, new Rect(0, height,     Bounds.Width, height), Red,           PatternColorB, Orientation);
                    _onRender?.Invoke(dc, new Rect(0, height * 2, Bounds.Width, height), Green,         PatternColorB, Orientation);
                    _onRender?.Invoke(dc, new Rect(0, height * 3, Bounds.Width, height), Blue,          PatternColorB, Orientation);
                    _onRender?.Invoke(dc, new Rect(0, height * 4, Bounds.Width, height), PatternColorB, PatternColorA, Orientation);
                }
                else
                {
                    var width = Bounds.Width / 5;

                    _onRender?.Invoke(dc, new Rect(0, 0, width,         Bounds.Height), PatternColorA, PatternColorB, Orientation);
                    _onRender?.Invoke(dc, new Rect(width, 0, width,     Bounds.Height), Red,           PatternColorB, Orientation);
                    _onRender?.Invoke(dc, new Rect(width * 2, 0, width, Bounds.Height), Green,         PatternColorB, Orientation);
                    _onRender?.Invoke(dc, new Rect(width * 3, 0, width, Bounds.Height), Blue,          PatternColorB, Orientation);
                    _onRender?.Invoke(dc, new Rect(width * 4, 0, width, Bounds.Height), PatternColorB, PatternColorA, Orientation);

                }

            }
            else
                _onRender?.Invoke(dc, new Rect(0, 0, Bounds.Width, Bounds.Height), PatternColorA, PatternColorB, Orientation);

        }

        base.Render(dc);
    }

}