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

        // Check for update
        if (mainService.MonitorsLayout.Options.AutoUpdate)
            await updater.CheckUpdateAsync(false);

        // Show control
        if (!mainService.MonitorsLayout.Options.StartMinimized)
            await mainService.ShowControlAsync();

        return BootState.Completed;
    }
}