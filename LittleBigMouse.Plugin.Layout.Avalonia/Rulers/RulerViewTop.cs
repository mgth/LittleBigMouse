#define TIMING

using System.Globalization;
using Avalonia;
using Avalonia.Controls;
using Avalonia.Media;
using HLab.Base.Avalonia.DependencyHelpers;
using HLab.ColorTools;
using HLab.ColorTools.Avalonia;

namespace LittleBigMouse.Plugin.Layout.Avalonia.Rulers;

using H = DependencyHelper<RulerViewTop>;

abstract class RulerOrientation
{
    public static RulerOrientation Create(double size, double length, Rect bounds, int orientation)
    {
        return orientation switch
        {
            0 => new RulerOrientationTop(size, length, bounds),
            1 => new RulerOrientationRight(size, length, bounds),
            2 => new RulerOrientationBottom(size, length, bounds),
            3 => new RulerOrientationLeft(size, length, bounds),
            _ => throw new ArgumentOutOfRangeException(nameof(orientation), orientation, null)
        };
    }

    protected Rect Bounds;
    protected Matrix Scale;
    public Double Ratio { get; }

    protected RulerOrientation(double size, double length, Rect bounds)
    {
        Size = size;
        Length = length;
        Bounds = bounds;
        Ratio = DisplayLength / length;
        Scale = Matrix.CreateScale(Ratio, Ratio);
    }

    public double Length { get; }
    public double Size { get; }

    public abstract double DisplayLength { get; }
    public abstract double DisplaySize { get; }

    public abstract Point Transform(Point p);

    public Rect Transform(Rect r)
    {
        var p1 = Transform(r.TopLeft);
        var p2 = Transform(r.BottomRight);
        return new Rect(p1, p2);
    }

    public abstract Rect Transform(double start, double end);

    public abstract Point TextTransform(Point p, double size, double length);

    public Brush GetBackground(Color color)
    {
        var c = color.ToColor<double>().ToHSL().Darken(0.7).ToAvaloniaColor();
        return GetBrush(c);
    }

    protected abstract Brush GetBrush(Color c);

    protected static Brush GetBrush(double x1, double y1, double x2, double y2, Color c0)
    {
        var c1 = c0.ToColor<double>();
        var c2 = new ColorRGB<double>(0, c1.Red / 3, c1.Green / 3, c1.Blue / 3).ToAvaloniaColor();

        return new LinearGradientBrush
        {
            StartPoint = new RelativePoint(new Point(x1, y1), RelativeUnit.Relative),
            EndPoint = new RelativePoint(new Point(x2, y2), RelativeUnit.Relative),
            GradientStops =
            {
                new GradientStop(c0, 0),
                new GradientStop(c0, 0.3),
                new GradientStop(c2, 1),
            }
        };
    }


}

class RulerOrientationTop : RulerOrientation
{
    public override double DisplayLength => Bounds.Width;
    public override double DisplaySize => Bounds.Height;

    public override Point Transform(Point p) => p * Scale;

    public override Rect Transform(double start, double end)
        => new(
            new Point(start * Ratio, 0),
            new Point(end * Ratio, DisplaySize)
            );

    public override Point TextTransform(Point p, double size, double length)
        => Transform(p) + (new Vector(size * 0.1, -size * 1.1));

    protected override Brush GetBrush(Color color) => RulerOrientation.GetBrush(0, 0, 0, 1, color);

    public RulerOrientationTop(double size, double length, Rect bounds) : base(size, length, bounds)
    {
    }
}

class RulerOrientationRight : RulerOrientation
{
    public override double DisplayLength => Bounds.Height;
    public override double DisplaySize => Bounds.Width;
    public override Point Transform(Point p)
    {
        var p1 = p * Scale;
        return new Point(DisplaySize - p1.Y, p1.X);
    }

    public override Rect Transform(double start, double end) => new(
        new Point(0, start * Ratio),
        new Point(DisplaySize, end * Ratio)
    );

    public override Point TextTransform(Point p, double size, double length) => Transform(p);

    protected override Brush GetBrush(Color color) => RulerOrientation.GetBrush(1, 0, 0, 0, color);

    public RulerOrientationRight(double size, double length, Rect bounds) : base(size, length, bounds)
    {
    }
}
class RulerOrientationBottom : RulerOrientation
{
    public override double DisplayLength => Bounds.Width;
    public override double DisplaySize => Bounds.Height;
    public override Point Transform(Point p)
    {
        var p1 = p * Scale;
        return new Point(p1.X, DisplaySize - p1.Y);
    }

    public override Rect Transform(double start, double end) => new(
        new Point(start * Ratio, 0),
        new Point(end * Ratio, DisplaySize)
    );

    public override Point TextTransform(Point p, double size, double length)
        => Transform(p) + new Vector(size * 0.1, -size * 0.3);
    protected override Brush GetBrush(Color color) => RulerOrientation.GetBrush(0, 1, 0, 0, color);

