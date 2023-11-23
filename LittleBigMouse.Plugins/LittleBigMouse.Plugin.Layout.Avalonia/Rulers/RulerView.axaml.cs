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

using Avalonia;
using Avalonia.Controls;
using Avalonia.Input;
using HLab.Mvvm.Annotations;

namespace LittleBigMouse.Plugin.Layout.Avalonia.Rulers;

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

    Point _oldPoint;
    Point? _dragStartPoint;


    void Window_PointerMoved(object? sender, PointerEventArgs e)
    {
        if (!Moving || _dragStartPoint == null) return;

        var newPoint = e.GetPosition(this);

        if (ViewModel.Vertical)
        {
            var pitch = ViewModel.DrawOn.Monitor.DepthProjection.Height / (Bounds.Height - 16.5);

            var offset = newPoint.Y - _oldPoint.Y;

            var old = ViewModel.DrawOn.Monitor.DepthProjection.Y;

            ViewModel.DrawOn.Monitor.DepthProjection.Y = _dragStartPoint.Value.Y - offset * pitch;

            if (ViewModel.DrawOn.Source.Primary && Math.Abs(ViewModel.DrawOn.Monitor.DepthProjection.Y - old) < double.Epsilon) 
                _oldPoint += new Vector(0,offset) ;
        }
        else
        {
            var pitch = ViewModel.DrawOn.Monitor.DepthProjection.Width / (Bounds.Width - 16.5);

            var offset = newPoint.X - _oldPoint.X;

            var old = ViewModel.DrawOn.Monitor.DepthProjection.X;


            ViewModel.DrawOn.Monitor.DepthProjection.X = _dragStartPoint.Value.X - offset * pitch;

            if (ViewModel.DrawOn.Source.Primary && Math.Abs(ViewModel.DrawOn.Monitor.DepthProjection.X - old) < double.Epsilon) 
                _oldPoint += new Vector(offset, 0);
        }
    }

    void Window_PointerPressed(object? sender, PointerPressedEventArgs e)
    {
        _oldPoint = e.GetPosition(this);

        var p =  InvertControl ? ViewModel.DrawOn.Monitor.DepthProjection.Location : ViewModel.Monitor.DepthProjection.Location;
        _dragStartPoint = new(p.X,p.Y);
        Moving = true;
        e.Pointer.Capture(this);
    }

    void Window_PointerReleased(object? sender, PointerReleasedEventArgs e)
    {
        if (!Moving) return;
        Moving = false;
        _dragStartPoint = null;
        e.Pointer.Capture(null);
    }

    bool Moving { get; set; } = false;
    public bool InvertControl { get; set; } = true;

    void Window_OnMouseWheel(object? sender, PointerWheelEventArgs e)
    {
        var delta = WheelDelta(e) * 0.01;

        var p = e.GetPosition(this);

        if (ViewModel.Vertical)
        {
            var pitch = ViewModel.DrawOn.Monitor.DepthProjection.Height / (Bounds.Height - 16.5);
            var pos = pitch * p.Y;
            ViewModel.DrawOn.Monitor.DepthRatio.Y += delta;

            var pitch2 = ViewModel.DrawOn.Monitor.DepthProjection.Height / (Bounds.Height - 16.5);
            var pos2 = pitch2 * p.Y;

            ViewModel.DrawOn.Monitor.DepthProjection.Y += pos - pos2;
        }
        else
        {
            var pitch = ViewModel.DrawOn.Monitor.DepthProjection.Width / (Bounds.Width - 16.5);
            var pos = pitch * p.X;
            ViewModel.DrawOn.Monitor.DepthRatio.X += delta;

            var pitch2 = ViewModel.DrawOn.Monitor.DepthProjection.Width / (Bounds.Width - 16.5);
            var pos2 = pitch2 * p.X;

            ViewModel.DrawOn.Monitor.DepthProjection.X += pos - pos2;
        }
    }

    static double WheelDelta(PointerWheelEventArgs e)
    {
        double delta = (e.Delta.Y > 0) ? 1 : -1;
        if ((e.KeyModifiers  & KeyModifiers.Control) != 0) delta /= 10;
        if ((e.KeyModifiers & KeyModifiers.Shift) != 0) delta *= 10;
        return delta;
    }




}
