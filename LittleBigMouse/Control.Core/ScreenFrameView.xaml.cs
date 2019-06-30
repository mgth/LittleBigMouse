/*
  LittleBigMouse.Control.Core
  Copyright (c) 2017 Mathieu GRENET.  All right reserved.

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
using System;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Data;
using System.Windows.Input;
using HLab.Mvvm.Annotations;
using HLab.Mvvm.Wpf;

namespace LittleBigMouse.Control.Core
{
    public partial class ScreenFrameView : UserControl , IView<ViewModeDefault, ScreenFrameViewModel>, IViewClassDefault
    {
        public ScreenFrameViewModel ViewModel => DataContext as ScreenFrameViewModel;

        public ScreenFrameView()
        {
            LayoutUpdated += ScreenFrameView_LayoutUpdated;
            InitializeComponent();
        }

        private void ScreenFrameView_LayoutUpdated(object sender, EventArgs e)
        {
            if (ViewModel == null) return;
            if (Presenter==null) return;
            ViewModel.Ratio = Presenter.GetRatio();
        }

        public MultiScreensView Presenter => this.FindParent<MultiScreensView>();

        private void TextBox_MouseWheel(object sender, MouseWheelEventArgs e)
        {
            TextBox tBox = (TextBox)sender;

            double delta = (e.Delta > 0) ? 1 : -1;

            DependencyProperty prop = TextBox.TextProperty;

            BindingExpression binding = BindingOperations.GetBindingExpression(tBox, prop);
            binding?.Target.SetValue(prop, (double.Parse(binding?.Target.GetValue(prop).ToString()) + delta).ToString() );
            binding?.UpdateSource();
        }

        private void ResetPlace_Click(object sender, RoutedEventArgs e)
        {
            ViewModel.Model.Config.SetPhysicalAuto();
        }

        private void ResetSize_Click(object sender, RoutedEventArgs e)
        {
            ViewModel.Model.ScreenModel.InitSize(ViewModel.Model.Monitor);
        }
        public double GetRatio()
        {
            if (ViewModel.Model.Config == null) return 1;

            Rect all = ViewModel.Model.Config.PhysicalOutsideBounds;

            if (all.Width * all.Height > 0)
            {
                return Math.Min(
                    this.FindParent<MultiScreensView>().Canvas.ActualWidth / all.Width,
                    this.FindParent<MultiScreensView>().Canvas.ActualHeight / all.Height
                );
            }
            return 1;

        }
    }
}

