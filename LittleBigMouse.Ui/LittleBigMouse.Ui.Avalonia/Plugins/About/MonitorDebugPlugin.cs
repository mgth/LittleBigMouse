using System.Threading.Tasks;
using HLab.Core.Annotations;
using LittleBigMouse.Plugins;
using LittleBigMouse.Plugins.Avalonia;

namespace LittleBigMouse.Ui.Avalonia.Plugins.About;

public class AboutMonitorPlugin(IMainService mainService) : Bootloader
{
    public override Task<BootState> LoadAsync()
    {
        mainService.AddControlPlugin(c =>
            c.AddViewModeButton<MonitorAboutViewMode>(
                "about",
                "Icon/MonitorAbout",
                "About")
        );
        return Task.FromResult(BootState.Completed);
    }
}