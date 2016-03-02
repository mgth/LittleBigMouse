using System;
using System.Collections.Generic;
using System.ComponentModel;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Controls.Primitives;
using System.Windows.Data;
using LbmScreenConfig;
using LittleBigMouse_Control.Properties;
using NotifyChange;

namespace LittleBigMouse_Control
{
    abstract class Plugin : Notifier
    {
        private MainViewModel _mainViewModel;

        public MainViewModel MainViewModel
        {
            get { return _mainViewModel; }
            set { SetProperty(ref _mainViewModel, value); }
        }

        public abstract bool Init();
    }
}
