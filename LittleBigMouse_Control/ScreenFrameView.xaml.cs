/*
  MouseControl - LbmMouse Managment in multi DPI monitors environment
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
using System.Windows;
using System.Windows.Controls;
using System.Windows.Data;
using System.Windows.Input;
using System.Windows.Media;
using LbmScreenConfig;

namespace LittleBigMouse_Control
{
    public partial class ScreenFrameView : UserControl
    {
        public Screen Screen => (DataContext as ScreenViewModel)?.Screen;

        public ScreenFrameView()
        {
            InitializeComponent();
        }

        private void TextBox_MouseWheel(object sender, MouseWheelEventArgs e)
        {
            TextBox tBox = (TextBox)sender;

            double delta = (e.Delta > 0) ? 1 : -1;

            DependencyProperty prop = TextBox.TextProperty;

            BindingExpression binding = BindingOperations.GetBindingExpression(tBox, prop);
            binding?.Target.SetValue(prop, (double.Parse(binding?.Target.GetValue(prop).ToString()) + delta).ToString() );
            binding?.UpdateSource();
        }

        private void ResetPlace_Click(object sender, RoutedEventArgs e)
        {
            Screen.Config.SetPhysicalAuto();
        }

        private void ResetSize_Click(object sender, RoutedEventArgs e)
        {
            Screen.RealPhysicalHeight = double.NaN;
            Screen.RealPhysicalWidth = double.NaN;
        }
    }

    public class Anchor
    {
        public Screen Screen { get; }
        public double Pos { get; }
        public Brush Brush { get; }

        public Anchor(Screen screen, double pos, Brush brush)
        {
            Screen = screen;
            Pos = pos;
            Brush = brush;
        }
    }
}

