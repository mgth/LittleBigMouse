/*
  LittleBigMouse.Plugin.Location
  Copyright (c) 2021 Mathieu GRENET.  All right reserved.

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
using System.Linq;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;
using HLab.Mvvm.Annotations;

namespace LittleBigMouse.Plugin.Location.Plugins.Location.Rulers;

/// <summary>
/// Logique d'interaction pour Sizer.xaml
/// </summary>
/// 

public partial class RulerView : UserControl, IView<RulerViewModel>
{
    public RulerView()
    {
        InitializeComponent();
    }

    public RulerViewModel ViewModel => DataContext as RulerViewModel;

    private Point _oldPoint;
    private Point? _dragStartPoint;

    private void Window_MouseMove(object sender, MouseEventArgs e)
    {
        if (!Moving || _dragStartPoint == null) return;

        var newPoint = e.GetPosition(this);

        if (ViewModel.Vertical)
        {
            var pitch = ViewModel.DrawOn.Monitor.InMm.Height / (ActualHeight - 16.5);

            var offset = newPoint.Y - _oldPoint.Y;

            var old = ViewModel.DrawOn.Monitor.InMm.Y;

            ViewModel.DrawOn.Monitor.InMm.Y = _dragStartPoint.Value.Y - offset * pitch;

            if (ViewModel.DrawOn.Primary && Math.Abs(ViewModel.DrawOn.Monitor.InMm.Y - old) < double.Epsilon) _oldPoint.Y += offset;
        }
        else
        {
            var pitch = ViewModel.DrawOn.Monitor.InMm.Width / (ActualWidth - 16.5);

            var offset = newPoint.X - _oldPoint.X;

            var old = ViewModel.DrawOn.Monitor.InMm.X;


            ViewModel.DrawOn.Monitor.InMm.X = _dragStartPoint.Value.X - offset * pitch;

            if (ViewModel.DrawOn.Primary && Math.Abs(ViewModel.DrawOn.Monitor.InMm.X - old) < double.Epsilon) _oldPoint.X += offset;
        }
    }

    private void Window_MouseDown(object sender, MouseButtonEventArgs e)
    {
        _oldPoint = e.GetPosition(this);

        var p =  InvertControl ? ViewModel.DrawOn.Monitor.InMm.Location : ViewModel.Screen.InMm.Location;
        _dragStartPoint = new(p.X,p.Y);
        Moving = true;
        CaptureMouse();
    }

    private void Window_MouseUp(object sender, MouseButtonEventArgs e)
    {
        if (!Moving) return;
        Moving = false;
        _dragStartPoint = null;
        ReleaseMouseCapture();
    }

    private bool Moving { get; set; } = false;
    public bool InvertControl { get; set; } = true;

    private void Ruler_OnMouseWheel(object sender, MouseWheelEventArgs e)
    {
        var delta = WheelDelta(e) * 0.01;

        var p = e.GetPosition(this);

        if (ViewModel.Vertical)
        {
            var pitch = ViewModel.DrawOn.Monitor.InMm.Height / (ActualHeight - 16.5);
            var pos = pitch * p.Y;
            ViewModel.DrawOn.Monitor.PhysicalRatio.Y += delta;

            var pitch2 = ViewModel.DrawOn.Monitor.InMm.Height / (ActualHeight - 16.5);
            var pos2 = pitch2 * p.Y;

            ViewModel.DrawOn.Monitor.InMm.Y += pos - pos2;
        }
        else
        {
            var pitch = ViewModel.DrawOn.Monitor.InMm.Width / (ActualWidth - 16.5);
            var pos = pitch * p.X;
            ViewModel.DrawOn.Monitor.PhysicalRatio.X += delta;

            var pitch2 = ViewModel.DrawOn.Monitor.InMm.Width / (ActualWidth - 16.5);
            var pos2 = pitch2 * p.X;

            ViewModel.DrawOn.Monitor.InMm.X += pos - pos2;
        }
    }
    private static double WheelDelta(MouseWheelEventArgs e)
    {
        double delta = (e.Delta > 0) ? 1 : -1;
        if ((Keyboard.Modifiers & ModifierKeys.Control) != 0) delta /= 10;
        if ((Keyboard.Modifiers & ModifierKeys.Shift) != 0) delta *= 10;
        return delta;
    }
}
