using System.Threading.Tasks;
using HLab.Core.Annotations;
using LittleBigMouse.Plugins;
using LittleBigMouse.Plugins.Avalonia;

namespace LittleBigMouse.Ui.Avalonia.Plugins.Debug;

public class MonitorDebugPlugin(IMainService mainService) : Bootloader
{
    public override Task<BootState> LoadAsync()
    {
        mainService.AddControlPlugin(c =>
            c.AddViewModeButton<MonitorDebugViewMode>(
                "info",
                "Icon/MonitorInfo",
                "Info")
        );
        return Task.FromResult(BootState.Completed);
    }
}