using System.Windows.Controls;
using HLab.Mvvm.Extensions;

namespace LittleBigMouse.Plugins.Wpf
{
    public static class ViewScreenFrameLayerExtension
    {
        public static IScreenFrameView GetFrame<T>(this T layer)
            where T : UserControl, IViewScreenFrameLayer
            => layer.FindVisualParent<IScreenFrameView>();

    }
}