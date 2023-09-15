using HLab.Core.Annotations;
using HLab.Mvvm.Annotations;
using LittleBigMouse.Plugins;

namespace LittleBigMouse.Ui.Core;

public class MainBootloader(
        IMainService mainService, 
        IMvvmService mvvm) : IBootloader
{
    public void Load(IBootContext bootstrapper)
    {
        if(bootstrapper.WaitingForService(mvvm)) return; 

        //_mainService.StartNotifier();
        mainService.ShowControlAsync();

    }

}