using System.ComponentModel;
using Hlab.Mvvm;
using Hlab.Notify;
using LbmScreenConfig;

namespace LittleBigMouse.LocationPlugin.Plugins.Location.Anchors
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
