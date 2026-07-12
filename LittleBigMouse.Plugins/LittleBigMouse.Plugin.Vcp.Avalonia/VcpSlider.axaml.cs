/*
  LittleBigMouse.Plugin.Vcp
  Copyright (c) 2021 Mathieu GRENET.  All right reserved.

  This file is part of LittleBigMouse.Plugin.Vcp.

    LittleBigMouse.Plugin.Vcp is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    LittleBigMouse.Plugin.Vcp is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with MouseControl.  If not, see <http://www.gnu.org/licenses/>.

	  mailto:mathieu@mgth.fr
	  http://www.mgth.fr
*/

using System;
using Avalonia.Controls;
using Avalonia.Input;
using Avalonia.Media;
using Avalonia.Media.Immutable;
using HLab.Base.Avalonia.DependencyHelpers;
using HLab.Mvvm.Annotations;
using HLab.Sys.Windows.MonitorVcp;
using ReactiveUI;

namespace LittleBigMouse.Plugin.Vcp.Avalonia;

public class VcpSliderViewModelDesign : VcpSliderViewModel, IDesignViewModel
{
    public VcpSliderViewModelDesign()
    {
        Model = new MonitorLevelDesign(VcpComponent.Red);
    }
}

/// <summary>
/// Logique d'interaction pour VcpSlider.xaml
/// </summary>
public partial class VcpSlider : UserControl, IView<DefaultViewMode,VcpSliderViewModel>
{
    class H : DependencyHelper<VcpSlider> { }

    IDisposable? _channelSub;

    public VcpSlider()
    {
        InitializeComponent();

        DataContextChanged += (_, _) =>
        {
            _channelSub?.Dispose();
            _channelSub = (DataContext as VcpSliderViewModel)?
                .WhenAnyValue(vm => vm.ChannelColor)
                .Subscribe(ApplyChannelColor);
        };
    }

    // Fluent draws the slider from theme resources: override them locally so the
    // whole bar and thumb take the RGB channel color of the level being edited.
    void ApplyChannelColor(Color? channel)
    {
        if (channel is not { } c) return;

        var fill = new ImmutableSolidColorBrush(c);
        var rail = new ImmutableSolidColorBrush(c, 0.35);

        string[] fillKeys =
        [
            "SliderTrackValueFill", "SliderTrackValueFillPointerOver", "SliderTrackValueFillPressed",
            "SliderThumbBackground", "SliderThumbBackgroundPointerOver", "SliderThumbBackgroundPressed"
        ];
        string[] railKeys = ["SliderTrackFill", "SliderTrackFillPointerOver", "SliderTrackFillPressed"];

        foreach (var key in fillKeys) LevelSlider.Resources[key] = fill;
        foreach (var key in railKeys) LevelSlider.Resources[key] = rail;
    }

    void OnPointerWheelChanged(object? sender, PointerWheelEventArgs e)
    {
        Delta(e.Delta.Y);
        // the wheel adjusts the level: don't let the panel's ScrollViewer scroll too
        e.Handled = true;
    }

    void Delta(double delta)
    {
        if (DataContext is not VcpSliderViewModel vm) return;
 
        switch (delta)
        {
            case > 0:
                vm.Up();
                break;
            case < 0:
                vm.Down();
                break;
        }
    }

}