/*
  HLab.Windows.MonitorVcp
  Copyright (c) 2021 Mathieu GRENET.  All right reserved.

  This file is part of HLab.Windows.MonitorVcp.

    HLab.Windows.MonitorVcp is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    HLab.Windows.MonitorVcp is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with MouseControl.  If not, see <http://www.gnu.org/licenses/>.

	  mailto:mathieu@mgth.fr
	  http://www.mgth.fr
*/

using System.Windows;
using System.Windows.Controls;
using System.Windows.Data;
using System.Windows.Input;
using HLab.Sys.Windows.Monitors;
using LittleBigMouse.DisplayLayout.Monitors;

//using System.Drawing;

namespace HLab.Sys.Windows.MonitorVcp
{
    /// <summary>
    /// Logique d'interaction pour Luminance.xaml
    /// </summary>
    public partial class Luminance : UserControl
    {
        public Luminance()
        {
            InitializeComponent();

            DataContext = new LuminanceViewModel();

            SetBinding(MonitorsProperty, new Binding("Config") { Mode = BindingMode.TwoWay });
        }

        LuminanceViewModel ViewModel => DataContext as LuminanceViewModel;

        public static readonly DependencyProperty MonitorsProperty = H.Property<IMonitorsLayout>().Register();
            
            //DependencyProperty.Register("Config", typeof(IMonitorsService), typeof(Luminance), new PropertyMetadata(null));

        public IMonitorsLayout Monitors
        {
            get => (IMonitorsLayout)GetValue(MonitorsProperty); set
            {
                ViewModel.Monitors = value;
                SetValue(MonitorsProperty, value);
            }
        }

        public double WheelDelta(MouseWheelEventArgs e)
        {
            double delta = (e.Delta > 0) ? 1 : -1;
            if ((Keyboard.Modifiers & ModifierKeys.Control) != 0) delta /= 10;
            return delta;
        }

        public void OnMouseWheel(object sender, MouseWheelEventArgs e)
        {
            ViewModel.Value += WheelDelta(e);
        }

        void Control_OnMouseDoubleClick(object sender, MouseButtonEventArgs e)
        {
            ViewModel.Value = ViewModel.MaxAll;
        }

        void UIElement_OnMouseLeftButtonDown(object sender, MouseButtonEventArgs e)
        {
            if (e.ClickCount == 2) ViewModel.Value = ViewModel.MinAll;
        }
    }

}

