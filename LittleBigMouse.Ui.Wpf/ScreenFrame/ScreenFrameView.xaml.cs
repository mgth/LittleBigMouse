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
using System.Windows.Controls;
using HLab.Mvvm.Annotations;
using HLab.Mvvm.Extensions;
using LittleBigMouse.Control.Wpf;
using LittleBigMouse.Plugins;
using LittleBigMouse.Plugins.Wpf;

namespace LittleBigMouse.Control.ScreenFrame
{
    public partial class ScreenFrameView : UserControl , IView<ViewModeDefault, ScreenFrameViewModel>, IViewClassDefault, IScreenFrameView
    {
        public IScreenFrameViewModel ViewModel => DataContext as ScreenFrameViewModel;

        public ScreenFrameView()
        {
            InitializeComponent();

            Loaded += ScreenFrameView_Loaded;
        }

        void ScreenFrameView_Loaded(object sender, RoutedEventArgs e)
        {
            var parent = this.FindVisualParent<MultiScreensView>();
            ViewModel.Presenter = parent.DataContext as IMultiScreensViewModel;
        }

        //TODO : replace with commands
        void ResetPlace_Click(object sender, RoutedEventArgs e)
        {
            ViewModel.Model.Layout.SetPhysicalAuto();
        }

        void ResetSize_Click(object sender, RoutedEventArgs e)
        {
            ViewModel.Model.Model.InitSize(ViewModel.Model.ActiveSource.Device);
        }

    }
}

