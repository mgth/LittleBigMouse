/*
  MouseControl - Mouse Managment in multi DPI monitors environment
  Copyright (c) 2015 Mathieu GRENET.  All right reserved.

  This file is part of MouseControl.

    ArduixPL is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    ArduixPL is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with MouseControl.  If not, see <http://www.gnu.org/licenses/>.

	  mailto:mathieu@mgth.fr
	  http://www.mgth.fr
*/

using Microsoft.Win32;
using System;
using System.Windows;
using System.Windows.Input;

namespace LittleBigMouse
{
    /// <summary>
    /// Interaction logic for Config.xaml
    /// </summary>
    public partial class FormConfig : Window
    {
        private ScreenConfig _newConfig;
        private ScreenConfig _currentConfig;

        public event EventHandler RegistryChanged;
        public FormConfig(ScreenConfig config)
        {
            _currentConfig = config;
            _newConfig = new ScreenConfig();

            _newConfig.RegistryChanged += _newConfig_RegistryChanged;

            InitializeComponent();

            if (App.Scheduled) 
                chkLoadAtStartup.IsChecked = true;
            else
                chkLoadAtStartup.IsChecked = false;

            // TODO : maybe use data binding;
            //DataContext = _newConfig;

            chkAdjustPointer.IsChecked = _newConfig.AdjustPointer;
            chkAdjustSpeed.IsChecked = _newConfig.AdjustSpeed;
            chkEnabled.IsChecked = _newConfig.Enabled;
            chkAllowToJump.IsChecked = _newConfig.AllowToJump;

            foreach (Screen s in _newConfig.AllScreens)
            {
                ScreenGUI sgui = new ScreenGUI(s,grid);
                grid.Children.Add(sgui);
                sgui.MouseMove += _gui_MouseMove;
                sgui.MouseLeftButtonDown += _gui_MouseLeftButtonDown;
                sgui.MouseLeftButtonUp += _gui_MouseLeftButtonUp;
                sgui.SelectedChanged += _gui_SelectedChanged;
            }

            LoadLocation();

            grid.SizeChanged += Grid_SizeChanged;

            SizeChanged += FormConfig_SizeChanged;
            LocationChanged += FormConfig_LocationChanged;
        }
        public Point PhysicalToUI(Point p)
        {
            Rect all = _newConfig.PhysicalOverallBounds;

            double ratio = Math.Min(
                grid.Width / all.Width,
                grid.Height / all.Height
                );

            return new Point(
                (p.X - all.Left) * ratio,
                (p.Y - all.Top) * ratio
                );
        }

        public Point UIToPhysical(Point p)
        {
            Rect all = _newConfig.PhysicalOverallBounds;

            double ratio = Math.Min(
                grid.ActualWidth / all.Width,
                grid.ActualHeight / all.Height
                );

            return new Point(
                (p.X / ratio) + all.Left,
                (p.Y / ratio) + all.Top
                );
        }

        private void _gui_SelectedChanged(Screen s, bool selected)
        {
            if (selected)
            {
                property.Children.Clear();
                property.Children.Add(new ScreenProperties(s));
            }
        }

        private void LoadLocation()
        {
            Rect wa = _currentConfig.PrimaryScreen.WpfWorkingArea;

            RegistryKey key = _currentConfig.Key.CreateSubKey("ConfigLocation");

            Left = double.Parse(key.GetValue("X", wa.X + (2 * wa.Width) / 3).ToString());
            Top = double.Parse(key.GetValue("Y", wa.Y + (2 * wa.Height) / 3).ToString());
            Width = double.Parse(key.GetValue("Width", wa.Width / 3).ToString());
            Height = double.Parse(key.GetValue("Height", wa.Height / 3).ToString());
        }
        private void SaveLocation()
        {
            RegistryKey key = _currentConfig.Key.CreateSubKey("ConfigLocation");
            key.SetValue("X", Left.ToString(), RegistryValueKind.String);
            key.SetValue("Y", Top.ToString(), RegistryValueKind.String);
            key.SetValue("Width", Width.ToString(), RegistryValueKind.String);
            key.SetValue("Height", Height.ToString(), RegistryValueKind.String);
        }

        private void FormConfig_LocationChanged(object sender, EventArgs e)
        {
            SaveLocation();
        }

        private void FormConfig_SizeChanged(object sender, SizeChangedEventArgs e)
        {
            SaveLocation();
        }

        private void _newConfig_RegistryChanged(object sender, EventArgs e)
        {
            if (RegistryChanged != null) RegistryChanged(sender, e);
        }

        private Point _oldPosition;
        private Point _dragStartPosition;
        private bool _moving = false;

