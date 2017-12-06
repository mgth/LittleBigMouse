using System.Collections.Generic;
using System.ComponentModel;
using System.Linq;
using System.Windows;
using System.Windows.Media;
using Erp.Mvvm;
using Erp.Notify;
using LbmScreenConfig;
using LittleBigMouse.LocationPlugin.Plugins.Location;
using LittleBigMouse_Control.Rulers;

namespace LittleBigMouse.LocationPlugin.Plugins.Default
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
