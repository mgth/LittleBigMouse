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
using System.Globalization;
using System.Linq;
using System.Windows;
using System.Windows.Input;

namespace LittleBigMouse
{
    /// <summary>
    /// Interaction logic for Config.xaml
    /// </summary>
    public partial class FormConfig : Window, IDisposable
    {
        private ScreenConfig _newConfig;
        private ScreenConfig _currentConfig;

        Screen _selected = null;
        public Screen Selected
        {
            get { return _selected; }
            set
            {
                _selected = value;
                if (PropertyPane != null)
                {
                    PropertyPane.Screen = value;
                }
            }
        }

        PropertyPane _propertyPane = null;
        PropertyPane PropertyPane
        {
            get { return _propertyPane; }
            set {
                if (_propertyPane!=null)
                {
                    property.Children.Clear();
                    _propertyPane.Screen = null;
                }

                _propertyPane = value;

                if (value!=null)
                {
                    property.Children.Add(value as UIElement);
                    value.Screen = Selected;
                }
            }
        }



        public event EventHandler RegistryChanged;
        public FormConfig(ScreenConfig config)
        {
            _currentConfig = config;
            _newConfig = new ScreenConfig();

            _newConfig.RegistryChanged += _newConfig_RegistryChanged;

            InitializeComponent();

            DataContext = _newConfig;

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
                if (PropertyPane!=null)
                {
                    PropertyPane.Screen = s;
                }
            }
        }

        private void LoadLocation()
        {
            Rect wa = _currentConfig.PrimaryScreen.WpfWorkingArea;

            using (RegistryKey configkey = _currentConfig.OpenRegKey())
            {
                using (RegistryKey k = configkey.CreateSubKey("ConfigLocation"))
                {
                    Left = double.Parse( k.GetValue( "X", (wa.X + (2 * wa.Width) / 3).ToString(CultureInfo.InvariantCulture) ).ToString() ,CultureInfo.InvariantCulture);
                    Top = double.Parse(k.GetValue("Y", (wa.Y + (2 * wa.Height) / 3).ToString(CultureInfo.InvariantCulture)).ToString(), CultureInfo.InvariantCulture);
                    Width = double.Parse(k.GetValue("Width", (wa.Width / 3).ToString(CultureInfo.InvariantCulture)).ToString(), CultureInfo.InvariantCulture);
                    Height = double.Parse(k.GetValue("Height", (wa.Height / 3).ToString(CultureInfo.InvariantCulture)).ToString(), CultureInfo.InvariantCulture);
                }
            }

            //TODO reinplement thet
            //Screen s = _currentConfig.FromWpfPoint(new PixelPoint(null, Left, Top));

            //if ( s==null)
            {
                Top = wa.Y + (2 * wa.Height) / 3;
                Left =  wa.Y + (2 * wa.Height) / 3;
            }
        }
        private void SaveLocation()
        {
            using (RegistryKey configkey = _currentConfig.OpenRegKey())
            {
                using (RegistryKey k = configkey.CreateSubKey("ConfigLocation"))
                {
                    k.SetValue("X", Left.ToString(CultureInfo.InvariantCulture), RegistryValueKind.String);
                    k.SetValue("Y", Top.ToString(CultureInfo.InvariantCulture), RegistryValueKind.String);
                    k.SetValue("Width", Width.ToString(CultureInfo.InvariantCulture), RegistryValueKind.String);
                    k.SetValue("Height", Height.ToString(CultureInfo.InvariantCulture), RegistryValueKind.String);
                }
            }
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
            RegistryChanged?.Invoke(sender, e);
        }

        private Point _oldPosition;
        private PhysicalPoint _dragStartPosition;
        private bool _moving = false;

        private void _gui_MouseLeftButtonUp(object sender, MouseButtonEventArgs e)
        {
            var gui = sender as ScreenGUI;
            if (gui == null) return;

            if (_moving)
            {
                gui.Screen.PhysicalLocation = gui.PhysicalLocation;

                _moving = false;

                foreach (var elGui in grid.Children.Cast<UIElement>().OfType<ScreenGUI>().Where(elGui => !Equals(elGui, gui)))
                {
                    elGui.Selected = false;
                }
                gui.Selected = true;
            }
            else // Its a click
            {
                foreach (var elGui in grid.Children.Cast<UIElement>().OfType<ScreenGUI>().Where(elGui => !Equals(elGui, gui)))
                {
                    elGui.Selected = false;
                }
                gui.Selected = !gui.Selected;
            }
        }

