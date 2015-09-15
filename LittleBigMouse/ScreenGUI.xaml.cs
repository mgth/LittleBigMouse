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
using System;
using System.Collections.Generic;
using System.ComponentModel;
using System.Linq;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Media;

namespace LittleBigMouse
{
    public delegate void ScreenGuiSelectedChangedHandler(object sender, bool selected);
    /// <summary>
    /// Interaction logic for ScreenGUI.xaml
    /// </summary>
    public partial class ScreenGui : UserControl, INotifyPropertyChanged
    {
        public event PropertyChangedEventHandler PropertyChanged;
        private void Changed(string name)
        {
            PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(name));
        }

        public Screen Screen { get; }

        public event ScreenGuiSelectedChangedHandler SelectedChanged; 
        private void OnSelectedChanged()
        {
            SelectedChanged?.Invoke(Screen, Selected);
        }

        private bool _selected = false;
        public bool Selected
        {
            get { return _selected; }
            set
            {
                _selected = value;
                if (value)
                {

                    selectStartColor.Color = Colors.Lime;
                    selectStopColor.Color = Colors.DarkGreen;
                    OnSelectedChanged();
                }
                else
                {
                    selectStartColor.Color = Colors.Gray;
                    selectStopColor.Color = Colors.Gray;
                    OnSelectedChanged();
                }
            }
        }

        private PhysicalPoint _physicalLocation = null;
        public PhysicalPoint PhysicalLocation
        {
            set {
                _physicalLocation = value;
                UpdateSize();
                Changed("PhysicalLocation");
            }
            get { return _physicalLocation; }
        }

        public void UpdateSize()
        {

            Rect all = Screen.Config.PhysicalOverallBounds;

            double ratio = Math.Min(
                _grid.ActualWidth / all.Width,
                _grid.ActualHeight / all.Height
                );

            if (double.IsNaN(ratio)) return;

            Margin = new Thickness(
                    (PhysicalLocation.X - all.X) * ratio,
                    (PhysicalLocation.Y - all.Y) * ratio,
                    0, 0);

            Width = Screen.PhysicalWidth * ratio;
            Height = Screen.PhysicalHeight * ratio;
        }

        private readonly Grid _grid;
         public ScreenGui(Screen s, Grid grid)
        {
            _grid = grid;
            Screen = s;
            _physicalLocation = s.PhysicalLocation;

            InitializeComponent();

            Screen.PropertyChanged += _screen_PropertyChanged;
            _grid.SizeChanged += _grid_SizeChanged;
            Screen.Config.PropertyChanged += Config_PropertyChanged;

            DataContext = this;
        }

        private void Config_PropertyChanged(object sender, PropertyChangedEventArgs e)
        {
            UpdateSize();
        }

        private void _grid_SizeChanged(object sender, SizeChangedEventArgs e)
        {
            UpdateSize();
        }

        private void _screen_PropertyChanged(object sender, PropertyChangedEventArgs e)
        {
            switch (e.PropertyName)
            {
                case "PhysicalBounds":
                    PhysicalLocation = Screen.PhysicalLocation;
                    break;
                case "DpiAvg":
                    if (Selected)
                    {
                        Selected = false;
                        Selected = true;
                    }
                    break;
            }
        }


        private void Grid_SizeChanged(object sender, SizeChangedEventArgs e)
        {
            center.Height = Math.Min(grid.ActualHeight, grid.ActualWidth)/3;
            center.Width =  center.Height;
            center.CornerRadius = new CornerRadius(center.Height / 2);

            if (center.Height>0)
                lblName.FontSize = center.Height / 2;
        }

        private void UserControl_SizeChanged(object sender, SizeChangedEventArgs e)
        {
            Changed("PhysicalLocation");
        }
    }
}
