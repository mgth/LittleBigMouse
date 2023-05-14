using Avalonia;
using ReactiveUI;

namespace LittleBigMouse.DisplayLayout.Dimensions;

public class DisplayRatioValue : DisplayRatio
{
    public DisplayRatioValue(double x, double y)
    {
        _x = x;
        _y = y;
    }
    public DisplayRatioValue(double r):this(r,r) {}
    public DisplayRatioValue(Vector v):this(v.X, v.Y) {}

    public DisplayRatioValue Set(double x, double y)
    {
        _x = x;
        _y = y;
        return this;
    }

    public DisplayRatioValue Set(Vector v)
    {
        _x = v.X;
        _y = v.Y;
        return this;
    }

    public override double X
    {
        get => _x;
        set => this.RaiseAndSetIfChanged(ref _x, value);
    }
    double _x;

    public override double Y
    {
        get => _y;
        set => this.RaiseAndSetIfChanged(ref _y, value);
    }
    double _y;
}
