using System;
using System.ComponentModel;

namespace LittleBigMouse.Control.Core
{
    public interface IPresenterViewModel : INotifyPropertyChanged
    {

        MainViewModel MainViewModel { get; set; }
        Type ViewMode { get; set; }

        //private IPluginScreenControl _screenControlGetter;
        //public IPluginScreenControl ScreenControlGetter
        //{
        //    get => this.Get<IPluginScreenControl>(); set => this.Set(value);
        //}

        //double Ratio { get; }
        //double PhysicalToUiX(double x);
        //double PhysicalToUiY(double y);
    }
}
