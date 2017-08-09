using System.ComponentModel;
using Erp.Notify;

namespace LittleBigMouse_Control.Plugins
{
    abstract class Plugin : INotifyPropertyChanged
    {
        public event PropertyChangedEventHandler PropertyChanged
        {
            add => this.Add(value);
            remove => this.Remove(value);
        }
        public MainViewModel MainViewModel
        {
            get => this.Get<MainViewModel>(); set => this.Set(value);
        }

        public abstract bool Init();
    }
}
