using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Data;
using System.Windows.Documents;
using System.Windows.Input;
using System.Windows.Media;
using System.Windows.Media.Imaging;
using System.Windows.Navigation;
using System.Windows.Shapes;
using LbmScreenConfig;

namespace LittleBigMouse_Control.BordersPlugin
{
    /// <summary>
    /// Logique d'interaction pour ScreenGuiBorders.xaml
    /// </summary>
    public partial class ScreenGuiBorders : ScreenGuiControl<ScreenGuiBorders>
    {
        public ScreenGuiBorders(Screen screen):base(screen)
        {
            InitializeComponent();
        }
    }
}
