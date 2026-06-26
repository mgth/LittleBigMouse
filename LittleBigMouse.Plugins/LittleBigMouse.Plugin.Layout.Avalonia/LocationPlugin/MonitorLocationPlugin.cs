using HLab.Core.Annotations;
using LittleBigMouse.Plugins;
using LittleBigMouse.Plugins.Avalonia;

namespace LittleBigMouse.Plugin.Layout.Avalonia.LocationPlugin;

public class MonitorLocationPlugin(IMainService mainService) : Bootloader
{
    public override Task<BootState> LoadAsync()
    {
        mainService.AddControlPlugin(c =>
            c.AddViewModeButton<MonitorLocationViewMode>(
                "location",
                "Icon/MonitorLocation",
                "Location"

        ));
        return Task.FromResult(BootState.Completed);
    }

}