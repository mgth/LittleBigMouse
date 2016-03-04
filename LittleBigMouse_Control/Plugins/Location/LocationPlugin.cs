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

namespace LittleBigMouse_Control.PluginLocation
{
    class LocationPlugin : Plugin, IPluginButton, IPluginScreenControl
    {
        private bool _isActivated;

        public override bool Init()
        {
            MainViewModel.AddButton(this);
            return true;
        }

        public string Caption => "Location";

        public Grid VerticalAnchors { get; } = new Grid();
        public Grid HorizontalAnchors { get; } = new Grid();

        public bool IsActivated
        {
            get { return _isActivated; }
            set
            {
                if (SetProperty(ref _isActivated, value))
                {
                    MultiScreensView gui = MainViewModel.Presenter.View as MultiScreensView;
                    if (value)
                    {
                        if (gui != null)
                        {
                            gui.MainGrid.Children.Add(VerticalAnchors);
                            gui.MainGrid.Children.Add(HorizontalAnchors);
                        }

                        _control = new LocationControlViewModel
                        {
                            Config = MainViewModel.Config,
                        };
                        MainViewModel.Control = _control;
                        MainViewModel.Presenter.ScreenControlGetter = this;
                    }
                    else
                    {
                        if (MainViewModel.Presenter.ScreenControlGetter == this)
                            MainViewModel.Presenter.ScreenControlGetter = null;

                        if(gui!=null)
                        {
                            gui.MainGrid.Children.Remove(VerticalAnchors);
                            gui.MainGrid.Children.Remove(HorizontalAnchors);
                        }

                        if (MainViewModel.Control == _control)
                        {
                            _control = null;
                            MainViewModel.Control = _control;
                        }

                     }                    
                }
            }
        }

        private LocationControlViewModel _control = null;

        ScreenControlViewModel IPluginScreenControl.GetScreenControlViewModel(Screen screen) => new LocationScreenViewModel
            {
                Plugin = this,
                Screen = screen
            };
    }
}
