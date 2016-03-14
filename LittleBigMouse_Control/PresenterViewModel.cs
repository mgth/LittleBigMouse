using LittleBigMouse_Control.Plugins;

namespace LittleBigMouse_Control
{
    internal abstract class PresenterViewModel : ViewModel
    {

        private MainViewModel _mainViewModel;
        public MainViewModel MainViewModel
        {
            get { return _mainViewModel; }
            set { SetProperty(ref _mainViewModel, value); }
        }

        private IPluginScreenControl _screenControlGetter;
        public IPluginScreenControl ScreenControlGetter
        {
            get { return _screenControlGetter; }
            set { SetProperty(ref _screenControlGetter, value); }
        }

        private double _ratio;
        public double Ratio
        {
            get { return _ratio; }
            protected set { SetProperty(ref _ratio, value); }
        }
        public abstract double PhysicalToUiX(double x);
        public abstract double PhysicalToUiY(double y);
    }
}
