using HLab.Mvvm.Annotations;

namespace LittleBigMouse.Plugins;

public interface IMainPluginsViewModel
{
    void AddButton(IUiCommand cmd);

    Type ContentViewMode { get; set; }
    IMainService MainService { get; set; }

    void SetMonitorFrameViewMode<T>() where T : ViewMode
    {
        ContentViewMode = typeof(T);
    }

}