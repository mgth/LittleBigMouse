using System.ComponentModel;
using Hlab.Mvvm;
using Hlab.Notify;
using LbmScreenConfig;

namespace LittleBigMouse.Control.Core.Plugins.Default
{
    class DefaultScreenViewModel : IViewModel<Screen>
    {
        public Screen Model => this.GetModel();
        public event PropertyChangedEventHandler PropertyChanged
        {
            add => this.Add(value);
            remove => this.Remove(value);
        }


        public DefaultScreenViewModel()
        {
            this.Subscribe();
        }
    }
}
