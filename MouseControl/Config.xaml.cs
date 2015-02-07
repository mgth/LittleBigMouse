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
using System.Windows.Shapes;

namespace MouseControl
{
    /// <summary>
    /// Interaction logic for Config.xaml
    /// </summary>
    public partial class Config : Window
    {
        public Config()
        {
            InitializeComponent();
            foreach(Screen s in Screen.AllScreens)
            {
                ScreenGUI sgui = new ScreenGUI { Screen = s };
                grid.Children.Add(sgui);
            }

            grid.SizeChanged += Grid_SizeChanged;
        }

        private void Grid_SizeChanged(object sender, SizeChangedEventArgs e)
        {
            foreach(UIElement element in grid.Children)
            {
                Rect all = Screen.OverallBounds;

                double xRatio = grid.ActualWidth / all.Width;
                double yRatio = grid.ActualHeight / all.Height;

                ScreenGUI gui = element as ScreenGUI;
                if (gui!=null)
                {
                    gui.HorizontalAlignment = HorizontalAlignment.Left;
                    gui.VerticalAlignment = VerticalAlignment.Top;

                    gui.Margin = new Thickness(
                        (gui.Screen.Bounds.Left - all.Left) * xRatio,
                        (gui.Screen.Bounds.Top - all.Top) * yRatio,
                        0, 0);
                    gui.Width = gui.Screen.Bounds.Width * xRatio;
                    gui.Height = gui.Screen.Bounds.Height * yRatio;
                }
            }
        }
    }
}
