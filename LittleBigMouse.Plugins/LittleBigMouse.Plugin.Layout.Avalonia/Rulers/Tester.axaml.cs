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
using Avalonia.Interactivity;

namespace LittleBigMouse.Plugin.Layout.Avalonia.Rulers;

/// <summary>
/// Logique d'interaction pour Tester.xaml
/// </summary>
public partial class Tester : Window
{
    public Tester()
    {
        InitializeComponent();

        SizeChanged += Tester_SizeChanged;

        PositionChanged += Tester_PositionChanged;

    }

    protected override void OnDataContextChanged(EventArgs e)
    {
        SetValues();
        (DataContext as TesterViewModel).PropertyChanged += Tester_PropertyChanged;

        base.OnDataContextChanged(e);
    }


    //private void Tester_DataContextChanged(object sender, EventArgs e)
    //{
    //    SetValues();
    //    (e.NewValue as TesterViewModel).PropertyChanged += Tester_PropertyChanged;
    //}

    void Tester_PropertyChanged(object sender, System.ComponentModel.PropertyChangedEventArgs e)
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

        Position = Position.WithX((int)l - 1); // TODO : should be in dip but is in pixels
        Position = new PixelPoint((int)l, (int)t);

        //FinalWidth = w;
        //Height = h;
        Show();
    }


    // todo : avalonia override
    protected  void HandleWindowStateChanged(WindowState state)
    {
        // SetValues();
        //base.HandleWindowStateChanged(state);
    }

    void Tester_PositionChanged(object sender, EventArgs e)
    {
        // SetValues();
    }

    TesterViewModel Vm => DataContext as TesterViewModel;

    void Tester_SizeChanged(object sender, SizeChangedEventArgs e)
    {
        // SetValues();
    }

    void SetValues()
    {
        var top = Vm.TopInDip = Position.Y; // TODO : should be in dip but is in pixels
        var left = Vm.LeftInDip = Position.X;

        Vm.BottomInDip = top + Bounds.Height;
        Vm.RightInDip = left + Bounds.Width;

        if (Bounds.Height > 0) Vm.HeightInDip = Bounds.Height;
        if (Bounds.Width > 0) Vm.WidthInDip = Bounds.Width;

    }

    void ButtonBase_OnClick(object sender, RoutedEventArgs e)
    {
        SetValues();
    }
}
