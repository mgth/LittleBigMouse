using LittleBigMouse.DisplayLayout.Dimensions;
using LittleBigMouse.DisplayLayout.Monitors;

namespace LittleBigMouse.Plugins;

public interface IMonitorsLayoutPresenterViewModel : IPresenterViewModel
{
    IDisplayRatio VisualRatio { get; }

    IMonitorsLayout Model { get; }

    IMonitorFrameViewModel? SelectedMonitor { get; set; }
}