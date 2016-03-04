using System;
using System.Collections.Generic;
using System.ComponentModel;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using System.Windows;
using System.Windows.Controls;
using LbmScreenConfig;
using LittleBigMouse_Control.Plugins;

namespace LittleBigMouse_Control.VcpPlugin
{
    class VcpPlugin : Plugin, IPluginButton, IPluginScreenControl
    {
        private bool _isActivated;


        public override bool Init()
        {
            MainViewModel.AddButton(this);
            return true;
        }

 
        public string Caption => "VCP";

        public bool IsActivated
        {
            get { return _isActivated; }
            set
            {
                if (!SetProperty(ref _isActivated, value)) return;

                if (value)
                {
                    MainViewModel.Control = new VcpControlViewModel();
                    MainViewModel.Presenter.ScreenControlGetter = this;
                }
            }
        }

        public ScreenControlViewModel GetScreenControlViewModel(Screen screen)=> new ScreenControlViewModel { Screen =screen};
                        //TODO create VcpScreenControlViewModel
    }
}
