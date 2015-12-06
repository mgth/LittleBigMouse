using System.Collections.Generic;
using System.Windows.Controls;
using LbmScreenConfig;
using LittleBigMouse_Control.SizerPlugin;
using LittleBigMouse_Control.VcpPlugin;

namespace LittleBigMouse_Control.BordersPlugin
{
    public class BordersPlugin : Plugin<BordersPlugin>, IPluginButton
    {
        private bool _isActivated;


        public override bool Init()
        {
            AddButton(this);
            MainGui.GetScreenGuiControl = screen => new ScreenGuiVcp(screen);
            return true;
        }


        public string Caption => "Borders";


        public bool IsActivated
        {
            get { return _isActivated; }
            set
            {
                if (Change.SetProperty(ref _isActivated, value))
                {
                    if (value)
                    {
                        MainGui.ControlGrid.Children.Clear();

                        MainGui.ControlGui = new ControlGuiSizer();

                    }
                    else
                    {
 //                       if (MainGui.ScreenGuiPlugin == this)
                        {
 //                           MainGui.ScreenGuiPlugin = null;
                            MainGui.ControlGui = null;
                        }
                    }                    
                }
            }
        }
    }
}
