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
                sgui.DragLeave += Sgui_DragLeave;
                sgui.MouseMove += Sgui_MouseMove;
            }

            grid.SizeChanged += Grid_SizeChanged;
        }

        private Point oldPosition;
        bool moving = false;
        private void Sgui_MouseMove(object sender, MouseEventArgs e)
        {
            ScreenGUI gui = sender as ScreenGUI;
            if (sender == null) return;

            label.Content=gui.Screen.DeviceName;
            if (e.LeftButton == MouseButtonState.Pressed)
            {
                if (moving == false)
                {
                    oldPosition = e.GetPosition(grid);
                    moving = true;
                }
                else
                {
                    Point newPosition = e.GetPosition(grid);

                    gui.Margin = new Thickness(
                        gui.Margin.Left - oldPosition.X + newPosition.X,
                        gui.Margin.Top - oldPosition.Y + newPosition.Y,
                        0,
                        0);

                    oldPosition = newPosition;
                }
            }
            else
            {
                if (moving)
                {
                    gui.Screen.PhysicalLocation = new Point((double)gui.Margin.Left / xRatio, (double)gui.Margin.Top / yRatio);
                    Screen.shrinkX();
                    moving = false;
                }
            }
        }

        private void Sgui_DragLeave(object sender, DragEventArgs e)
        {
            
        }

        private double xRatio = 1.0;
        private double yRatio = 1.0;

        private void Grid_SizeChanged(object sender, SizeChangedEventArgs e)
        {
            foreach(UIElement element in grid.Children)
            {
                Rect all = Screen.PhysicalOverallBounds;

                xRatio = grid.ActualWidth / all.Width;
                yRatio = grid.ActualHeight / all.Height;

                ScreenGUI gui = element as ScreenGUI;
                if (gui!=null)
                {
                    gui.HorizontalAlignment = HorizontalAlignment.Left;
                    gui.VerticalAlignment = VerticalAlignment.Top;

                    gui.XRatio = xRatio;
                    gui.YRatio = yRatio;

                    gui.Margin = new Thickness(
                        (gui.Screen.PhysicalBounds.Left - all.Left) * xRatio,
                        (gui.Screen.PhysicalBounds.Top - all.Top) * yRatio,
                        0, 0);
                    gui.Width = gui.Screen.PhysicalBounds.Width * xRatio;
                    gui.Height = gui.Screen.PhysicalBounds.Height * yRatio;
                }
            }
        }
    }
}
