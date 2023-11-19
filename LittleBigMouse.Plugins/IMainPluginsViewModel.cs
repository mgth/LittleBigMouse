using HLab.Mvvm.Annotations;

namespace LittleBigMouse.Plugins;

public interface IMainPluginsViewModel
{
    void AddButton(IUiCommand cmd);

    Type ContentViewMode { get; set; }

    object? Content { get; set; }

    void SetMonitorFrameViewMode<T>() where T : ViewMode
    {
        ContentViewMode = typeof(T);
    }

}