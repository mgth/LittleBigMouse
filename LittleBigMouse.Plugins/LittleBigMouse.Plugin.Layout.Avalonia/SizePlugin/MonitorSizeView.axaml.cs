/*
  LittleBigMouse.Plugin.Layout.Avalonia
  Copyright (c) 2021 Mathieu GRENET.  All right reserved.

  This file is part of LittleBigMouse.Plugin.Layout.Avalonia.

    LittleBigMouse.Plugin.Layout.Avalonia is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    LittleBigMouse.Plugin.Layout.Avalonia is distributed in the hope that it will be useful,
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
using HLab.Base.Avalonia.Controls;
using HLab.Mvvm.Annotations;
using LittleBigMouse.Plugins;
using LittleBigMouse.Plugins.Avalonia;

namespace LittleBigMouse.Plugin.Layout.Avalonia.SizePlugin;

public partial class MonitorSizeView : UserControl, IView<ScreenSizeViewMode, ScreenSizeViewModel>, IMonitorFrameContentViewClass
{
    readonly WheelPointerCapture _wheelPointerCapture = new();

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

        db.Value += WheelDelta(e);
        this.GetLayout()?.Compact();
        _wheelPointerCapture.KeepOn(db, e.Pointer);
    }
}
