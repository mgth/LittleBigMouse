using System.Reactive.Linq;
using HLab.Core.Annotations;
using HLab.Icons.Avalonia.Icons;
using HLab.Mvvm.Annotations;
using LittleBigMouse.Plugins;
using ReactiveUI;

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
            c.AddButton(
                "location",
                "Icon/MonitorLocation",
                "Location",

                ReactiveCommand.Create<bool>(b =>
                    {
                        if (b)
                            c.SetMonitorFrameViewMode<MonitorLocationViewMode>();
                        else
                            c.SetMonitorFrameViewMode<DefaultViewMode>();
                    }
                    , outputScheduler: RxApp.MainThreadScheduler
                    , canExecute: Observable.Return(true)))
        );
    }

}