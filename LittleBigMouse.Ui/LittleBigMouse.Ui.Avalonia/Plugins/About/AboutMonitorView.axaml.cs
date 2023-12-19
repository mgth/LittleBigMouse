/*
  LittleBigMouse.Control.Core
  Copyright (c) 2021 Mathieu GRENET.  All right reserved.

  This file is part of LittleBigMouse.Control.Core.

    LittleBigMouse.Control.Core is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    LittleBigMouse.Control.Core is distributed in the hope that it will be useful,
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
using Avalonia.VisualTree;
using HLab.Mvvm.Annotations;
using LittleBigMouse.Plugins;
using LittleBigMouse.Ui.Avalonia.MonitorFrame;

namespace LittleBigMouse.Ui.Avalonia.Plugins.About;

public partial class AboutMonitorView : UserControl, IView<MonitorAboutViewMode, AboutMonitorViewModel>, IMonitorFrameContentViewClass
{
    public AboutMonitorView()
    {
        InitializeComponent();
    }

    public MonitorFrameView Frame => this.FindAncestorOfType<MonitorFrameView>();


    // TODO : Avalonia
    //void OnKeyEnterUpdate(object sender, KeyEventArgs e)
    //{
    //    ViewHelper.OnKeyEnterUpdate(sender, e);
    //}
    public double WheelDelta(PointerWheelEventArgs e)
    {
        double delta = (e.Delta.Y > 0) ? 1 : -1;
        if ((e.KeyModifiers & KeyModifiers.Control) != 0) delta /= 10;
        return delta;
    }

    void InputElement_OnPointerPressed(object? sender, PointerPressedEventArgs e)
    {
        throw new System.NotImplementedException();
    }
}