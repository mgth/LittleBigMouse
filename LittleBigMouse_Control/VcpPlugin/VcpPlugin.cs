using System;
using System.Collections.Generic;
using System.ComponentModel;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using System.Windows;
using System.Windows.Controls;
using LbmScreenConfig;

namespace LittleBigMouse_Control.VcpPlugin
{
    public class VcpPlugin : Plugin<VcpPlugin>, IPluginButton
    {
        private bool _isActivated;


        public override bool Init()
        {
            AddButton(this);
            return true;
        }

 
        public string Caption => "VCP";

        public bool IsActivated
        {
            get { return _isActivated; }
            set
            {
                if (!Change.SetProperty(ref _isActivated, value)) return;

                if (value)
                {
                    MainGui.ControlGui = new ControlGuiVcp();
                    MainGui.GetScreenGuiControl = screen => new ScreenGuiVcp(screen);
                }
            }
        }
    }
}
