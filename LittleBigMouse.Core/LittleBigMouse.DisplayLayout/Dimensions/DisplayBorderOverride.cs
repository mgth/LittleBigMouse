using System.Reactive.Concurrency;
using ReactiveUI;

namespace LittleBigMouse.DisplayLayout.Dimensions;

/// <summary>
/// A size decorator that keeps the width/height/position of its <see cref="DisplaySize.Source"/>
/// (the shared per-model physical size) but takes the four bezel borders from a separate per-monitor
/// <see cref="DisplayBorders"/> holder. This is what "Border values: per monitor" roots a monitor's
/// geometry at, so identical monitors can carry different borders while still sharing their size.
/// Sits at the raw-size root of the chain (below rotation/scale), so those transforms apply to the
/// overridden borders exactly as they do to the model's.
/// </summary>
public class DisplayBorderOverride : DisplaySize
{
    public DisplayBorderOverride(IDisplaySize source, DisplayBorders borderSource) : base(source)
    {
        BorderSource = borderSource;

        _x = this.WhenAnyValue(e => e.Source.X).ToProperty(this, e => e.X, scheduler: Scheduler.Immediate);
        _y = this.WhenAnyValue(e => e.Source.Y).ToProperty(this, e => e.Y, scheduler: Scheduler.Immediate);
        _width = this.WhenAnyValue(e => e.Source.Width).ToProperty(this, e => e.Width, scheduler: Scheduler.Immediate);
        _height = this.WhenAnyValue(e => e.Source.Height).ToProperty(this, e => e.Height, scheduler: Scheduler.Immediate);

        _leftBorder = this.WhenAnyValue(e => e.BorderSource.Left).ToProperty(this, e => e.LeftBorder, scheduler: Scheduler.Immediate);
        _topBorder = this.WhenAnyValue(e => e.BorderSource.Top).ToProperty(this, e => e.TopBorder, scheduler: Scheduler.Immediate);
        _rightBorder = this.WhenAnyValue(e => e.BorderSource.Right).ToProperty(this, e => e.RightBorder, scheduler: Scheduler.Immediate);
        _bottomBorder = this.WhenAnyValue(e => e.BorderSource.Bottom).ToProperty(this, e => e.BottomBorder, scheduler: Scheduler.Immediate);

        base.Init();
    }

    /// <summary>The per-monitor border values this override reads from (distinct from the inherited
    /// <see cref="DisplaySize.Borders"/>, which is the computed Thickness output).</summary>
    public DisplayBorders BorderSource { get; }

    // Size + position: pass through to the shared per-model source.
    public override double X { get => _x?.Value ?? 0; set => Source.X = value; }
    readonly ObservableAsPropertyHelper<double> _x;

    public override double Y { get => _y?.Value ?? 0; set => Source.Y = value; }
    readonly ObservableAsPropertyHelper<double> _y;

    public override double Width { get => _width?.Value ?? 0; set => Source.Width = value; }
    readonly ObservableAsPropertyHelper<double> _width;

    public override double Height { get => _height?.Value ?? 0; set => Source.Height = value; }
    readonly ObservableAsPropertyHelper<double> _height;

    // Borders: read from / written to the per-monitor holder, NOT the shared source.
    public override double LeftBorder { get => _leftBorder?.Value ?? 0; set => BorderSource.Left = value; }
    readonly ObservableAsPropertyHelper<double> _leftBorder;

    public override double TopBorder { get => _topBorder?.Value ?? 0; set => BorderSource.Top = value; }
    readonly ObservableAsPropertyHelper<double> _topBorder;

    public override double RightBorder { get => _rightBorder?.Value ?? 0; set => BorderSource.Right = value; }
    readonly ObservableAsPropertyHelper<double> _rightBorder;

    public override double BottomBorder { get => _bottomBorder?.Value ?? 0; set => BorderSource.Bottom = value; }
    readonly ObservableAsPropertyHelper<double> _bottomBorder;

    public override string TransformToString => "BorderOverride";
}
