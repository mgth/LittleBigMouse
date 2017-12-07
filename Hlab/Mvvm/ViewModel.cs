using System.ComponentModel;
using Hlab.Notify;

namespace Hlab.Mvvm
{
    public class ViewModel : INotifyPropertyChanged
    {
        public event PropertyChangedEventHandler PropertyChanged
        {
            add => this.Add(value);
            remove => this.Remove(value);
        }
    }
}
