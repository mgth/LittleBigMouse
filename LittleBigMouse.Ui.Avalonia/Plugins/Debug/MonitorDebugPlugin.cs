using System.Reactive.Linq;
using System.Threading.Tasks;
using HLab.Core.Annotations;
using HLab.Mvvm.Annotations;
using HLab.Sys.Windows.Monitors;
using LittleBigMouse.Plugins;
using LittleBigMouse.Plugins.Avalonia;
using ReactiveUI;

namespace LittleBigMouse.Ui.Avalonia.Plugins.Debug;

public class MonitorDebugPlugin(IMainService mainService) : IBootloader
{
    public Task LoadAsync(IBootContext bootstrapper)
    {
        mainService.AddControlPlugin(c =>
            c.AddViewModeButton<MonitorDebugViewMode>(
                "info",
                "Icon/MonitorInfo",
                "Info")
        );
        return Task.CompletedTask;
    }
}