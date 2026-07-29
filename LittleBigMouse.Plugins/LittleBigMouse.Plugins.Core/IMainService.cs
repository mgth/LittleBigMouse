using LittleBigMouse.DisplayLayout.Monitors;

namespace LittleBigMouse.Plugins;

public interface IMainService
{
    void UpdateLayout();

    /// <summary>
    /// Rebuild the layout from the machine's actual displays, dropping any virtual
    /// layout currently shown — including one forced by LBM_VIRTUAL_LAYOUT, which is
    /// ignored from this call on (until the app restarts).
    /// </summary>
    void ReloadSystemLayout();

    IMonitorsLayout MonitorsLayout {get; set;}

    Task StartNotifierAsync();

    Task ShowControlAsync();

    void AddControlPlugin(Action<IMainPluginsViewModel>? action);

}