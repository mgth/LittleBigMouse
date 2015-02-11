using Microsoft.Win32;
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
    public partial class FormConfig : Window
    {
        private ScreenConfig _config;
        public FormConfig(ScreenConfig config)
        {
            _config = config;

            InitializeComponent();

            RegistryKey rk = Registry.CurrentUser.OpenSubKey("SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Run", true);
            String startup = rk.GetValue(System.Windows.Forms.Application.ProductName, "").ToString();
            if (startup == System.Windows.Forms.Application.ExecutablePath.ToString())
                chkLoadAtStartup.IsChecked = true;
            else
                chkLoadAtStartup.IsChecked = false;

            foreach (Screen s in _config.AllScreens)
            {
                ScreenGUI sgui = new ScreenGUI(s,grid);
                grid.Children.Add(sgui);
                sgui.DragLeave += Sgui_DragLeave;
                sgui.MouseMove += Sgui_MouseMove;
                sgui.MouseLeftButtonDown += Sgui_MouseLeftButtonDown;
                sgui.MouseLeftButtonUp += Sgui_MouseLeftButtonUp;
            }

            grid.SizeChanged += Grid_SizeChanged;
        }

        private Point oldPosition;
        private Point dragStartPosition;
        private bool moving = false;

        private void Sgui_MouseLeftButtonUp(object sender, MouseButtonEventArgs e)
        {
            ScreenGUI gui = sender as ScreenGUI;
            if (sender == null) return;

            if (moving)
            {

                Point p = _config.FromUI(new Size(grid.ActualWidth, grid.ActualHeight), new Point(gui.Margin.Left, gui.Margin.Top));

                double xOffset = p.X - gui.Screen.PhysicalLocation.X;
                double yOffset = p.Y - gui.Screen.PhysicalLocation.Y;

                gui.Screen.PhysicalLocation = new Point(gui.Screen.PhysicalLocation.X + xOffset, gui.Screen.PhysicalLocation.Y + yOffset);

                moving = false;
                ResizeAll();

                foreach(UIElement el in grid.Children)
                {
                    ScreenGUI el_gui = el as ScreenGUI;
                    if (el_gui != null && el_gui!=gui) el_gui.HideSizers();
                }
                gui.ShowSizers();
            }
            else
            {
                foreach (UIElement el in grid.Children)
                {
                    ScreenGUI el_gui = el as ScreenGUI;
                    if (el_gui != null && el_gui != gui) el_gui.HideSizers();
                }
                gui.SwitchSizers();
            }
        }

        private void Sgui_MouseLeftButtonDown(object sender, MouseButtonEventArgs e)
        {
            ScreenGUI gui = sender as ScreenGUI;
            if (sender == null) return;

            oldPosition = _config.FromUI(new Size(grid.ActualWidth, grid.ActualHeight), e.GetPosition(grid));
            dragStartPosition = gui.Screen.PhysicalLocation;

            // bring element to front so we can move it over the others
            grid.Children.Remove(gui);
            grid.Children.Add(gui);
        }


        private void Sgui_MouseMove(object sender, MouseEventArgs e)
        {
            ScreenGUI gui = sender as ScreenGUI;
            if (sender == null) return;

            if (e.LeftButton == MouseButtonState.Pressed)
            {
                moving = true;

                Point newPosition = _config.FromUI(new Size(grid.ActualWidth,grid.ActualHeight), e.GetPosition(grid));

                    double left = dragStartPosition.X - oldPosition.X + newPosition.X;
                    double right = left+gui.Screen.PhysicalBounds.Width;

                    Point pNear = newPosition;
                    foreach (Screen s in _config.AllScreens)
                    {
                        if (s == gui.Screen) continue;

                        double minOffset = 10;
                        
                        double offset = s.PhysicalBounds.Right - left;
                        if (Math.Abs(offset) < minOffset)
                        {
                            pNear = new Point(newPosition.X + offset, newPosition.Y);
                            minOffset = Math.Abs(offset);
                        }

                        offset = s.PhysicalBounds.Left - left;
                        if (Math.Abs(offset) < minOffset)
                        {
                            pNear = new Point(newPosition.X + offset, newPosition.Y);
                            minOffset = Math.Abs(offset);
                        }

                        offset = s.PhysicalBounds.Right - right;
                        if (Math.Abs(offset) < minOffset)
                        {
                            pNear = new Point(newPosition.X + offset, newPosition.Y);
                            minOffset = Math.Abs(offset);
                        }

                        offset = s.PhysicalBounds.Left - right;
                        if (Math.Abs(offset) < minOffset)
                        {
                            pNear = new Point(newPosition.X + offset, newPosition.Y);
                            minOffset = Math.Abs(offset);
                        }
                    }

                    newPosition = pNear;
                    double top = dragStartPosition.Y - oldPosition.Y + newPosition.Y;
                    double bottom = top + gui.Screen.PhysicalBounds.Height;
                    foreach (Screen s in _config.AllScreens)
                    {
                        if (s == gui.Screen) continue;

                        double minOffset = 10;
                        double offset = s.PhysicalBounds.Bottom - top;
                        if (Math.Abs(offset) < minOffset)
                        {
                            pNear = new Point(newPosition.X , newPosition.Y + offset);
                            minOffset = Math.Abs(offset);
                        }

                        offset = s.PhysicalBounds.Bottom - bottom;
                        if (Math.Abs(offset) < minOffset)
                        {
                            pNear = new Point(newPosition.X, newPosition.Y + offset);
                            minOffset = Math.Abs(offset);
                        }

                        offset = s.PhysicalBounds.Top - top;
                        if (Math.Abs(offset) < minOffset)
                        {
                            pNear = new Point(newPosition.X, newPosition.Y + offset);
                            minOffset = Math.Abs(offset);
                        }

                        offset = s.PhysicalBounds.Top - bottom;
                        if (Math.Abs(offset) < minOffset)
                        {
                            pNear = new Point(newPosition.X, newPosition.Y + offset);
                            minOffset = Math.Abs(offset);
                        }

                    }
                    newPosition = pNear;

                    Point p = _config.PhysicalToUI(
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
                Rect all = _config.PhysicalOverallBounds;


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

        private void Save()
        {
            _config.Save();

            RegistryKey rk = Registry.CurrentUser.OpenSubKey("SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Run", true);

            if (chkLoadAtStartup.IsChecked==true)
            {
                rk.SetValue(System.Windows.Forms.Application.ProductName, System.Windows.Forms.Application.ExecutablePath.ToString());
            }
            else rk.DeleteValue(System.Windows.Forms.Application.ProductName, false);
        }

        private void cmdOk_Click(object sender, RoutedEventArgs e)
        {
            Save();
            Close();
        }

        private void cmdApply_Click(object sender, RoutedEventArgs e)
        {
            Save();
        }

        private void cmdCancel_Click(object sender, RoutedEventArgs e)
        {
            Close();
        }

        private void Window_Closing(object sender, System.ComponentModel.CancelEventArgs e)
        {
            foreach (UIElement el in grid.Children)
            {
                ScreenGUI el_gui = el as ScreenGUI;
                if (el_gui != null) el_gui.HideSizers();
            }

        }

        private void cmdUnload_Click(object sender, RoutedEventArgs e)
        {
            Application.Current.Shutdown();
        }
    }
}
