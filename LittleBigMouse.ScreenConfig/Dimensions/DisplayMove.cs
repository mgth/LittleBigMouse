using ReactiveUI;

namespace LittleBigMouse.DisplayLayout.Dimensions;

public abstract class DisplayMove : DisplaySize
{
    protected DisplayMove(IDisplaySize source) : base(source)
    {
        this.WhenAnyValue(e => e.Source.Width)
            .ToProperty(this, e => e.Width,out _width);

        this.WhenAnyValue(e => e.Source.Height)
            .ToProperty(this, e => e.Height,out _height);

        this.WhenAnyValue(e => e.Source.LeftBorder)
            .ToProperty(this, e => e.LeftBorder,out _leftBorder);

        this.WhenAnyValue(e => e.Source.TopBorder)
            .ToProperty(this, e => e.TopBorder,out _topBorder);

        this.WhenAnyValue(e => e.Source.RightBorder)
            .ToProperty(this, e => e.RightBorder,out _rightBorder);

        this.WhenAnyValue(e => e.Source.BottomBorder)
            .ToProperty(this, e => e.BottomBorder,out _bottomBorder);
    }

    public override double Width
    {
        get => _width.Get();
        set => Source.Width = value;
    }
    readonly ObservableAsPropertyHelper<double> _width;

    public override double Height
    {
        get => _height.Get();
        set => Source.Height = value;
    }
    readonly ObservableAsPropertyHelper<double> _height;

    public override double TopBorder
    {
        get => _topBorder.Get();
        set => Source.TopBorder = value;
    }
    readonly ObservableAsPropertyHelper<double> _topBorder;

    public override double RightBorder
    {
        get => _rightBorder.Get();
        set => Source.RightBorder = value;
    }
    readonly ObservableAsPropertyHelper<double> _rightBorder;

    public override double BottomBorder
    {
        get => _bottomBorder.Get();
        set => Source.BottomBorder = value;
    }
    readonly ObservableAsPropertyHelper<double> _bottomBorder;

    public override double LeftBorder
    {
        get => _leftBorder.Get();
        set => Source.LeftBorder = value;
    }
    readonly ObservableAsPropertyHelper<double> _leftBorder;

}