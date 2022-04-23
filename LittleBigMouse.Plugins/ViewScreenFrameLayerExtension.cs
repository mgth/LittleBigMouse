using HLab.Mvvm.Extensions;

using System.Windows.Controls;

namespace LittleBigMouse.Plugins
{
    public static class ViewScreenFrameLayerExtension
    {
        public static IScreenFrameView GetFrame<T>(this T layer)
            where T : UserControl, IViewScreenFrameLayer
            => layer.FindVisualParent<IScreenFrameView>();

    }
}