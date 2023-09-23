using Avalonia;

namespace LittleBigMouse.Plugins.Avalonia;

public interface IMonitorFrameView
{
    IMonitorFrameViewModel? ViewModel { get; }
    Thickness Margin { get; set; }

    Rect Bounds { get; }

    double Height { get; }
    double Width { get; }

}