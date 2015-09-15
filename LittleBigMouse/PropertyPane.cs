using System;
using System.Collections.Generic;
using System.ComponentModel;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using System.Windows.Controls;

namespace LittleBigMouse
{
    public interface IPropertyPane : INotifyPropertyChanged
    {
        Screen Screen { get; set; }
    }
}
