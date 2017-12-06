using Erp.Base;
using LittleBigMouse.ControlCore;
using Erp.Mvvm;
using Plugin;

namespace LittleBigMouse.LocationPlugin.Plugins.Size
{
    public class ViewModeScreenSize : ViewMode { }
    internal class ScreenSizePlugin : PluginModule<ScreenSizePlugin>
    {
        public override void Register()
        {
            MainService.D.MainViewModel.AddButton("Size",
                () => MainService.D.MainViewModel.Presenter.ViewMode = typeof(ViewModeScreenSize),
                () => MainService.D.MainViewModel.Presenter.ViewMode = typeof(ViewModeDefault));
        }
    }

}
