using Avalonia.Collections;
using Avalonia.Media;
using LittleBigMouse.DisplayLayout.Monitors;

namespace LittleBigMouse.Plugin.Layout.Avalonia.LocationPlugin.Anchors;

public class Anchor
{
    public PhysicalMonitor Monitor { get; }
    public double Pos { get; }
    public IBrush Brush { get; }
    public AvaloniaList<double>? StrokeDashArray { get; }
    public Anchor(
        PhysicalMonitor monitor,
        double pos,
        IBrush brush,
        AvaloniaList<double>? strokeDashArray
        )
    {
        Monitor = monitor;
        Pos = pos;
        Brush = brush;
        StrokeDashArray = strokeDashArray;
    }
}
