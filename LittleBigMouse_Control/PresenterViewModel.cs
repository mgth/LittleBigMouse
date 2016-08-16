using LittleBigMouse_Control.Plugins;

namespace LittleBigMouse_Control
{
    internal abstract class PresenterViewModel : ViewModel
    {

        public MainViewModel MainViewModel
        {
            get { return GetProperty<MainViewModel>(); }
            set { SetProperty( value); }
        }

        private IPluginScreenControl _screenControlGetter;
        public IPluginScreenControl ScreenControlGetter
        {
            get { return GetProperty<IPluginScreenControl>(); }
            set { SetProperty(value); }
        }

        public double Ratio
        {
            get { return GetProperty<double>(); }
            protected set { SetProperty(value); }
        }
        public abstract double PhysicalToUiX(double x);
        public abstract double PhysicalToUiY(double y);
    }
}
