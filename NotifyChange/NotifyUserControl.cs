using System.ComponentModel;
using System.Windows.Controls;

namespace NotifyChange
{
    public class NotifyUserControl : UserControl, INotifyPropertyChanged
    {
            // PropertyChanged Handling
            protected readonly PropertyChangedHelper Change;
            public event PropertyChangedEventHandler PropertyChanged { add { Change.Add(this, value); } remove { Change.Remove(value); } }

            public NotifyUserControl()
            {
                Change = new PropertyChangedHelper(this);
            }
     }
}
