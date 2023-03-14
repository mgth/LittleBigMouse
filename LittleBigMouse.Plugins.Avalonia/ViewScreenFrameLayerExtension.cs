using Avalonia;
using Avalonia.Controls;

namespace LittleBigMouse.Plugins.Avalonia
{
    public static class ViewScreenFrameLayerExtension
    {
        public static IMonitorFrameView GetFrame<T>(this T @this)
            where T : UserControl, IMonitorFrameLayerViewClass
        {
            StyledElement? c = @this;
            while(c is not null && c is not IMonitorFrameView)
                c = (StyledElement)c.Parent;

            return c as IMonitorFrameView;
        }
    }
}