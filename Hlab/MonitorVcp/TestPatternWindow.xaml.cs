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
using System.Windows.Input;
using System.Windows.Media;
using LittleBigMouse.ScreenConfigs;

namespace HLab.Windows.MonitorVcp
{
    /// <summary>
    /// Logique d'interaction pour TestPatternWindow.xaml
    /// </summary>
    public partial class TestPatternWindow : Window
    {
        public TestPatternType PatternType
        {
            set => pattern.PatternType = value; get => pattern.PatternType;
        }
        public Color PatternColor
        {
            set => pattern.PatternColor = value; get => pattern.PatternColor;
        }
        public TestPatternWindow()
        {
            InitializeComponent();
        }
        private void Window_PreviewKeyDown(object sender, KeyEventArgs e)
        {
            Window win = (Window)sender;

            if (e.Key == Key.Escape)
                win.Visibility = System.Windows.Visibility.Hidden;//.Close();
                                                                  //else if( e.Key == Key.Left )

        }


        //TODO
        public void ShowOnScreen(Screen s)
        {
            if (s != null)
            {
                //Left = s.Bounds.TopLeft.Dip.X;
                //Top = s.Bounds.TopLeft.Dip.Y;
                //Width = s.Bounds.BottomRight.Dip.X - s.Bounds.TopLeft.Dip.X;
                //Height = s.Bounds.BottomRight.Dip.Y - s.Bounds.TopLeft.Dip.Y;

                Show();
            }
            else
            {
                Hide();
            }
        }

    }
}