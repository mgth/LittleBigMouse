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

        // A blind update check is not actually silent when a newer release exists: the
        // updater opens its window.  "Start minimized to tray" must suppress every
        // automatic startup window, not only the main configuration window (#549).
        if (options.AutoUpdate && !options.StartMinimized)
            await updater.CheckUpdateAsync(false);

        if (!options.StartMinimized)
            await mainService.ShowControlAsync();
        else
            Console.Error.WriteLine("Startup UI suppressed: running in the notification area.");

        return BootState.Completed;
    }
}
