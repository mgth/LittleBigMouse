/*
  HLab.Windows.MonitorVcp
  Copyright (c) 2021 Mathieu GRENET.  All right reserved.

  This file is part of HLab.Windows.MonitorVcp.

    HLab.Windows.MonitorVcp is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    HLab.Windows.MonitorVcp is distributed in the hope that it will be useful,
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
using HLab.Base.Avalonia.DependencyHelpers;
using HLab.Sys.Windows.Monitors;

//using System.Drawing;

namespace HLab.Sys.Windows.MonitorVcp.Avalonia;

using H = DependencyHelper<Luminance>;

/// <summary>
/// Logique d'interaction pour Luminance.xaml
/// </summary>
public partial class Luminance : UserControl
{
    public Luminance()
    {
        InitializeComponent();

        DataContext = new LuminanceViewModel();

        // TODO : Avalonia
        // SetBinding(MonitorsProperty, new Binding("Config") { Mode = BindingMode.TwoWay });
    }

    LuminanceViewModel ViewModel => DataContext as LuminanceViewModel;

    public static readonly StyledProperty<ISystemMonitorsService> MonitorsProperty = H.Property<ISystemMonitorsService>().Register();

    public ISystemMonitorsService Monitors
    {
        get => (ISystemMonitorsService)GetValue(MonitorsProperty); set
        {
            ViewModel.Monitors = value;
            SetValue(MonitorsProperty, value);
        }
    }

    public double WheelDelta(PointerWheelEventArgs e)
    {
        double delta = (e.Delta.Y > 0) ? 1 : -1;
        if ((e.KeyModifiers & KeyModifiers.Control) != 0) delta /= 10;
        return delta;
    }

    public void OnMouseWheel(object sender, PointerWheelEventArgs e)
    {
        ViewModel.Value += WheelDelta(e);
    }

    void Control_OnMouseDoubleClick(object sender, TappedEventArgs e)
    {
        ViewModel.Value = ViewModel.MaxAll;
    }

    void UIElement_OnMouseLeftButtonDown(object sender, PointerPressedEventArgs e)
    {
        if (e.ClickCount == 2) ViewModel.Value = ViewModel.MinAll;
    }

}