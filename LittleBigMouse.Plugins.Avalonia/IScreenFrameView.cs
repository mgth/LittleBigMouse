using Avalonia;

namespace LittleBigMouse.Plugins.Avalonia;

public interface IMonitorFrameView
{
    IMonitorFrameViewModel ViewModel { get; }
    Thickness Margin { get; set; }
    //double ActualHeight => Height;
    //double ActualWidth => Width;
    double Height { get; }
    double Width { get; }

}