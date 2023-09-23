
using System.Windows.Media;
using LittleBigMouse.DisplayLayout;

namespace LittleBigMouse.Plugin.Location.Plugins.Location.Anchors;

public class Anchor
{
    public Monitor Screen { get; }
    public double Pos { get; }
    public Brush Brush { get; }
    public DoubleCollection StrokeDashArray { get; }
    public Anchor(Monitor screen, double pos, Brush brush, DoubleCollection strokeDashArray)
    {
        Screen = screen;
        Pos = pos;
        Brush = brush;
        StrokeDashArray = strokeDashArray;
    }
}
