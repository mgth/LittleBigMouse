using Avalonia.Media;

namespace LittleBigMouse.Ui.Avalonia.MonitorFrame;

/// <summary>
/// One colored strip along a monitor frame's edge, in frame-local visual units:
/// the rendering of a single edge-prober run (red = wall, green = crossing).
/// </summary>
public sealed record ProbeStripViewModel(
    double Left,
    double Top,
    double Width,
    double Height,
    bool IsWall,
    string Tip)
{
    static readonly IBrush WallBrush = new SolidColorBrush(Color.Parse("#E5484D"), 0.9);
    static readonly IBrush CrossBrush = new SolidColorBrush(Color.Parse("#46A758"), 0.7);

    public IBrush Fill => IsWall ? WallBrush : CrossBrush;
}
