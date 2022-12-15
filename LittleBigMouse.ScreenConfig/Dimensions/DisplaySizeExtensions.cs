
using Avalonia;
using ReactiveUI;

namespace LittleBigMouse.DisplayLayout.Dimensions;

public static class DisplaySizeExtensions
{
    public static T Get<T>(this ObservableAsPropertyHelper<T> @this) => @this.Value;

    public static Point GetPoint(this IDisplaySize sz, IDisplaySize source, Point point)
    {
        var x = (point.X - source.X) / source.Width;
        var y = (point.Y - source.Y) / source.Height;

        return new Point(sz.X + x * sz.Width, sz.Y + y * sz.Height);
    }
    public static Point Inside(this IDisplaySize sz, Point p)
    {
        var x = p.X < sz.X ? sz.X : p.X > sz.Bounds.Right - 1 ? sz.Bounds.Right - 1 : p.X;
        var y = p.Y < sz.Y ? sz.Y : p.Y > sz.Bounds.Bottom - 1 ? sz.Bounds.Bottom - 1 : p.Y;

        return new Point(x, y);
    }
}
