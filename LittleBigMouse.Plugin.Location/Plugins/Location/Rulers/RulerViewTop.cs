#define TIMING
using HLab.Base.Wpf;

using System;
using System.Diagnostics;
using System.Globalization;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Media;


namespace LittleBigMouse.Plugin.Location.Plugins.Location.Rulers;

using H = DependencyHelper<RulerViewTop>;



public class RulerViewTop : Grid
{
    //public RulerViewTop()
    //{
    //    SizeChanged += RulerViewTop_SizeChanged;

    //}

    //private void RulerViewTop_SizeChanged(object sender, SizeChangedEventArgs e)
    //{
    //    Render();
    //}


    public static DependencyProperty LengthProperty = H.Property<double>().OnChange(e => e.Render()).Register();
    public static DependencyProperty SizeProperty = H.Property<double>().OnChange(e => e.Render()).Register();
    public static DependencyProperty RulerLengthProperty = H.Property<double>().OnChange(e => e.Render()).Register();
    public static DependencyProperty RulerStartProperty = H.Property<double>().OnChange(e => e.Render()).Register();
    public static DependencyProperty RulerEndProperty = H.Property<double>().OnChange(e => e.Render()).Register();
    public static DependencyProperty ZeroProperty = H.Property<double>().OnChange(e => e.Render()).Register();
    public static DependencyProperty OrientationProperty = H.Property<int>().OnChange(e => e.Render()).Register();
    public static DependencyProperty SelectedProperty = H.Property<bool>().OnChange(e => e.Render()).Register();

    public double Length
    {
        get => (double)GetValue(LengthProperty);
        set => SetValue(LengthProperty, value);
    }
    public double Size
    {
        get => (double)GetValue(SizeProperty);
        set => SetValue(SizeProperty, value);
    }
    public double RulerLength
    {
        get => (double)GetValue(RulerLengthProperty);
        set => SetValue(RulerLengthProperty, value);
    }
    public double RulerStart
    {
        get => (double)GetValue(RulerStartProperty);
        set => SetValue(RulerStartProperty, value);
    }
    public double RulerEnd
    {
        get => (double)GetValue(RulerEndProperty);
        set => SetValue(RulerEndProperty, value);
    }
    public double Zero
    {
        get => (double)GetValue(ZeroProperty);
        set => SetValue(ZeroProperty, value);
    }
    public int Orientation
    {
        get => (int)GetValue(OrientationProperty);
        set => SetValue(OrientationProperty, value);
    }
    public bool Selected
    {
        get => (bool)GetValue(SelectedProperty);
        set => SetValue(SelectedProperty, value);
    }

    private Func<double, double, Point> GetPointFunc()
    {
        return Orientation switch
        {
            0 => (x, y) => new Point(x, y),
            1 => (x, y) => new Point(ActualWidth - y, x),
            2 => (x, y) => new Point(x, ActualHeight - y),
            3 => (x, y) => new Point(y, x),
            _ => (x, y) => new Point()
        };
    }
    private Func<double, double, double, double, Point> GetTextPointFunc()
    {
        var point = GetPointFunc();

        return Orientation switch
        {
            0 => (x, y, s, l) => point(x, y) + new Vector(s * 0.1, -s * 1.1),
            1 => (x, y, s, l) => point(x, y),
            2 => (x, y, s, l) => point(x, y) + new Vector(s * 0.1, -s * 0.3),
            3 => (x, y, s, l) => point(x, y) + new Vector(-l, 0),
            _ => (x, y, s, l) => new Point()
        };
    }



#if TIMING
    private long _elapsed = long.MaxValue;
#endif
    private readonly Pen _penIn = new Pen(Brushes.WhiteSmoke, 1);
    private readonly Pen _penOut = new Pen(new SolidColorBrush(Color.FromScRgb(0.7f, 0.7f, 0.7f, 0.7f)), 1);


    protected void Render()
    {
        InvalidateVisual();
        return;

    }

    private Brush GetBackground(Color color)
    {
        var c = Color.Multiply(color, 0.7f);

        return Orientation switch
        {
            0 => GetBrush(0, 0, 0, 1, c),
            1 => GetBrush(1, 0, 0, 0, c),
            2 => GetBrush(0, 1, 0, 0, c),
            3 => GetBrush(0, 0, 1, 0, c),
            _ => throw new ArgumentOutOfRangeException()
        };
    }
    public Brush GetBrush(double x1, double y1, double x2, double y2, Color c1)
    {
        var c2 = Color.FromScRgb(0, c1.ScR / 3, c1.ScG / 3, c1.ScB / 3);

        c2.A = 0;

        return new LinearGradientBrush
        {
            StartPoint = new Point(x1, y1),
            EndPoint = new Point(x2, y2),
            GradientStops =
                {
                    new GradientStop(c1, 0),
                    new GradientStop(c1, 0.3),
                    new GradientStop(c2, 1),
                }
        };
    }

