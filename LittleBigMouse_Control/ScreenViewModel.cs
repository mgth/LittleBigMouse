using System.Windows;
using System.Windows.Controls;
using Erp.Notify;
using LbmScreenConfig;

namespace LittleBigMouse_Control
{
    public class ScreenViewModel : ViewModel
    {
        public ViewModel Control => this.Get(()=>NewControl);
        public virtual ViewModel NewControl => null;

        public Screen Screen
        {
            get => this.Get<Screen>(); set => this.Set(value);
        }



        private bool _power = true;
        public Viewbox PowerButton => _power
            ? (Viewbox)Application.Current.FindResource("LogoPowerOn")
            : (Viewbox)Application.Current.FindResource("LogoPowerOff");
    }
}