    public RulerOrientationBottom(double size, double length, Rect bounds) : base(size, length, bounds)
    {
    }
}

class RulerOrientationLeft : RulerOrientation
{
    public override double DisplayLength => Bounds.Height;
    public override double DisplaySize => Bounds.Width;

    public override Point Transform(Point p) => new Point(p.Y, p.X) * Scale;

    public override Rect Transform(double start, double end)=> new(
        new Point(0, start * Ratio),
        new Point(DisplaySize, end * Ratio)
    );

    public override Point TextTransform(Point p, double size, double length)
        => Transform(p) + new Vector(-length, 0);

    protected override Brush GetBrush(Color color) => RulerOrientation.GetBrush(0, 0, 1, 0, color);

    public RulerOrientationLeft(double size, double length, Rect bounds) : base(size, length, bounds)
    {
    }
}

public class RulerViewTop : Control
{
    public RulerViewTop()
    {
        //SizeChanged += RulerViewTop_SizeChanged;
    }

    //private void RulerViewTop_SizeChanged(object sender, SizeChangedEventArgs e)
    //{
    //    Render();
    //}


    public static readonly StyledProperty<double> LengthProperty = H
        .Property<double>()
        .OnChanged((e, a) => e.Render())
        .Register();

    public static readonly StyledProperty<double> SizeProperty = H
        .Property<double>()
        .OnChanged((e, a) => e.Render())
        .Register();

    public static readonly StyledProperty<double> RulerLengthProperty = H
        .Property<double>()
        .OnChanged((e, a) => e.Render())
        .Register();

    public static readonly StyledProperty<double> RulerStartProperty = H
        .Property<double>()
        .OnChanged((e, a) => e.Render())
        .Register();

    public static readonly StyledProperty<double> RulerEndProperty = H
        .Property<double>()
        .OnChanged((e, a) => e.Render())
        .Register();

    public static readonly StyledProperty<double> ZeroProperty = H
        .Property<double>()
        .OnChanged((e, a) => e.Render())
        .Register();

    public static readonly StyledProperty<int> OrientationProperty = H
        .Property<int>()
        .OnChanged((e, a) => e.Render())
        .Register();

    public static readonly StyledProperty<bool> SelectedProperty = H
        .Property<bool>()
        .OnChanged((e, a) => e.Render())
        .Register();

    public double Length
    {
        get => GetValue(LengthProperty);
        set => SetValue(LengthProperty, value);
    }

    public double Size
    {
        get => GetValue(SizeProperty);
        set => SetValue(SizeProperty, value);
    }

    public double RulerLength
    {
        get => GetValue(RulerLengthProperty);
        set => SetValue(RulerLengthProperty, value);
    }

    public double RulerStart
    {
        get => GetValue(RulerStartProperty);
        set => SetValue(RulerStartProperty, value);
    }

    public double RulerEnd
    {
        get => GetValue(RulerEndProperty);
        set => SetValue(RulerEndProperty, value);
    }

    public double Zero
    {
        get => GetValue(ZeroProperty);
        set => SetValue(ZeroProperty, value);
    }

    public int Orientation
    {
        get => GetValue(OrientationProperty);
        set => SetValue(OrientationProperty, value);
    }

    public bool Selected
    {
        get => GetValue(SelectedProperty);
        set => SetValue(SelectedProperty, value);
    }


    readonly Pen _penIn = new(Brushes.WhiteSmoke, 1);
    readonly Pen _penOut = new(new SolidColorBrush(new ColorRGB<double>(0.7, 0.7, 0.7, 0.7).ToAvaloniaColor()), 1);

    protected void Render()
    {
        InvalidateVisual();
        return;

    }

