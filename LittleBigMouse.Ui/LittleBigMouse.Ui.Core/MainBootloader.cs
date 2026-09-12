using HLab.Core.Annotations;
using HLab.Mvvm.Annotations;
using LittleBigMouse.Plugins;

namespace LittleBigMouse.Ui.Core;

public class MainBootloader(
        IMainService mainService,
        IMvvmService mvvm,
        IApplicationUpdater updater
        ) : Bootloader
{
    public override async Task<BootState> LoadAsync()
    {
        if (WaitingForServices(mvvm)) return BootState.Requeue;

        mainService.UpdateLayout();

        await mainService.StartNotifierAsync();

        var options = mainService.MonitorsLayout.Options;
        Console.Error.WriteLine(
            $"Startup options: StartMinimized={options.StartMinimized}, AutoUpdate={options.AutoUpdate}");

        if (options.AutoUpdate) await updater.CheckUpdateAsync(false);

        // The window always opens (v6): this process is a frontend the user just launched,
        // it has no tray icon to sit behind, and nothing else to be. "Start minimized" is
        // about what runs at login, which is the agent — it brings up the notification area
        // on its own and never opens this window. Suppressing the window here would leave a
        // process the user cannot see, cannot reach and cannot stop (the shape of #589).
        await mainService.ShowControlAsync();

        return BootState.Completed;
    }
}
