using System.ComponentModel;
using System.Windows;
using System.Windows.Controls;

namespace NotifyChange
{
    public class NotifyUserControl : UserControl, INotifyPropertyChanged
    {
        protected readonly NotifierHelper Notifier;
        public event PropertyChangedEventHandler PropertyChanged { add { Notifier.Add(value); } remove { Notifier.Remove(value); } }

        public NotifyUserControl()
        {
            Notifier = new NotifierHelper(this);
        }
     }

    public class NotifyWindow : Window, INotifyPropertyChanged
    {
        protected readonly NotifierHelper Notifier;
        public event PropertyChangedEventHandler PropertyChanged { add { Notifier.Add(value); } remove { Notifier.Remove(value); } }

        public NotifyWindow()
        {
            Notifier = new NotifierHelper(this);
        }
    }

}
