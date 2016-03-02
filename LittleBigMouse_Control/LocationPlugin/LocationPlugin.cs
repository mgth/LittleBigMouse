using System;
using System.Collections.Generic;
using System.ComponentModel;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using System.Windows;
using System.Windows.Controls;
using LbmScreenConfig;

namespace LittleBigMouse_Control.LocationPlugin
{
    class LocationPlugin : Plugin, IPluginButton
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

                        MainViewModel.Control = new LocationControlViewModel
                        {
                            Config = MainViewModel.Config,
                        };

                        MainViewModel.Presenter.GetScreenControlViewModel = 
                            screen => new LocationScreenViewModel
                            {
                                Plugin = this,
                                Screen = screen
                            };
                    }
                    else
                    {
                        if(gui!=null)
                        {
                            gui.MainGrid.Children.Remove(VerticalAnchors);
                            gui.MainGrid.Children.Remove(HorizontalAnchors);
                        }

                     }                    
                }
            }
        }
    }
}
