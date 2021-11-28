using System;
using System.Windows;

namespace LittleBigMouse.ScreenConfig.Dimensions
{
    public interface IScreenSize : IEquatable<IScreenSize>
    {
        double Width { get; set; }
        double Height { get; set; }
        double X { get; set; }
        double Y { get; set; }
        double TopBorder { get; set; }
        double BottomBorder { get; set; }
        double LeftBorder { get; set; }
        double RightBorder { get; set; }

        Rect Bounds { get; }
        Point Center { get; }

        Rect OutsideBounds { get; }
        double OutsideWidth { get; }
        double OutsideHeight { get; }
        double OutsideX { get; }
        double OutsideY { get; }

        Point Location { get; }
    }
}