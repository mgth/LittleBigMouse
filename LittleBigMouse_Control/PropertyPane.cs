using System.ComponentModel;
using LbmScreenConfig;

namespace LittleBigMouse_Control
{
    public interface IPropertyPane : INotifyPropertyChanged
    {
        Screen Screen { get; set; }
    }
}