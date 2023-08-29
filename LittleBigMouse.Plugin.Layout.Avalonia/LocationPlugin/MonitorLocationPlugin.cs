using HLab.Core.Annotations;
using HLab.Icons.Avalonia.Icons;
using LittleBigMouse.Plugins;
using LittleBigMouse.Plugins.Avalonia;

namespace LittleBigMouse.Plugin.Layout.Avalonia.LocationPlugin;

public class MonitorLocationPlugin : IBootloader
{
    readonly IMainService _mainService;

    public MonitorLocationPlugin(IMainService mainService, IIconService iconService)
    {
        _mainService = mainService;
    }

    public void Load(IBootContext bootstrapper)
    {
        _mainService.AddControlPlugin(c =>
            c.AddViewModeButton<MonitorLocationViewMode>(
                "location",
                "Icon/MonitorLocation",
                "Location"

        ));
    }

}