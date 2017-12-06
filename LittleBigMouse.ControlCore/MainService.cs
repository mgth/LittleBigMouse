using System.Windows;
using Erp.Base;
using LbmScreenConfig;
using LittleBigMouse_Control;
using Erp.Mvvm;
using Plugin;

namespace LittleBigMouse.ControlCore
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
