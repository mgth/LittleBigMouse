using HLab.Geo;
using LittleBigMouse.DisplayLayout.Monitors;

namespace LittleBigMouse.DisplayLayout.Dimensions;

public static class DisplayExtensions
{
    public static IMutableDisplaySize Scale(this IMutableDisplaySize source, IDisplayRatio ratio) => new DisplayScale(source, ratio);
    public static IMutableDisplaySize ScaleWithLocation(this IMutableDisplaySize source, IDisplayRatio ratio) => new DisplayScaleWithLocation(source, ratio);
    public static IMutableDisplaySize Locate(this IMutableDisplaySize source, Point? point = null) => new DisplayLocate(source, point);
    public static IMutableDisplaySize Rotate(this IMutableDisplaySize source, int rotation) => new DisplayRotate(source, rotation);
    public static IMutableDisplayBounds ScaleDip(this IMutableDisplayBounds source, IDisplayRatio effectiveDpi, IMonitorsLayout config)
        => new DisplayScaleDip(source, effectiveDpi, config);

    public static IDisplayRatio Multiply(this IDisplayRatio sourceA, IDisplayRatio sourceB) => new DisplayRatioRatio(sourceA, sourceB);
    public static IDisplayRatio Inverse(this IDisplayRatio source) => new DisplayInverseRatio(source);
}
