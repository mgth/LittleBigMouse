using System.Windows;

namespace LittleBigMouse.Plugins.Wpf;

public interface IScreenFrameView
{
    IScreenFrameViewModel ViewModel { get; }
    Thickness Margin { get; set; }
    double ActualHeight { get; }
    double ActualWidth { get; }

}