using HLab.Core.Annotations;
using HLab.Mvvm.Annotations;
using LittleBigMouse.Plugins;

namespace LittleBigMouse.Ui.Core;

public class MainBootloader(
        IMainService mainService, 
        IMvvmService mvvm) : IBootloader
{
    public Task LoadAsync(IBootContext bootstrapper)
    {
        if(bootstrapper.WaitingForService(mvvm)) return Task.CompletedTask; 

        mainService.StartNotifier();
        return mainService.ShowControlAsync();

    }

}