using System;
using System.Windows;
using HLab.Core.Annotations;
using HLab.DependencyInjection.Annotations;
using HLab.Mvvm;
using HLab.Mvvm.Annotations;
using LittleBigMouse.Plugins;

namespace LittleBigMouse.Control.Main
{
    public class MainBootloader : IBootloader
    {
        [Import]
        public MainBootloader(IMainService mainService, IMvvmService mvvmService)
        {
            _mainService = mainService;
            _mvvmService = mvvmService;
        }

        private readonly IMainService _mainService;
        private readonly IMvvmService _mvvmService;

        [Import] private Func<ScreenConfig.ScreenConfig,MultiScreensViewModel> _getViewModel;

        public void Load(IBootContext bootstrapper)
        {
            if (_mainService is MainService service)
            {
                var viewModel = service.MainViewModel;

                viewModel.Config = service.Config;

                viewModel.Presenter = _getViewModel(service.Config);

                var view = (Window)_mvvmService.MainContext.GetView<ViewModeDefault>(viewModel, typeof(IViewClassDefault));

                view.Show();

            }
        }

    }
}