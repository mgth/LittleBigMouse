using Hlab.Mvvm;
using Hlab.Plugin;
using LittleBigMouse.Control.Core;

namespace LittleBigMouse.Plugin.Vcp
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
