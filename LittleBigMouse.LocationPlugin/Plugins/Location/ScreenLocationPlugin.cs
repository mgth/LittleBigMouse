using System.Windows.Controls;
using Erp.Base;
using LittleBigMouse.ControlCore;
using Erp.Mvvm;
using Plugin;

namespace LittleBigMouse.LocationPlugin.Plugins.Location
{
    class ViewModeScreenLocation : ViewMode { }

    class ScreenLocationPlugin : PluginModule<ScreenLocationPlugin>
    {
        public override void Register()
        {
            MainService.D.MainViewModel.AddButton("Location",
                ()=> MainService.D.MainViewModel.Presenter.ViewMode = typeof(ViewModeScreenLocation),
                ()=> MainService.D.MainViewModel.Presenter.ViewMode = typeof(ViewModeDefault));
        }

    }
}
