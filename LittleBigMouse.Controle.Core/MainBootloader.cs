using HLab.Core.Annotations;
using HLab.Mvvm.Annotations;
using LittleBigMouse.Plugins;

namespace LittleBigMouse.Ui.Core;

public class MainBootloader : IBootloader
{
    public MainBootloader(IMainService mainService, IMvvmService mvvm)
    {
        _mainService = mainService;
        _mvvm = mvvm;
    }

    readonly IMainService _mainService;
    readonly IMvvmService _mvvm;


    public void Load(IBootContext bootstrapper)
    {
        if(bootstrapper.WaitService(_mvvm)) return; 

        //_mainService.StartNotifier();
        _mainService.ShowControl();

    }

}