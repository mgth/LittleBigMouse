using Erp.Notify;
using LittleBigMouse_Control.Plugins;

namespace LittleBigMouse_Control
{
    internal abstract class PresenterViewModel : ViewModel
    {

        public MainViewModel MainViewModel
        {
            get => this.Get<MainViewModel>(); set => this.Set(value);
        }

        private IPluginScreenControl _screenControlGetter;
        public IPluginScreenControl ScreenControlGetter
        {
            get => this.Get<IPluginScreenControl>(); set => this.Set(value);
        }

        public double Ratio
        {
            get => this.Get<double>(); protected set => this.Set(value);
        }
        public abstract double PhysicalToUiX(double x);
        public abstract double PhysicalToUiY(double y);
    }
}
