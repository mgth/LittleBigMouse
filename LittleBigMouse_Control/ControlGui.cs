using System;
using System.Collections.Generic;
using System.ComponentModel;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using System.Windows.Controls;
using LbmScreenConfig;
using NotifyChange;

namespace LittleBigMouse_Control
{
    public class ControlGui : NotifyUserControl
    {
        public ScreenConfig Config => MainGui.Instance.Config;
        public MainGui MainGui => MainGui.Instance ;



    }
}
