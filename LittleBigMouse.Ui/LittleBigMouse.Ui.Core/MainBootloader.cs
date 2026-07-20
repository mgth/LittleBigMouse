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

        // Show control
        if (!mainService.MonitorsLayout.Options.StartMinimized)
            await mainService.ShowControlAsync();

        // Update discovery must never delay or fault normal startup. The updater
        // has its own short network timeout; this boundary also contains any
        // implementation failure that escapes it.
        if (mainService.MonitorsLayout.Options.AutoUpdate)
            _ = CheckUpdateSafelyAsync(updater);

        return BootState.Completed;
    }

    public static async Task CheckUpdateSafelyAsync(IApplicationUpdater updater)
    {
        try { await updater.CheckUpdateAsync(false); }
        catch (Exception error) { System.Diagnostics.Debug.WriteLine(error); }
    }
}
