using Hlab.Mvvm;
using Hlab.Plugin;
using LittleBigMouse.Control.Core;

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
