using System.Collections.Generic;
using System.Windows.Controls;
using LbmScreenConfig;
using LittleBigMouse_Control.LocationPlugin;
using LittleBigMouse_Control.SizePlugin;
using LittleBigMouse_Control.VcpPlugin;
using NotifyChange;

namespace LittleBigMouse_Control.SizePlugin
{
    class BordersPlugin : Plugin, IPluginButton
    {
        private bool _isActivated;

        public override bool Init()
        {
            MainViewModel?.AddButton(this);
            return true;
        }

        public string Caption => "Size";

        public bool IsActivated
        {
            get { return _isActivated; }
            set
            {
                if (SetProperty(ref _isActivated, value))
                {
                    if (value)
                    {
                        MainViewModel.Presenter.GetScreenControlViewModel = 
                            screen => new SizeViewModel
                            {
                                Screen = screen
                            };

                        MainViewModel.Control = new LocationControlViewModel
                        {
                            Config = MainViewModel.Config,
                        };
                    }
                }
            }
        }
    }
}
