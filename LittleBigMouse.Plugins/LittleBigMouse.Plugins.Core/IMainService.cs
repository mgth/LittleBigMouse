using HLab.Sys.Windows.Monitors;
using LittleBigMouse.DisplayLayout.Monitors;

namespace LittleBigMouse.Plugins;

public interface IMainService
{
    void UpdateLayout();

    ISystemMonitorsService MonitorsSet {get;}

    IMonitorsLayout MonitorsLayout {get;}

    Task StartNotifierAsync();

    Task ShowControlAsync();

    void AddControlPlugin(Action<IMainPluginsViewModel>? action);

}