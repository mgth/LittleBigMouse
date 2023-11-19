
using Avalonia;

namespace LittleBigMouse.DisplayLayout.Dimensions;

public static class DisplaySizeExtensions
{
    //public static T Get<T>(this ObservableAsPropertyHelper<T> @this) => @this.Value;

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

    public static T Set<T>(this T @this, Size size) where T : IDisplaySize
    {
        using (@this.DelayChangeNotifications())
        {
            @this.Width = size.Width;
            @this.Height = size.Height;
            return @this;
        }
    }

    public static T Set<T>(this T @this, Rect rect) where T : IDisplaySize
    {
        using (@this.DelayChangeNotifications())
        {
            @this.Width = rect.Width;
            @this.Height = rect.Height;
            @this.X = rect.X;
            @this.Y = rect.Y;
            return @this;
        }
    }


}