        private void _gui_MouseLeftButtonDown(object sender, MouseButtonEventArgs e)
        {
            var gui = sender as ScreenGUI;
            if (gui == null) return;

            _oldPosition = UIToPhysical(e.GetPosition(grid));
            _dragStartPosition = gui.Screen.PhysicalLocation;

            // bring element to front so we can move it over the others
            grid.Children.Remove(gui);
            grid.Children.Add(gui);
        }

        private bool _allowMove = false;
        private void _gui_MouseMove(object sender, MouseEventArgs e)
        {
            var gui = sender as ScreenGUI;
            if (gui == null) return;

            if (!_allowMove || e.LeftButton != MouseButtonState.Pressed) return;
            _moving = true;

            var newPosition = UIToPhysical(e.GetPosition(grid));

            var left = _dragStartPosition.X - _oldPosition.X + newPosition.X;
            if (gui.Screen != null)
            {
                var right = left+gui.Screen.PhysicalWidth;

                var pNear = newPosition;
                foreach (var s in _newConfig.AllScreens)
                {
                    if (s == gui.Screen) continue;

                    double minOffset = 10;
                        
                    var offset = s.PhysicalBounds.Right - left;
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
                var top = _dragStartPosition.Y - _oldPosition.Y + newPosition.Y;
                var bottom = top + gui.Screen.PhysicalBounds.Height;
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
            }

            gui.PhysicalLocation =
                new PhysicalPoint(
                    gui.Screen,
                    _dragStartPosition.X - _oldPosition.X + newPosition.X,
                    _dragStartPosition.Y - _oldPosition.Y + newPosition.Y
                    )
                ;


            //oldPosition = newPosition;
        }


        private void Save()
        {
            _newConfig.Save();
        }

        private void cmdOk_Click(object sender, RoutedEventArgs e)
        {
            _newConfig.Stop();
            Save();
            Close();
        }

        private void cmdApply_Click(object sender, RoutedEventArgs e)
        {
            _newConfig.Stop();
            Save();
        }

        private void cmdCancel_Click(object sender, RoutedEventArgs e)
        {
            _newConfig.Stop();
            _currentConfig.Start();
            Close();
        }

        private void Window_Closing(object sender, System.ComponentModel.CancelEventArgs e)
        {
            Selected = null;
        }

        private void cmdUnload_Click(object sender, RoutedEventArgs e)
        {
            Application.Current.Shutdown();
        }


        private void chkLiveUpdate_Checked(object sender, RoutedEventArgs e)
        {
            if (_currentConfig != null) _currentConfig.Stop();
            _newConfig.Start();
        }

        private void chkLiveUpdate_Unchecked(object sender, RoutedEventArgs e)
        {
            _newConfig.Stop();
            if (_currentConfig != null) _currentConfig.Start();
        }

        private void ToggleButton_Click(object sender, RoutedEventArgs e)
        {

        }

        private void cmdLayout_Click(object sender, RoutedEventArgs e)
        {
        }

        private void cmdPattern_Click(object sender, RoutedEventArgs e)
        {
        }

        private void cmdLayout_Checked(object sender, RoutedEventArgs e)
        {
            PropertyPane = new ScreenProperties();
            _allowMove = true;
        }

        private void cmdLayout_Unchecked(object sender, RoutedEventArgs e)
        {
            _allowMove = false;
            if (PropertyPane.GetType() == typeof(ScreenProperties)) PropertyPane = null;
        }

        private void cmdPattern_Checked(object sender, RoutedEventArgs e)
        {
            PropertyPane = new ControlPane();
        }

        private void cmdPattern_Unchecked(object sender, RoutedEventArgs e)
        {
            if (PropertyPane.GetType() == typeof(ControlPane)) PropertyPane = null;
        }

        #region IDisposable Support
        private bool disposedValue = false; // To detect redundant calls

        protected virtual void Dispose(bool disposing)
        {
            if (!disposedValue)
            {
                if (disposing)
                {
                    if (_newConfig != null)
                    {
                        _newConfig.Dispose();
                        _newConfig = null;
                    }
                    if (_currentConfig != null)
                    {
                        _currentConfig.Dispose();
                        _currentConfig = null;
                    }
                }

                // TODO: free unmanaged resources (unmanaged objects) and override a finalizer below.
                // TODO: set large fields to null.

                disposedValue = true;
            }
        }

        // TODO: override a finalizer only if Dispose(bool disposing) above has code to free unmanaged resources.
        // ~FormConfig() {
        //   // Do not change this code. Put cleanup code in Dispose(bool disposing) above.
        //   Dispose(false);
        // }

        // This code added to correctly implement the disposable pattern.
        public void Dispose()
        {
            // Do not change this code. Put cleanup code in Dispose(bool disposing) above.
            Dispose(true);
            // TODO: uncomment the following line if the finalizer is overridden above.
            // GC.SuppressFinalize(this);
        }
        #endregion
    }
}
