using NotifyChange;

namespace LittleBigMouse_Control.Plugins
{
    abstract class Plugin : Notifier
    {
        private MainViewModel _mainViewModel;

        public MainViewModel MainViewModel
        {
            get { return _mainViewModel; }
            set { SetProperty(ref _mainViewModel, value); }
        }

        public abstract bool Init();
    }
}