    protected override void OnRender(DrawingContext dc)
    {

#if TIMING
        var watch = new Stopwatch();
        watch.Start();
#endif
        //  if (!IsLoaded) return;
        if (Math.Abs(ActualHeight) < double.Epsilon || Math.Abs(ActualWidth) < Double.Epsilon) return;
        if (Size < double.Epsilon) return;
        if (Length < double.Epsilon) return;

        var point = GetPointFunc();
        var pointT = GetTextPointFunc();

        var background = GetBackground(Selected ? Colors.DarkGreen : Colors.DarkBlue);
        var backgroundOut = GetBackground(Colors.Black);

        dc.PushClip(new RectangleGeometry(
            new Rect(
                new Point(7, 7), new System.Windows.Size(ActualWidth - 13, ActualHeight - 13))));


        double length, size;
        if (Orientation == 0 || Orientation == 2)
        {
            length = ActualWidth;
            size = ActualHeight;
        }
        else
        {
            length = ActualHeight;
            size = ActualWidth;
        }

        var lengthRatio = length / Length;
        var sizeRatio = size / Size;

        var rulerLength = RulerLength;
        var rulerStart = RulerStart;
        var zero = Zero;


        var pixelsPerDip = 96 / 185;

        //   neg     0 actual ruler    L    outside positive
        // |---------|-----------------|----------|
        // r0        r1                r2         r3

        const double r0 = 0.0;
        var r1 = zero * lengthRatio;
        var r2 = (zero + rulerLength) * lengthRatio;
        var r3 = length;


        var size100 = sizeRatio * 20;
        var size050 = sizeRatio * 15;
        var size010 = sizeRatio * 10;
        var size005 = sizeRatio * 5;
        var size001 = sizeRatio * 2.5;

        var sizeT100 = sizeRatio * 5;
        var sizeT050 = sizeRatio * 4;
        var sizeT010 = sizeRatio * 3;


        if (r0 < r3 && r1 > r0)
            dc.DrawRectangle(backgroundOut, null,
                new Rect(point(r0, r0), point(Math.Min(r1, r3), size)));

        if (r2 < r3)
            dc.DrawRectangle(backgroundOut, null,
                new Rect(point(Math.Max(r2, r0), r0), point(r3, size)));

        if (r1 < r3 && r2 > r0)
            dc.DrawRectangle(background, null,
                new Rect(point(Math.Max(r1, r0), r0), point(Math.Min(r2, r3), size)));

        var mm = (int)rulerStart - 10;

        var pos = (zero + mm) * lengthRatio;

        while (pos < length)
        {
            var pen = (mm < 0 || mm > rulerLength) ? _penOut : _penIn;

            if (mm % 5 == 0)
            {
                if (mm % 10 == 0)
                {
                    if (mm % 50 == 0)
                    {
                        if (mm % 100 == 0)
                        {
                            var t = mm / 100;
                            var txt = new FormattedText(t.ToString(), CultureInfo.CurrentCulture, FlowDirection.LeftToRight,
                            new Typeface("Segoe UI"), sizeT100, pen.Brush, pixelsPerDip);

                            dc.DrawText(txt, pointT(pos, size100, sizeT100, txt.Width));
                            dc.DrawLine(pen, point(pos, r0), point(pos, size100));
                        }
                        else
                        {
                            var txt = new FormattedText("5", CultureInfo.CurrentCulture, FlowDirection.LeftToRight,
                                new Typeface("Segoe UI"), sizeT050, _penIn.Brush, pixelsPerDip);
                            dc.DrawText(txt, pointT(pos, size050, sizeT050, txt.Width));
                            dc.DrawLine(pen, point(pos, r0), point(pos, size050));
                        }
                    }
                    else
                    {
                        var t = (mm % 100) / 10;
                        var txt = new FormattedText(t.ToString(), CultureInfo.CurrentCulture, FlowDirection.LeftToRight,
                            new Typeface("Segoe UI"), sizeT010, pen.Brush, pixelsPerDip);

                        dc.DrawText(txt, pointT(pos, size010, sizeT010, txt.Width));
                        dc.DrawLine(pen, point(pos, 0), point(pos, size010));
                    }
                }
                else
                    dc.DrawLine(pen, point(pos, 0), point(pos, size005));
            }
            else
                dc.DrawLine(pen, point(pos, 0), point(pos, size001));

            mm++;
            pos += lengthRatio;
        }
#if TIMING
        watch.Stop();
        var elapsed = watch.ElapsedTicks;
        _elapsed = Math.Min(elapsed, _elapsed);

        var tb = new FormattedText(_elapsed.ToString(), CultureInfo.CurrentCulture, FlowDirection.LeftToRight,
            new Typeface("Segoe UI"), sizeRatio * 10, _penIn.Brush, pixelsPerDip);

        dc.DrawText(tb, point(Length / 2, Size / 2));
#endif
        base.OnRender(dc);
    }

}
