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
using Avalonia.Interactivity;
using HLab.Mvvm.Annotations;
using LittleBigMouse.Plugins;

namespace LittleBigMouse.Plugin.Vcp.Avalonia;

/// <summary>
/// Logique d'interaction pour ScreenGuiSizer.xaml
/// </summary>
public partial class VcpScreenView : UserControl, IView<MonitorVcpViewMode, VcpScreenViewModel>, IMonitorFrameContentViewClass
{
    public VcpScreenView()
    {
        InitializeComponent();
    }

    new VcpScreenViewModel ViewModel => DataContext as VcpScreenViewModel;

    void ButtonOff_OnClick(object sender, RoutedEventArgs e)
    {
        if (ViewModel?.Vcp == null) return;
        ViewModel.Vcp.Power = false;
    }

    void WakeUp_OnClick(object sender, RoutedEventArgs e)
    {
        if (ViewModel?.Vcp == null) return;
        ViewModel.Vcp.Power = true;
    }
    //private void ButtonOn_OnClick(object sender, RoutedEventArgs e)
    //{
    //    uint code = Convert.ToUInt32(txtCode.Text, 16);
    //    uint value = Convert.ToUInt32(txtValue.Text, 16);

    //    uint pvct;
    //    uint current;
    //    uint max;

    //    Dxva2.GetVCPFeatureAndVCPFeatureReply(ScreenGui.Screen.HPhysical, code, out pvct, out current, out max);

    //    Debug.Print(pvct.ToString() + ":" + current.ToString() + "<" + max.ToString());

    //    Dxva2.SetVCPFeature(ScreenGui.Screen.HPhysical, code, value);
    //    //for (uint i = 0; i < max; i++)
    //    //{
    //    //    if (i==5 && code==0xD6) continue; 
    //    //    bool result = Dxva2.SetVCPFeature(Screen.HPhysical, code, i);
    //    //    Debug.Print(i.ToString() + (result?":O":"X"));
    //    //}

    //    //IntPtr desk = User32.GetDesktopWindow();
    //    //IntPtr win = User32.FindWindowEx(IntPtr.Zero, IntPtr.Zero, null, null);

    //    //User32.SendMessage(-1, User32.WM_SYSCOMMAND, User32.SC_MONITORPOWER, 2);
    //    //User32.SendMessage(-1, User32.WM_SYSCOMMAND, User32.SC_MONITORPOWER, -1);
    //}

    void ProbeLowLuminance_OnClick(object sender, RoutedEventArgs e)
    {
        ViewModel?.ProbeLowLuminance();
    }

    void ProbeLuminance_OnClick(object sender, RoutedEventArgs e)
    {
        ViewModel?.ProbeLowLuminance();
    }


    void Probe_OnClick(object sender, RoutedEventArgs e)
    {
        ViewModel?.Probe();
    }

    void Tune_OnClick(object sender, RoutedEventArgs e)
    {
        ViewModel?.Tune();
    }


    void UIElement_OnMouseLeftButtonDown(object sender, PointerPressedEventArgs e)
    {
    }

    void Save_OnClick(object sender, RoutedEventArgs e)
    {
        ViewModel?.Save();
    }

}
