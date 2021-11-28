using System.Windows;

namespace LittleBigMouse.ScreenConfig.Dimensions
{
    public static class ScreenRatioExt
    {
        public static IScreenSize Scale(this IScreenSize source, IScreenRatio ratio) => new ScreenScale(source, ratio);
        public static IScreenSize ScaleWithLocation(this IScreenSize source, IScreenRatio ratio) => new ScreenScaleWithLocation(source, ratio);
        public static IScreenSize Locate(this IScreenSize source, Point? point = null) => new ScreenLocate(source, point);
        public static IScreenSize Rotate(this IScreenSize source, int rotation) => new ScreenRotate(source, rotation);
        public static IScreenSize ScaleDip(this IScreenSize source, IScreenRatio effectiveDpi,ScreenConfig config) => new ScreenScaleDip(source, effectiveDpi, config);

        public static IScreenRatio Multiply(this IScreenRatio sourceA, IScreenRatio sourceB) => new ScreenRatioRatio(sourceA, sourceB);
        public static IScreenRatio Inverse(this IScreenRatio source) => new ScreenInverseRatio(source);
    }
}