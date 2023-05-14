using System.Windows.Input;
using HLab.Mvvm.Annotations;
using LittleBigMouse.DisplayLayout.Monitors;

namespace LittleBigMouse.Plugins;

public interface IMainPluginsViewModel
{
    void AddButton(string id, string iconPath, string toolTypeText, ICommand cmd);
    Type MonitorFrameViewMode { get; set; }

    IMonitorsLayout Layout { get; set; }

    void SetMonitorFrameViewMode<T>()
    {
        MonitorFrameViewMode = typeof(T);
    }

}