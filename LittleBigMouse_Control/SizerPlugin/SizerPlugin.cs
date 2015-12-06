using System;
using System.Collections.Generic;
using System.ComponentModel;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using System.Windows;
using System.Windows.Controls;
using LbmScreenConfig;

namespace LittleBigMouse_Control.SizerPlugin
{
    public class SizerPlugin : Plugin<SizerPlugin>, IPluginButton
    {
        private bool _isActivated;

        public override bool Init()
        {
            AddButton(this);
            return true;
        }


        public string Caption => "Sizer";

        public Grid VerticalAnchors { get; } = new Grid();
        public Grid HorizontalAnchors { get; } = new Grid();

        public bool IsActivated
        {
            get { return _isActivated; }
            set
            {
                if (Change.SetProperty(ref _isActivated, value))
                {
                    MultiScreensGui gui = MainGui.ScreensPresenter as MultiScreensGui;
                    if (value)
                    {

                        if (gui != null)
                        {
                            gui.MainGrid.Children.Add(VerticalAnchors);
                            gui.MainGrid.Children.Add(HorizontalAnchors);                           
                        }

                        MainGui.ControlGrid.Children.Clear();

                        MainGui.ControlGui = new ControlGuiSizer();
                        MainGui.GetScreenGuiControl = screen => new ScreenGuiSizer(screen);

                    }
                    else
                    {
                        if(gui!=null)
                        {
                            gui.MainGrid.Children.Remove(VerticalAnchors);
                            gui.MainGrid.Children.Remove(HorizontalAnchors);
                        }

                        MainGui.ControlGui = null;
                     }                    
                }
            }
        }
    }
}
