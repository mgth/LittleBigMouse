using HLab.Mvvm.Annotations;

namespace LittleBigMouse.Plugins;

public interface IMonitorFrameViewModel : IViewModel<DisplayLayout.Monitor>
{
    IMonitorsLayoutPresenterViewModel? MonitorsPresenter { get; set; }
}