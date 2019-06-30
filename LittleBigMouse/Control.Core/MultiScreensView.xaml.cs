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
using System.Collections.Specialized;
using System.Linq;
using System.Windows;
using System.Windows.Controls;
using HLab.Mvvm;
using HLab.Mvvm.Annotations;
using LittleBigMouse.ScreenConfigs;

namespace LittleBigMouse.Control.Core
{
    public class ViewModeMultiScreenBackground : ViewMode { }

    /// <summary>
    /// Logique d'interaction pour MultiScreensGui.xaml
    /// </summary>
    public partial class MultiScreensView : UserControl, IView<ViewModeDefault,MultiScreensViewModel>, IViewClassDefault
    {
        public MultiScreensView()
        {
            InitializeComponent();

            DataContextChanged += (a, b) =>
            {
                if (b.OldValue is MultiScreensViewModel oldvm)
                {
                    oldvm.Config.AllScreens.CollectionChanged -= AllScreens_CollectionChanged;
                }

                if (b.NewValue is MultiScreensViewModel newvm)
                {
                    AllScreens_CollectionChanged(this, new NotifyCollectionChangedEventArgs(NotifyCollectionChangedAction.Add,newvm.Config.AllScreens));
                    newvm.Config.AllScreens.CollectionChanged += AllScreens_CollectionChanged;
                }
            };
        }

        private void AllScreens_CollectionChanged(object sender, NotifyCollectionChangedEventArgs e)
        {
            switch (e.Action)
            {
                case NotifyCollectionChangedAction.Add:
                    if (e.NewItems != null)
                    {
                        foreach (var s in e.NewItems.OfType<Screen>())
                        {
                            var view = ViewModel.MvvmContext.GetView<ViewModeDefault>(s, typeof(IViewClassDefault));
                            Canvas.Children.Add((UIElement)view);
                        }
                    }
                    break;
                case NotifyCollectionChangedAction.Remove:
                    if (e.OldItems != null)
                    {
                        foreach (var s in e.OldItems.OfType<Screen>())
                        {
                            foreach (var element in Canvas.Children.OfType<FrameworkElement>().ToList())
                            {
                                if (element is ScreenFrameView view && ReferenceEquals(view.ViewModel.Model, s))
                                    Canvas.Children.Remove(element);
                            }
                        }
                    }
                    else throw  new ArgumentException("OldItems should not be null for remove Action");
                    break;
                case NotifyCollectionChangedAction.Replace:
                case NotifyCollectionChangedAction.Move:
                case NotifyCollectionChangedAction.Reset:
                    throw new NotImplementedException();
                default:
                    throw new ArgumentOutOfRangeException();
            }
        }

        //protected override void OnRenderSizeChanged(SizeChangedInfo sizeInfo)
        //{
        //    base.OnRenderSizeChanged(sizeInfo);

        //    foreach (var frameView in Canvas.Children.OfType<ScreenFrameView>())
        //    {
        //        frameView.SetPosition();
        //    }
        //}

        internal MultiScreensViewModel ViewModel => DataContext as MultiScreensViewModel;

        private ScreenConfig Config => ViewModel.Config;

        public double GetRatio()
        {
            if (Config == null) return 1;

            Rect all = Config.PhysicalOutsideBounds;

            if (all.Width * all.Height > 0)
            {
                return Math.Min(
                    BackgoundGrid.ActualWidth / all.Width,
                    BackgoundGrid.ActualHeight / all.Height
                );
            }
            return 1;
        }

        public double PhysicalToUiX(double x)
            => (x - Config.PhysicalOutsideBounds.Left - Config.PhysicalOutsideBounds.Width / 2) * GetRatio();


        public double PhysicalToUiY(double y)
            => (y - Config.PhysicalOutsideBounds.Top - Config.PhysicalOutsideBounds.Height / 2) * GetRatio();

    }
}
