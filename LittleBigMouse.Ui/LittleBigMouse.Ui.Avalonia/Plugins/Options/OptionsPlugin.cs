using System.Threading.Tasks;
using HLab.Core.Annotations;
using LittleBigMouse.Plugins;
using LittleBigMouse.Plugins.Avalonia;
using LittleBigMouse.Ui.Avalonia.Plugins.Debug;

namespace LittleBigMouse.Ui.Avalonia.Plugins.Options;

public class OptionsPlugin(IMainService mainService) : IBootloader
{
    public Task LoadAsync(IBootContext bootstrapper)
    {
        mainService.AddControlPlugin(c =>
            c.AddViewModeButton<MonitorDebugViewMode>(
                "options",
                "Icon/Options",
                "Options")
        );
        return Task.CompletedTask;
    }
}