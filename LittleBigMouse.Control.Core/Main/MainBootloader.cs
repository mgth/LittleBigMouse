using HLab.Core.Annotations;
using LittleBigMouse.Plugins;

namespace LittleBigMouse.Control.Main;

public class MainBootloader : IBootloader
{
    public MainBootloader(IMainService mainService )
    {
        _mainService = mainService;
    }

    readonly IMainService _mainService;


    public void Load(IBootContext bootstrapper)
    {
        _mainService.StartNotifier();


    }

}