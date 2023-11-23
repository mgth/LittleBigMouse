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
using System.Windows;

namespace LittleBigMouse.Plugin.Location.Plugins.Location.Rulers;

/// <summary>
/// Logique d'interaction pour Tester.xaml
/// </summary>
public partial class Tester : Window
{
    public Tester()
    {
        InitializeComponent();

        DataContextChanged += Tester_DataContextChanged;

        SizeChanged += Tester_SizeChanged;
        LocationChanged += Tester_LocationChanged;
        StateChanged += Tester_StateChanged;
    }

    private void Tester_DataContextChanged(object sender, DependencyPropertyChangedEventArgs e)
    {
        SetValues();
        (e.NewValue as TesterViewModel).PropertyChanged += Tester_PropertyChanged;
    }

    private void Tester_PropertyChanged(object sender, System.ComponentModel.PropertyChangedEventArgs e)
    {
        SetBack();
    }

    public void SetBack()
    {
        Hide();
        var l = Vm.LeftInDip;
        var t = Vm.TopInDip;
        var w = Vm.WidthInDip;
        var h = Vm.HeightInDip;

        Left = l - 1;
        Left = l;
        Top = t;
        //FinalWidth = w;
        //Height = h;
        Show();
    }

    private void Tester_StateChanged(object sender, EventArgs e)
    {
        // SetValues();
    }

    private void Tester_LocationChanged(object sender, EventArgs e)
    {
        // SetValues();
    }

    private TesterViewModel Vm => DataContext as TesterViewModel;

    private void Tester_SizeChanged(object sender, SizeChangedEventArgs e)
    {
        // SetValues();
    }

    private void SetValues()
    {
        Vm.TopInDip = Top;
        Vm.LeftInDip = Left;

        Vm.BottomInDip = Top + ActualHeight;
        Vm.RightInDip = Left + ActualWidth;

        if (ActualHeight > 0) Vm.HeightInDip = ActualHeight;
        if (ActualWidth > 0) Vm.WidthInDip = ActualWidth;

    }

    private void ButtonBase_OnClick(object sender, RoutedEventArgs e)
    {
        SetValues();
    }
}
