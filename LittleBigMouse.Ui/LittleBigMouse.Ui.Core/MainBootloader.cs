using HLab.Core.Annotations;
using HLab.Mvvm.Annotations;
using LittleBigMouse.Plugins;

namespace LittleBigMouse.Ui.Core;

public class MainBootloader(
        IMainService mainService, 
        IMvvmService mvvm,
        IApplicationUpdater updater
        ) : IBootloader
{
    public async Task LoadAsync(IBootContext bootstrapper)
    {
        if(bootstrapper.WaitingForService(mvvm)) return; 

        mainService.UpdateLayout();

        await mainService.StartNotifierAsync();
        
        // Check for update
        if (mainService.MonitorsLayout.Options.AutoUpdate)
            await updater.CheckUpdateAsync(false);

        // Show control
        if (!mainService.MonitorsLayout.Options.StartMinimized)
            await mainService.ShowControlAsync();
    }
}