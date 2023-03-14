using LittleBigMouse.DisplayLayout.Dimensions;

namespace LittleBigMouse.Plugins;

public interface IMonitorsLayoutPresenterViewModel : IPresenterViewModel
{
    IDisplayRatio VisualRatio { get; }
}