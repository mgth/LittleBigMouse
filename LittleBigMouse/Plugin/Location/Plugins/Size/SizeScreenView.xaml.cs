/*
  LittleBigMouse.Plugin.Location
  Copyright (c) 2017 Mathieu GRENET.  All right reserved.

  This file is part of LittleBigMouse.Plugin.Location.

    LittleBigMouse.Plugin.Location is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    LittleBigMouse.Plugin.Location is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with MouseControl.  If not, see <http://www.gnu.org/licenses/>.

	  mailto:mathieu@mgth.fr
	  http://www.mgth.fr
*/

using System;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;
using HLab.Mvvm;
using HLab.Mvvm.Annotations;
using HLab.Windows.API;
using LittleBigMouse.Control.Core;

namespace LittleBigMouse.Plugin.Location.Plugins.Size
{
    class SizeScreenContentView : UserControl, IView<ViewModeScreenSize, ScreenSizeViewModel>, IViewScreenFrameTopLayer 
    {
    }

    /// <summary>
    /// Logique d'interaction pour ScreenGuiBorders.xaml
    /// </summary>
    public partial class SizeScreenView : UserControl , IView<ViewModeScreenSize, ScreenSizeViewModel>, IViewScreenFrameContent
    {
        public SizeScreenView()
        {
            InitializeComponent();

            SizeChanged += SizeScreenView_SizeChanged;
            LayoutUpdated += SizeScreenView_LayoutUpdated;
        }

        private void SizeScreenView_LayoutUpdated(object sender, EventArgs e)
        {
            if (_staticPoint != null)
            {
                 Point p2 = PointToScreen(
                     new Point(
                         ActualWidth * _staticPoint.Value.X, 
                         ActualHeight * _staticPoint.Value.Y
                         ));
                    NativeMethods.SetCursorPos((int)p2.X, (int)p2.Y);

                _staticPoint = null;
            }
        }

        private void SizeScreenView_SizeChanged(object sender, SizeChangedEventArgs e)
        {
        }

        ScreenSizeViewModel ViewModel => (DataContext as ScreenSizeViewModel);


        private void Height_MouseWheel(object sender, MouseWheelEventArgs e)
        {
            SetStaticPoint();
            ViewModel.OutsideHeight += WheelDelta(e);
        }
        private void Width_MouseWheel(object sender, MouseWheelEventArgs e)
        {
            SetStaticPoint();
            ViewModel.OutsideWidth += WheelDelta(e);
        }

        private void Bottom_MouseWheel(object sender, MouseWheelEventArgs e)
        {
            SetStaticPoint();
            ViewModel.Model.InMm.BottomBorder += WheelDelta(e);
        }

        private void Right_MouseWheel(object sender, MouseWheelEventArgs e)
        {
            SetStaticPoint();
            ViewModel.Model.InMm.RightBorder += WheelDelta(e);
        }

        private void Left_MouseWheel(object sender, MouseWheelEventArgs e)
        {
            SetStaticPoint();
            ViewModel.Model.InMm.LeftBorder += WheelDelta(e);
        }

        private void Top_MouseWheel(object sender, MouseWheelEventArgs e)
        {
            SetStaticPoint();
            ViewModel.Model.InMm.TopBorder += WheelDelta(e);
        }

        private void OnKeyEnterUpdate(object sender, KeyEventArgs e)
        {
            ViewHelper.OnKeyEnterUpdate(sender, e);
        }

        public double WheelDelta(MouseWheelEventArgs e)
        {
            double delta = (e.Delta > 0) ? 1 : -1;
            if ((Keyboard.Modifiers & ModifierKeys.Control) != 0) delta /= 10;
            return delta;
        }

        private Point? _staticPoint = null;

        private void SetStaticPoint()
        {
            Point p = Mouse.GetPosition(this);
            _staticPoint = new Point(
                p.X/ActualWidth,
                p.Y/ActualHeight);
        }

        private void InsideHeight_MouseWheel(object sender, MouseWheelEventArgs e)
        {
            SetStaticPoint();
            ViewModel.Height += WheelDelta(e);
        }

        private void InsideWidth_MouseWheel(object sender, MouseWheelEventArgs e)
        {
            SetStaticPoint();
            ViewModel.Width += WheelDelta(e);
        }
    }
}
