using Erp.Base;
using LittleBigMouse.ControlCore;
using Erp.Mvvm;
using Plugin;

namespace LittleBigMouse.LocationPlugin.Plugins.Vcp
{
    class ViewModeScreenVcp : ViewMode { }

    class VcpPlugin : PluginModule<VcpPlugin>
    {
        public override void Register()
        {
#if DEBUG
            MainService.D.MainViewModel.AddButton("VCP",
                () => MainService.D.MainViewModel.Presenter.ViewMode = typeof(ViewModeScreenVcp),
                () => MainService.D.MainViewModel.Presenter.ViewMode = typeof(ViewModeDefault));
#endif
        }
    }
}
