using HLab.Mvvm.Annotations;
using LittleBigMouse.DisplayLayout.Monitors;

namespace LittleBigMouse.Plugins;

public interface IMainPluginsViewModel
{
    ILayoutOptions Options { get; }

    void AddButton(IUiCommand cmd);

    Type ContentViewMode { get; set; }
    Type PresenterViewMode { get; }

    IMainService? MainService { get; set; }

    void SetMonitorFrameViewMode<T>() where T : ViewMode
    {
        ContentViewMode = typeof(T);
    }

}