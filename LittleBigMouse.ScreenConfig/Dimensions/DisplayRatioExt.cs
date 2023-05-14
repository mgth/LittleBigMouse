using Avalonia;
using LittleBigMouse.DisplayLayout.Monitors;

namespace LittleBigMouse.DisplayLayout.Dimensions;

public static class DisplayExtensions
{
    public static IDisplaySize Scale(this IDisplaySize source, IDisplayRatio ratio) => new DisplayScale(source, ratio);
    public static IDisplaySize ScaleWithLocation(this IDisplaySize source, IDisplayRatio ratio) => new DisplayScaleWithLocation(source, ratio);
    public static IDisplaySize Locate(this IDisplaySize source, Point? point = null) => new DisplayLocate(source, point);
    public static IDisplaySize Rotate(this IDisplaySize source, int rotation) => new DisplayRotate(source, rotation);
    public static IDisplaySize ScaleDip(this IDisplaySize source, IDisplayRatio effectiveDpi, IMonitorsLayout config) 
        => new DisplayScaleDip(source, effectiveDpi, config);

    public static IDisplayRatio Multiply(this IDisplayRatio sourceA, IDisplayRatio sourceB) => new DisplayRatioRatio(sourceA, sourceB);
    public static IDisplayRatio Inverse(this IDisplayRatio source) => new DisplayInverseRatio(source);
}
