using System.Windows;
using Hlab.Mvvm;
using Hlab.Plugin;
using LbmScreenConfig;

namespace LittleBigMouse.Control.Core
{
    public class MainService : PluginModule<MainService>
    {
        public MainViewModel MainViewModel { get; } = new MainViewModel();
        public override void Register()
        {
            var viewModel = D.MainViewModel;

            var config = new ScreenConfig();

            viewModel.Config = config;
            viewModel.Presenter = new  MultiScreensViewModel { Config = config };

            var view = (Window)MvvmService.D.MainViewModeContext.GetView<ViewModeDefault>(viewModel, typeof(IViewClassDefault));

            view.Show();
        }
    }
}
