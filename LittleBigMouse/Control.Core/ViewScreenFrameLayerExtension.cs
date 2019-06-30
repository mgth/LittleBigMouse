using System.Windows.Controls;
using HLab.Mvvm.Wpf;

namespace LittleBigMouse.Control.Core
{
    public static class ViewScreenFrameLayerExtension
    {
        public static ScreenFrameView GetFrame<T>(this T layer)
            where T : UserControl, IViewScreenFrameLayer
            => layer.FindVisualParent<ScreenFrameView>();

    }
}