using System.Windows;

namespace LittleBigMouse.ScreenConfig.Dimensions
{
    public static class ScreenSizeExtensions
    {
        public static Point GetPoint(this IScreenSize sz, IScreenSize source, Point point)
        {
            var x = (point.X - source.X) / source.Width;
            var y = (point.Y - source.Y) / source.Height;

            return new Point(sz.X + x * sz.Width, sz.Y + y * sz.Height);
        }
        public static Point Inside(this IScreenSize sz, Point p)
        {
            var x = p.X < sz.X ? sz.X : (p.X > sz.Bounds.Right - 1) ? (sz.Bounds.Right - 1) : p.X;
            var y = p.Y < sz.Y ? sz.Y : (p.Y > sz.Bounds.Bottom - 1) ? (sz.Bounds.Bottom - 1) : p.Y;

            return new Point(x, y);
        }
    }
}