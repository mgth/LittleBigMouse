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
using Avalonia.Interactivity;
using HLab.Base.Avalonia.Controls;
using HLab.Mvvm.Annotations;
using LittleBigMouse.Plugins;
using LittleBigMouse.Plugins.Avalonia;

namespace LittleBigMouse.Plugin.Layout.Avalonia.BorderResistancePlugin;

// The view model used to be declared as SizePlugin's ScreenSizeViewModel — copied
// along with the rest of that plugin's skeleton. It compiled because both derive
// from ViewModel<PhysicalMonitor> and the bindings only ever went through Model,
// but it meant BorderResistanceViewModel was never actually instantiated.
public partial class BorderResistanceView : UserControl, IView<BorderResistanceViewMode, BorderResistanceViewModel>, IMonitorFrameContentViewClass
{
    readonly WheelPointerCapture _wheelPointerCapture = new();

    public BorderResistanceView()
    {
        InitializeComponent();
    }

    BorderResistanceViewModel? ViewModel => DataContext as BorderResistanceViewModel;


    /// <summary>
    /// Leaving the view mode — or closing the main window — takes the on-screen
    /// bands with it, as the rulers do. LittleBigMouse lives on in the tray with its
    /// window shut, so topmost bands left behind would have nothing to dismiss them.
    /// </summary>
    protected override void OnUnloaded(RoutedEventArgs e)
    {
        if (ViewModel != null) ViewModel.ShowOnScreens = false;

        base.OnUnloaded(e);
    }

    void OnMirror(object? sender, RoutedEventArgs e)
    {
        var selected = ViewModel?.Selected;
        if (selected == null) return;

        selected.Side.MirrorToFacingEdge(selected.Model);
        this.GetLayout()?.Compact();
    }

    void OnDelete(object? sender, RoutedEventArgs e)
    {
        var selected = ViewModel?.Selected;
        if (selected == null) return;

        selected.Side.Delete(selected.Model);
        BorderSectionSelection.Select(null);
        this.GetLayout()?.Compact();
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

        // Floored here as well as in the model: the wheel would otherwise walk the
        // box down into negatives that the model silently drops.
        db.Value = Math.Max(0, db.Value + WheelDelta(e));
        this.GetLayout()?.Compact();
        _wheelPointerCapture.KeepOn(db, e.Pointer);
    }
}
