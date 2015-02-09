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
        private Point dragStartPosition;

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
                    oldPosition = Screen.FromUI(new Size(grid.ActualWidth, grid.ActualHeight), e.GetPosition(grid));
                    dragStartPosition = gui.Screen.PhysicalLocation;
                    moving = true;

                    // bring element to front so we can move it over the others
                    grid.Children.Remove(gui);
                    grid.Children.Add(gui);
                }
                else
                {
                    Point newPosition = Screen.FromUI(new Size(grid.ActualWidth,grid.ActualHeight), e.GetPosition(grid));

                    double left = dragStartPosition.X - oldPosition.X + newPosition.X;
                    double right = left+gui.Screen.PhysicalBounds.Width;

                    Point pNear = newPosition;
                    foreach (Screen s in Screen.AllScreens)
                    {
                        double minoffset = 10;
                        
                        double offset = s.PhysicalBounds.Right - left;
                        if (Math.Abs(offset) < minoffset)
                        {
                            pNear = new Point(newPosition.X + offset, newPosition.Y);
                            minoffset = Math.Abs(offset);
                        }

                        offset = s.PhysicalBounds.Left - left;
                        if (Math.Abs(offset) < minoffset)
                        {
                            pNear = new Point(newPosition.X + offset, newPosition.Y);
                            minoffset = Math.Abs(offset);
                        }
                    }

                    newPosition = pNear;
                    double top = dragStartPosition.Y - oldPosition.Y + newPosition.Y;
                    double bottom = top + gui.Screen.PhysicalBounds.Height;
                    foreach (Screen s in Screen.AllScreens)
                    {
                        double minoffset = 10;
                        double offset = s.PhysicalBounds.Bottom - top;
                        if (Math.Abs(offset) < minoffset)
                        {
                            pNear = new Point(newPosition.X , newPosition.Y + offset);
                            minoffset = Math.Abs(offset);
                        }

                        offset = s.PhysicalBounds.Bottom - bottom;
                        if (Math.Abs(offset) < minoffset)
                        {
                            pNear = new Point(newPosition.X, newPosition.Y + offset);
                            minoffset = Math.Abs(offset);
                        }

                    }
                    newPosition = pNear;

                    Point p = Screen.PhysicalToUI(
                        new Size(grid.ActualWidth, grid.ActualHeight),
                        new Point(
                            dragStartPosition.X - oldPosition.X + newPosition.X,
                            dragStartPosition.Y - oldPosition.Y + newPosition.Y
                            )
                        );

                    gui.Margin = new Thickness(
                        p.X,
                        p.Y,
                        0,
                        0);

                    //oldPosition = newPosition;
                }
            }
            else
            {
                if (moving)
                {
                    Point p = Screen.FromUI(new Size(grid.ActualWidth, grid.ActualHeight), new Point(gui.Margin.Left, gui.Margin.Top));

                    double xOffset = p.X - gui.Screen.PhysicalLocation.X;
                    double yOffset = p.Y - gui.Screen.PhysicalLocation.Y;

                    if (gui.Screen.Primary)
                    {

                        foreach (Screen s in Screen.AllScreens)
                        {
                            if (!s.Primary)
                            {
                                s.PhysicalLocation = new Point(s.PhysicalLocation.X - xOffset, s.PhysicalLocation.Y - yOffset);
                            }
                        }
                    }
                    else
                    {
                        gui.Screen.PhysicalLocation = new Point(gui.Screen.PhysicalLocation.X + xOffset, gui.Screen.PhysicalLocation.Y + yOffset);
                    }

                    //Screen.shrinkX();
                    moving = false;
                    ResizeAll();
                }
            }
        }

        private void Sgui_DragLeave(object sender, DragEventArgs e)
        {
            
        }


        private void Grid_SizeChanged(object sender, SizeChangedEventArgs e)
        {
            ResizeAll();
        }

        private void ResizeAll() 
        {
            foreach(UIElement element in grid.Children)
            {
                Rect all = Screen.PhysicalOverallBounds;


                ScreenGUI gui = element as ScreenGUI;
                if (gui!=null)
                {
                    gui.HorizontalAlignment = HorizontalAlignment.Left;
                    gui.VerticalAlignment = VerticalAlignment.Top;

                    Rect r = gui.Screen.ToUI(new Size(grid.ActualWidth,grid.ActualHeight));

                    gui.Margin = new Thickness(
                        r.X,
                        r.Y,
                        0, 0);

                    gui.Width = r.Width;
                    gui.Height = r.Height;
                }
            }
        }
    }
}
