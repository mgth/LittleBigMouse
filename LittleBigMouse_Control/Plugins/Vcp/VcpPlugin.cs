using Erp.Notify;
using LbmScreenConfig;
using LittleBigMouse_Control.VcpPlugin;

namespace LittleBigMouse_Control.Plugins.Vcp
{
    class VcpPlugin : Plugin, IPluginButton, IPluginScreenControl
    {
        private bool _isActivated;


        public override bool Init()
        {
            MainViewModel.AddButton(this);
            return true;
        }

 
        public string Caption => "VCP";

        public bool IsActivated
        {
            get => this.Get<bool>(); set
            {
                if (!this.Set(value)) return;

                if (value)
                {
                    MainViewModel.Presenter.ScreenControlGetter = this;

                    _control = new VcpControlViewModel();

                    MainViewModel.Control = _control;
                }
            }
        }
        private VcpControlViewModel _control = null;

        public ScreenControlViewModel GetScreenControlViewModel(Screen screen)
            => new VcpScreenViewModel { Screen = screen};
    }
}
