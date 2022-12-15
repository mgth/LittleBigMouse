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

using System.Windows;
using System.Windows.Input;
using Avalonia.Controls;
using Avalonia.Input;
using Avalonia.Interactivity;
using HLab.Base.Wpf;
using HLab.Mvvm.Annotations;
using LittleBigMouse.Control.Sys;

namespace LittleBigMouse.Control.Main
{
    /// <summary>
    /// Interaction logic for Config.xaml
    /// </summary>
    public partial class MainView : Window,IView<ViewModeDefault,MainControlViewModel>, IViewClassDefault
    {
        public MainView()
        {
            InitializeComponent();
        }

        void Window_Loaded(object? sender, RoutedEventArgs routedEventArgs)
        {
            //this.EnableBlur();
        }

        public void OnMouseDown(object? sender, PointerPressedEventArgs pointerPressedEventArgs)
        {
        }

    }
}