    public override void Render(DrawingContext dc)
    {
        // Check actual bounds and size to avoid rendering errors
        if (Math.Abs(Bounds.Height) < double.Epsilon || Math.Abs(Bounds.Width) < double.Epsilon) return;
        if (Size < double.Epsilon) return;
        if (Length < double.Epsilon) return;

        // Create orientation helper
        var o = RulerOrientation.Create(Size, Length, Bounds, Orientation);

        // Get brushes for background
        var background = o.GetBackground(Selected ? Colors.DarkGreen : Colors.DarkBlue);
        var backgroundOut = o.GetBackground(Colors.Black);

        //dc.PushClip(new RoundedRect(
        //    new Rect(
        //        new Point(7, 7),
        //        new Size(Bounds.Width - 13, Bounds.Height - 13))));
        switch (Orientation)
        {
            case 0:
                dc.PushGeometryClip(new PolylineGeometry(
                    new List<Point>()
                    {
                        new Point(Bounds.Left, Bounds.Top),
                        new Point(Bounds.Left + Bounds.Height, Bounds.Bottom), 
                        new Point(Bounds.Right - Bounds.Height, Bounds.Bottom), 
                        new Point(Bounds.Right, Bounds.Top), 
                    }, true));
                break;
            case 1:
                dc.PushGeometryClip(new PolylineGeometry(
                    new List<Point>()
                    {
                        new Point(Bounds.Right, Bounds.Top), 
                        new Point(Bounds.Left, Bounds.Top + Bounds.Width), 
                        new Point(Bounds.Left, Bounds.Bottom - Bounds.Width), 
                        new Point(Bounds.Right, Bounds.Bottom), 
                    }, true));
                break;
            case 2:
                dc.PushGeometryClip(new PolylineGeometry(
                    new List<Point>()
                    {
                        new Point(Bounds.Left, Bounds.Bottom), 
                        new Point(Bounds.Left + Bounds.Height, Bounds.Top), 
                        new Point(Bounds.Right - Bounds.Height, Bounds.Top), 
                        new Point(Bounds.Right, Bounds.Bottom), 
                    }, true));
                break;
            case 3:
                dc.PushGeometryClip(new PolylineGeometry(
                    new List<Point>()
                    {
                        new Point(Bounds.Left, Bounds.Top), 
                        new Point(Bounds.Right, Bounds.Top + Bounds.Width), 
                        new Point(Bounds.Right, Bounds.Bottom - Bounds.Width), 
                        new Point(Bounds.Left, Bounds.Bottom), 
                    }, true));
                break;
        }


        var rulerLength = RulerLength;
        var rulerStart = RulerStart;
        var zero = Zero;

        //var pixelsPerDip = 96 / 185;

        //   neg     0 actual ruler    L    outside positive
        // |---------|-----------------|----------|
        // r0        r1                r2         r3

        const double r0 = 0.0;
        var r1 = zero;
        var r2 = (zero + rulerLength);
        var r3 = o.Length;

        var v100 = new Vector(0, 20); // size of 10cm graduations
        var v050 = new Vector(0, 15); // size of 5cm graduations
        var v010 = new Vector(0, 10); // size of 1cm graduations
        var v005 = new Vector(0, 5);  // size of 5mm graduations
        var v001 = new Vector(0, 2.5); // size of 1mm graduations

        var sizeT100 = 5.0 * o.Ratio;
        var sizeT050 = 4.0 * o.Ratio;
        var sizeT010 = 3.0 * o.Ratio;

        //draw outside background negative part
        if (r0 < r3 && r1 > r0)
            dc.DrawRectangle(backgroundOut, null,
                o.Transform(r0, Math.Min(r1, r3)));

        //draw outside background positive part
        if (r2 < r3)
            dc.DrawRectangle(backgroundOut, null,
                o.Transform(Math.Max(r2, r0), r3));

        // draw inside background
        if (r1 < r3 && r2 > r0)
            dc.DrawRectangle(background, null,
                o.Transform(Math.Max(r1, r0), Math.Min(r2, r3)));

        var mm = (int)rulerStart - 10;

        var pos = (zero + mm);

        while (pos < o.Length)
        {
            var pen = mm < 0 || mm > rulerLength ? _penOut : _penIn;

            var p0 = new Point(pos, r0);

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
                            new Typeface("Segoe UI"), sizeT100, pen.Brush/*, pixelsPerDip*/);

                            dc.DrawText(txt, o.TextTransform(p0 + v100, sizeT100, txt.Width));
                            dc.DrawLine(pen, o.Transform(p0), o.Transform(p0 + v100));

                        }
                        else
                        {
                            var txt = new FormattedText("5", CultureInfo.CurrentCulture, FlowDirection.LeftToRight,
                                new Typeface("Segoe UI"), sizeT050, _penIn.Brush/*, pixelsPerDip*/);

                            dc.DrawText(txt, o.TextTransform(p0 + v050, sizeT050, txt.Width));
                            dc.DrawLine(pen, o.Transform(p0), o.Transform(p0 + v050));
                        }
                    }
                    else
                    {
                        var t = mm % 100 / 10;
                        var txt = new FormattedText(
                            t.ToString(),
                            CultureInfo.CurrentCulture,
                            FlowDirection.LeftToRight,
                            new Typeface("Segoe UI"),
                            sizeT010,
                            pen.Brush/*, pixelsPerDip*/);

                        dc.DrawText(txt, o.TextTransform(p0 + v010, sizeT010, txt.Width));
                        dc.DrawLine(pen, o.Transform(p0), o.Transform(p0 + v010));
                    }
                }
                else
                    dc.DrawLine(pen, o.Transform(p0), o.Transform(p0 + v005));
            }
            else
                dc.DrawLine(pen, o.Transform(p0), o.Transform(p0 + v001));

            mm++;
            pos += 1.0;
        }

        base.Render(dc);
    }

}
