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

using Avalonia.Controls;
using Avalonia.Input;
using HLab.Base.Avalonia.DependencyHelpers;
using HLab.Mvvm.Annotations;
using HLab.Sys.Windows.MonitorVcp;

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

    public VcpSlider()
    {
        InitializeComponent();
    }

    void OnPointerWheelChanged(object? sender, PointerWheelEventArgs e)
    {
        Delta(e.Delta.Y);
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