        private void _gui_MouseLeftButtonUp(object sender, MouseButtonEventArgs e)
        {
            ScreenGUI gui = sender as ScreenGUI;
            if (sender == null) return;

            if (_moving)
            {

                gui.Screen.PhysicalLocation = gui.PhysicalLocation;

                _moving = false;
                ResizeAll();

                foreach(UIElement el in grid.Children)
                {
                    ScreenGUI el_gui = el as ScreenGUI;
                    if (el_gui != null && el_gui != gui)
                    {
                        el_gui.Selected = false;
                    }
                }
                gui.Selected = true;
            }
            else // Its a click
            {
                foreach (UIElement el in grid.Children)
                {
                    ScreenGUI el_gui = el as ScreenGUI;
                    if (el_gui != null && el_gui != gui)
                    {
                        el_gui.Selected = false;
                    }
                }
                gui.Selected = !gui.Selected;
            }
        }

        private void _gui_MouseLeftButtonDown(object sender, MouseButtonEventArgs e)
        {
            ScreenGUI gui = sender as ScreenGUI;
            if (sender == null) return;

            _oldPosition = UIToPhysical(e.GetPosition(grid));
            _dragStartPosition = gui.Screen.PhysicalLocation;

            // bring element to front so we can move it over the others
            grid.Children.Remove(gui);
            grid.Children.Add(gui);
        }


        private void _gui_MouseMove(object sender, MouseEventArgs e)
        {
            ScreenGUI gui = sender as ScreenGUI;
            if (sender == null) return;

            if (e.LeftButton == MouseButtonState.Pressed)
            {
                _moving = true;

                Point newPosition = UIToPhysical(e.GetPosition(grid));

                    double left = _dragStartPosition.X - _oldPosition.X + newPosition.X;
                    double right = left+gui.Screen.PhysicalWidth;

                    Point pNear = newPosition;
                    foreach (Screen s in _newConfig.AllScreens)
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
                    double top = _dragStartPosition.Y - _oldPosition.Y + newPosition.Y;
                    double bottom = top + gui.Screen.PhysicalBounds.Height;
                    foreach (Screen s in _newConfig.AllScreens)
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

                    gui.PhysicalLocation =
                        new Point(
                            _dragStartPosition.X - _oldPosition.X + newPosition.X,
                            _dragStartPosition.Y - _oldPosition.Y + newPosition.Y
                            )
                        ;


                    //oldPosition = newPosition;
                }
        }

        private void Grid_SizeChanged(object sender, SizeChangedEventArgs e)
        {
            ResizeAll();
        }

        private void ResizeAll() 
        {
            foreach(UIElement element in grid.Children)
            {
                Rect all = _newConfig.PhysicalOverallBounds;


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
            _newConfig.Save();

            if (chkLoadAtStartup.IsChecked == true)
                App.Schedule();
            else
                App.Unschedule();
        }

        private void cmdOk_Click(object sender, RoutedEventArgs e)
        {
            _newConfig.Disable();
            Save();
            Close();
        }

        private void cmdApply_Click(object sender, RoutedEventArgs e)
        {
            _newConfig.Disable();
            Save();
        }

        private void cmdCancel_Click(object sender, RoutedEventArgs e)
        {
            _newConfig.Disable();
            _currentConfig.Enable();
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


        private void chkLiveUpdate_Checked(object sender, RoutedEventArgs e)
        {
            if (_currentConfig != null) _currentConfig.Disable();
            _newConfig.Enable();
        }

        private void chkLiveUpdate_Unchecked(object sender, RoutedEventArgs e)
        {
            _newConfig.Disable();
            if (_currentConfig != null) _currentConfig.Enable();
        }

        private void chkEnabled_Checked(object sender, RoutedEventArgs e)
        {
            _newConfig.Enabled = true;
        }

        private void chkEnabled_Unchecked(object sender, RoutedEventArgs e)
        {
            _newConfig.Enabled = false;
        }

        private void chkAdjustSpeed_Checked(object sender, RoutedEventArgs e)
        {
            _newConfig.AdjustSpeed = true;
        }

        private void chkAdjustPointer_Checked(object sender, RoutedEventArgs e)
        {
            _newConfig.AdjustPointer = true;
        }

        private void chkAdjustSpeed_Unchecked(object sender, RoutedEventArgs e)
        {
            _newConfig.AdjustSpeed = false;
        }

        private void chkAdjustPointer_Unchecked(object sender, RoutedEventArgs e)
        {
            _newConfig.AdjustPointer = false;
        }

        private void chkAllowToJump_Checked(object sender, RoutedEventArgs e)
        {
            _newConfig.AllowToJump = true;
        }

        private void chkAllowToJump_Unchecked(object sender, RoutedEventArgs e)
        {
            _newConfig.AllowToJump = false;
        }
    }
}
