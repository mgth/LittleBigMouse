using HLab.Mvvm.Annotations;
using LittleBigMouse.DisplayLayout.Monitors;

namespace LittleBigMouse.Plugins;

public interface IMonitorFrameViewModel : IViewModel<PhysicalMonitor>
{
    IMonitorsLayoutPresenterViewModel? MonitorsPresenter { get; set; }

    IFrameLocation Location { get; set; }

    public void Select()
    {
        if(MonitorsPresenter is {} presenter)
            presenter.SelectedMonitor = Model;
    }
}