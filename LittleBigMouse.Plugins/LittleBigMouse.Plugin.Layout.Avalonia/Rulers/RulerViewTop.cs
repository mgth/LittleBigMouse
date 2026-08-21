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

    /// <summary>
    /// The ruler's outline: a trapezoid whose narrow side is the one facing the monitor, so two
    /// rulers meeting at a corner mitre into each other instead of overlapping.
    /// </summary>
    public abstract IReadOnlyList<Point> ClipOutline();

    public Brush GetBackground(Color color)
    {
        var c = color.ToColor<double>().ToHSL().Darken(0.7).ToAvaloniaColor();
        return GetBrush(c);
    }

    protected abstract Brush GetBrush(Color c);

    protected static Brush GetBrush(double x1, double y1, double x2, double y2, Color c0)
    {
        var c1 = c0.ToColor<double>();
        var c2 = HLabColors.RGB(0, c1.Red / 3, c1.Green / 3, c1.Blue / 3).ToAvaloniaColor();

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

    public override IReadOnlyList<Point> ClipOutline() =>
    [
        new Point(Bounds.Left, Bounds.Top),
        new Point(Bounds.Left + Bounds.Height, Bounds.Bottom),
        new Point(Bounds.Right - Bounds.Height, Bounds.Bottom),
        new Point(Bounds.Right, Bounds.Top),
    ];

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

    public override IReadOnlyList<Point> ClipOutline() =>
    [
        new Point(Bounds.Right, Bounds.Top),
        new Point(Bounds.Left, Bounds.Top + Bounds.Width),
        new Point(Bounds.Left, Bounds.Bottom - Bounds.Width),
        new Point(Bounds.Right, Bounds.Bottom),
    ];

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

    public override IReadOnlyList<Point> ClipOutline() =>
    [
        new Point(Bounds.Left, Bounds.Bottom),
        new Point(Bounds.Left + Bounds.Height, Bounds.Top),
        new Point(Bounds.Right - Bounds.Height, Bounds.Top),
        new Point(Bounds.Right, Bounds.Bottom),
    ];

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

    public override IReadOnlyList<Point> ClipOutline() =>
    [
        new Point(Bounds.Left, Bounds.Top),
        new Point(Bounds.Right, Bounds.Top + Bounds.Width),
        new Point(Bounds.Right, Bounds.Bottom - Bounds.Width),
        new Point(Bounds.Left, Bounds.Bottom),
    ];

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
    readonly Pen _penOut = new(new SolidColorBrush(HLabColors.RGB(0.7, 0.7, 0.7, 0.7).ToAvaloniaColor()), 1);

    protected void Render() => InvalidateVisual();

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

        dc.PushGeometryClip(new PolylineGeometry(o.ClipOutline(), true));

        //   neg     0 actual ruler    L    outside positive
        // |---------|-----------------|----------|

        var geometry = new RulerGeometry(o.Length, RulerStart, RulerLength, Zero, o.Ratio);

        if (geometry.OutsideBefore is { } before)
            dc.DrawRectangle(backgroundOut, null, o.Transform(before.Start, before.End));

        if (geometry.OutsideAfter is { } after)
            dc.DrawRectangle(backgroundOut, null, o.Transform(after.Start, after.End));

        if (geometry.Inside is { } inside)
            dc.DrawRectangle(background, null, o.Transform(inside.Start, inside.End));

        foreach (var graduation in geometry.Graduations())
            Draw(dc, o, graduation);

        base.Render(dc);
    }

    void Draw(DrawingContext dc, RulerOrientation o, RulerGraduation graduation)
    {
        var pen = graduation.Inside ? _penIn : _penOut;

        var origin = new Point(graduation.Position, 0);
        var tip = origin + new Vector(0, graduation.TickLength);

        if (graduation.Label is { } label)
        {
            // A label belongs to its tick: both dim together outside the measured edge. The 5 cm
            // label used to be the exception, drawn with the inside brush whatever its tick did,
            // which left it glaring in the middle of a dimmed overhang.
            var text = new FormattedText(
                label,
                CultureInfo.CurrentCulture,
                FlowDirection.LeftToRight,
                new Typeface("Segoe UI"),
                graduation.LabelSize,
                pen.Brush);

            dc.DrawText(text, o.TextTransform(tip, graduation.LabelSize, text.Width));
        }

        dc.DrawLine(pen, o.Transform(origin), o.Transform(tip));
    }

}
