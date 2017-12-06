using System.ComponentModel;
using Erp.Mvvm;
using Erp.Notify;
using LbmScreenConfig;

namespace LittleBigMouse.LocationPlugin.Plugins.Location
{
    class AnchorsViewModel : IViewModel<ScreenConfig>
    {
        public AnchorsViewModel()
        {
            this.Subscribe();
        }

        public ScreenConfig Model => this.GetModel();

        public event PropertyChangedEventHandler PropertyChanged
        {
            add => this.Add(value);
            remove => this.Remove(value);
        }
    }
}
