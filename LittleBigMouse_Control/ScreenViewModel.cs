using System.Windows;
using System.Windows.Controls;
using LbmScreenConfig;
using NotifyChange;

namespace LittleBigMouse_Control
{
    public class ScreenViewModel : ViewModel
    {
        public ViewModel Control => GetProperty<ViewModel>();
        public ViewModel Control_default => NewControl;
        public virtual ViewModel NewControl => null;

        public Screen Screen
        {
            get { return GetProperty<Screen>(); }
            set { SetAndWatch(value); }
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
