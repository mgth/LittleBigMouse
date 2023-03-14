using ReactiveUI;
using System.Reactive.Concurrency;

namespace LittleBigMouse.DisplayLayout.Dimensions;

public abstract class DisplayMove : DisplaySize
{
    protected DisplayMove(IDisplaySize source) : base(source)
    {
        _width = this.WhenAnyValue(e => e.Source.Width)
            .ToProperty(this, e => e.Width, scheduler: Scheduler.Immediate);

        _height = this.WhenAnyValue(e => e.Source.Height)
            .ToProperty(this, e => e.Height, scheduler: Scheduler.Immediate);

        _leftBorder = this.WhenAnyValue(e => e.Source.LeftBorder)
            .ToProperty(this, e => e.LeftBorder, scheduler: Scheduler.Immediate);

        _topBorder = this.WhenAnyValue(e => e.Source.TopBorder)
            .ToProperty(this, e => e.TopBorder, scheduler: Scheduler.Immediate);

        _rightBorder = this.WhenAnyValue(e => e.Source.RightBorder)
            .ToProperty(this, e => e.RightBorder, scheduler: Scheduler.Immediate);

        _bottomBorder = this.WhenAnyValue(e => e.Source.BottomBorder)
            .ToProperty(this, e => e.BottomBorder, scheduler: Scheduler.Immediate);

        Init();
    }

    public override double Width
    {
        get => _width.Value;
        set => Source.Width = value;
    }
    readonly ObservableAsPropertyHelper<double> _width;

    public override double Height
    {
        get => _height.Value;
        set => Source.Height = value;
    }
    readonly ObservableAsPropertyHelper<double> _height;

    public override double TopBorder
    {
        get => _topBorder.Value;
        set => Source.TopBorder = value;
    }
    readonly ObservableAsPropertyHelper<double> _topBorder;

    public override double RightBorder
    {
        get => _rightBorder.Value;
        set => Source.RightBorder = value;
    }
    readonly ObservableAsPropertyHelper<double> _rightBorder;

    public override double BottomBorder
    {
        get => _bottomBorder.Value;
        set => Source.BottomBorder = value;
    }
    readonly ObservableAsPropertyHelper<double> _bottomBorder;

    public override double LeftBorder
    {
        get => _leftBorder.Value;
        set => Source.LeftBorder = value;
    }
    readonly ObservableAsPropertyHelper<double> _leftBorder;

    public override string TransformToString => $"Move";

}