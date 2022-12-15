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
using System.Globalization;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Data;
using System.Windows.Input;
using System.Windows.Threading;
using HLab.Mvvm;
using HLab.Mvvm.Annotations;
using HLab.Sys.Windows.API;
using LittleBigMouse.Plugins;

namespace LittleBigMouse.Plugin.Location.Plugins.Size;

class SizeScreenContentView : UserControl, IView<ViewModeScreenSize, ScreenSizeViewModel>, IViewScreenFrameTopLayer
{
}

/// <summary>
/// Logique d'interaction pour ScreenGuiBorders.xaml
/// </summary>
public partial class SizeScreenView : UserControl, IView<ViewModeScreenSize, ScreenSizeViewModel>, IViewScreenFrameContent
{
    public SizeScreenView()
    {
        InitializeComponent();
    }

    ScreenSizeViewModel ViewModel => (DataContext as ScreenSizeViewModel);

    private void OnKeyEnterUpdate(object sender, KeyEventArgs e)
    {
        ViewHelper.OnKeyEnterUpdate(sender, e);
    }

    private static double WheelDelta(MouseWheelEventArgs e)
    {
        double delta = (e.Delta > 0) ? 1 : -1;
        if ((Keyboard.Modifiers & ModifierKeys.Control) != 0) delta /= 10;
        if ((Keyboard.Modifiers & ModifierKeys.Shift) != 0) delta *= 10;
        return delta;
    }

    private void OnMouseWheel(object sender, MouseWheelEventArgs e)
    {
        if (sender is TextBox tb)
        {
            var p = e.GetPosition(tb);
            var rx = p.X / tb.ActualWidth;
            var ry = p.Y / tb.ActualHeight;

            var delta = WheelDelta(e);

            var prop = TextBox.TextProperty;

            var binding = BindingOperations.GetBindingExpression(tb, prop);

            var val = binding?.Target.GetValue(prop);
            if (val is string s)
            {
                if (double.TryParse(s, NumberStyles.AllowDecimalPoint, CultureInfo.InvariantCulture, out var d))
                {
                    binding?.Target.SetValue(prop, (d + delta).ToString(CultureInfo.InvariantCulture));
                    binding?.UpdateSource();
                }
            }

            Dispatcher.BeginInvoke(() =>
            {
                var p2 = new Point(rx * tb.ActualWidth, ry * tb.ActualHeight);
                var l = tb.PointToScreen(p2);
                User32.SetCursorPos((int)l.X, (int)l.Y);
            }, DispatcherPriority.Loaded);
        }
    }
}
