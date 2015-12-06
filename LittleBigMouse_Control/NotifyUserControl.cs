using System;
using System.Collections.Generic;
using System.ComponentModel;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Data;
using System.Windows.Input;
using LbmScreenConfig;

namespace LittleBigMouse_Control
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
