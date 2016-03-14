using System.Windows;
using System.Windows.Controls;
using LbmScreenConfig;
using NotifyChange;

namespace LittleBigMouse_Control
{
    public class ScreenViewModel : ViewModel
    {
        private ViewModel _control = null;
        public ViewModel Control => _control??(_control=NewControl);
        public virtual ViewModel NewControl => null;

        private Screen _screen;
        public Screen Screen
        {
            get { return _screen; }
            set { SetAndWatch(ref _screen, value); }
        }

        [DependsOn("Screen")]
        private void WatchConfig()
        {
            if (Screen?.Config != null)
                Watch(Screen.Config,"Config");

            if (Screen?.Monitor != null)
                Watch(Screen.Monitor,"Monitor");
        }


        private bool _power = true;
        public Viewbox PowerButton => _power
            ? (Viewbox)Application.Current.FindResource("LogoPowerOn")
            : (Viewbox)Application.Current.FindResource("LogoPowerOff");
    }
}
