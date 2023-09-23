using System.ComponentModel;
using System.Windows.Input;
using LittleBigMouse.DisplayLayout.Dimensions;
using LittleBigMouse.DisplayLayout.Monitors;

namespace LittleBigMouse.Plugins;

public interface IMonitorsLayoutPresenterViewModel : INotifyPropertyChanged
{
    IMainPluginsViewModel MainViewModel { get; }

    IDisplayRatio VisualRatio { get; }

    IMonitorsLayout Model { get; }

    IMonitorFrameViewModel? SelectedMonitor { get; set; }

    public ICommand ResetLocationsFromSystem { get; }
    public ICommand ResetSizesFromSystem { get; }

}