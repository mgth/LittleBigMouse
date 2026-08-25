using System.ComponentModel;
using System.Windows.Input;
using LittleBigMouse.DisplayLayout.Dimensions;
using LittleBigMouse.DisplayLayout.Monitors;
using LittleBigMouse.Zoning;

namespace LittleBigMouse.Plugins;

public interface IMonitorsLayoutPresenterViewModel : INotifyPropertyChanged
{
    IMainPluginsViewModel MainViewModel { get; }

    IMutableDisplayRatio VisualRatio { get; }

    IMonitorsLayout Model { get; }

    PhysicalMonitor? SelectedMonitor { get; set; }

    /// <summary>
    /// Latest edge-prober report from the daemon (null when none): the monitor frames
    /// render it as colored strips along their edges (wall / crossing).
    /// </summary>
    ProbeReport? ProbeReport { get; }

    public ICommand ResetLocationsFromSystem { get; }
    public ICommand ApplyLocationsToSystem { get; }

}
