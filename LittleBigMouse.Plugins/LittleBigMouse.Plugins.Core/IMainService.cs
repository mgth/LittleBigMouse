using HLab.Sys.Windows.Monitors;
using LittleBigMouse.DisplayLayout.Monitors;

namespace LittleBigMouse.Plugins;

public interface IMainService
{
    void UpdateLayout();

    IMonitorsLayout MonitorsLayout {get; set;}

    Task StartNotifierAsync();

    Task ShowControlAsync();

    void AddControlPlugin(Action<IMainPluginsViewModel>? action);

}