using System;
using System.Windows;

using HLab.Core.Annotations;
using HLab.Mvvm;
using HLab.Mvvm.Annotations;

using LittleBigMouse.DisplayLayout;
using LittleBigMouse.Plugins;

namespace LittleBigMouse.Control.Main
{
    public class MainBootloader : IBootloader
    {
        public MainBootloader(IMainService mainService )
        {
            _mainService = mainService;
        }

        private readonly IMainService _mainService;


        public void Load(IBootContext bootstrapper)
        {
            _mainService.StartNotifier();


        }

    }
}