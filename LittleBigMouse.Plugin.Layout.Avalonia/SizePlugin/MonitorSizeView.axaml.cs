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
using Avalonia.Threading;
using HLab.Mvvm.Annotations;
using HLab.Mvvm.ReactiveUI;
using HLab.Sys.Windows.API;
using LittleBigMouse.Plugins;

namespace LittleBigMouse.Plugin.Layout.Avalonia.SizePlugin;

public partial class MonitorSizeView : UserControl, IView<ViewModeScreenSize, ScreenSizeViewModel>, IMonitorFrameContentViewClass
{
    public MonitorSizeView()
    {
        InitializeComponent();

        this.SizeChanged += MonitorSizeView_SizeChanged;
    }

    private void MonitorSizeView_SizeChanged(object? sender, SizeChangedEventArgs e)
    {
        if(DataContext is ScreenSizeViewModel vm)
            vm.UpdateArrows(Bounds);
    }

    protected override void OnMeasureInvalidated()
    {
        base.OnMeasureInvalidated();
    }

    void OnKeyEnterUpdate(object sender, KeyEventArgs e)
    {
        // TODO Avalonia
        // ViewHelper.OnKeyEnterUpdate(sender, e);
    }

    static double WheelDelta(PointerWheelEventArgs e)
    {
        double delta = (e.Delta.Y > 0) ? 1 : -1;
        if ((e.KeyModifiers & KeyModifiers.Control) != 0) delta /= 10;
        if ((e.KeyModifiers & KeyModifiers.Shift) != 0) delta *= 10;
        return delta;
    }

    void OnMouseWheel(object sender, PointerWheelEventArgs e)
    {
        if (sender is TextBox tb)
        {
            var p = e.GetPosition(tb);
            var rx = p.X / tb.Bounds.Width;
            var ry = p.Y / tb.Bounds.Height;

            var delta = WheelDelta(e);

            var prop = TextBox.TextProperty;

            /* TODO Avalonia

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
            */

            Dispatcher.UIThread.InvokeAsync(() =>
            {
                var p2 = new Point(rx * tb.Bounds.Width, ry * tb.Bounds.Height);
                var l = tb.PointToScreen(p2);
                WinUser.SetCursorPos((int)l.X, (int)l.Y);
            }, DispatcherPriority.Loaded);
        }
    }
}