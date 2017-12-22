/*
  HLab.Windows.MonitorVcp
  Copyright (c) 2017 Mathieu GRENET.  All right reserved.

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
using LittleBigMouse.ScreenConfigs;

//using System.Drawing;

namespace Hlab.Windows.MonitorVcp
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

            SetBinding(ConfigProperty, new Binding("Config") { Mode = BindingMode.TwoWay });
        }

        private LuminanceViewModel ViewModel => DataContext as LuminanceViewModel;

        public static readonly DependencyProperty ConfigProperty = DependencyProperty.Register("Config", typeof(ScreenConfig), typeof(Luminance), new PropertyMetadata(null));

        public ScreenConfig Config
        {
            get => (ScreenConfig)GetValue(ConfigProperty); set
            {
                ViewModel.Config = value;
                SetValue(ConfigProperty, value);
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

        private void Control_OnMouseDoubleClick(object sender, MouseButtonEventArgs e)
        {
            ViewModel.Value = ViewModel.MaxAll;
        }

        private void UIElement_OnMouseLeftButtonDown(object sender, MouseButtonEventArgs e)
        {
            if (e.ClickCount == 2) ViewModel.Value = ViewModel.MinAll;
        }
    }

}

