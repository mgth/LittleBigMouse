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
using HLab.Base.Avalonia.Controls;
using HLab.Mvvm.Annotations;
using HLab.Sys.Windows.API;
using LittleBigMouse.Plugins;
using LittleBigMouse.Plugins.Avalonia;

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
        if (sender is not DoubleBox db) return;

        var p = e.GetPosition(db);
        var rx = p.X / db.Bounds.Width;
        var ry = p.Y / db.Bounds.Height;

        db.Value += WheelDelta(e);
        this.GetLayout()?.Compact();

        Dispatcher.UIThread.InvokeAsync(() =>
        {
            var p2 = new Point(rx * db.Bounds.Width, ry * db.Bounds.Height);
            var l = db.PointToScreen(p2);
            WinUser.SetCursorPos((int)l.X, (int)l.Y);
        }, DispatcherPriority.Loaded);
    }
}