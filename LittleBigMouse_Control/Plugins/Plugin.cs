using NotifyChange;

namespace LittleBigMouse_Control.Plugins
{
    abstract class Plugin : Notifier
    {
        public MainViewModel MainViewModel
        {
            get { return GetProperty<MainViewModel>(); }
            set { SetProperty(value); }
        }

        public abstract bool Init();
    }
}
