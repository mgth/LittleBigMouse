using Avalonia.Controls;

namespace LittleBigMouse.Plugins.Avalonia;

public interface IMonitorsLayoutPresenterView
{
    Panel MainPanel { get; }
    Panel BackPanel { get; }
    
    IMonitorsLayoutPresenterViewModel? ViewModel { get; }

    double GetRatio();
}