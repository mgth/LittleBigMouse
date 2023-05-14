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

using System;
using System.Collections.Generic;
using System.Globalization;
using Avalonia.Controls;
using Avalonia.Data.Converters;
using HLab.Mvvm.Annotations;
using LittleBigMouse.DisplayLayout.Monitors;
using LittleBigMouse.Plugins;
using LittleBigMouse.Plugins.Avalonia;
using LittleBigMouse.Ui.Avalonia.Plugins.Debug;

namespace LittleBigMouse.Ui.Avalonia;

public class SizeRatioConverter : IMultiValueConverter
{
    public object? Convert(IList<object?> values, Type targetType, object? parameter, CultureInfo culture)
    {
        if (values == null || values.Count < 3) return null;

        if(values[0] is double ui && values[1] is double vm && values[3] is double size)
        {
            return (ui / vm) * size;
        }

        return null;
    }

    public object[] ConvertBack(object value, Type[] targetTypes, object parameter, CultureInfo culture)
    {
        throw new NotImplementedException();
    }
}

/// <summary>
/// Logique d'interaction pour MultiScreensGui.xaml
/// </summary>
public partial class MultiMonitorsLayoutPresenterView : UserControl
    , IView<DefaultViewMode,MonitorsLayoutPresenterViewModel>
    , IView<MonitorDebugViewMode,MonitorsLayoutPresenterViewModel>
    , IMonitorsLayoutPresenterViewClass
    , IMonitorsLayoutPresenterView
{
    public MultiMonitorsLayoutPresenterView()
    {
        InitializeComponent();

        //DataContextChanged += (a, b) =>
        //{
        //    if (b.OldValue is MultiScreensViewModel oldvm)
        //    {
        //        AllScreens_CollectionChanged(this, new NotifyCollectionChangedEventArgs(NotifyCollectionChangedAction.Remove,oldvm.Config.AllScreens));
        //        oldvm.Config.AllScreens.CollectionChanged -= AllScreens_CollectionChanged;
        //    }

        //    if (b.NewValue is MultiScreensViewModel newvm)
        //    {
        //        AllScreens_CollectionChanged(this, new NotifyCollectionChangedEventArgs(NotifyCollectionChangedAction.Add,newvm.Config.AllScreens));
        //        newvm.Config.AllScreens.CollectionChanged += AllScreens_CollectionChanged;
        //    }
        //};
    }

    //private void AllScreens_CollectionChanged(object sender, NotifyCollectionChangedEventArgs e)
    //{
    //    switch (e.Action)
    //    {
    //        case NotifyCollectionChangedAction.Add:
    //            if (e.NewItems != null)
    //            {
    //                foreach (var s in e.NewItems.OfType<Screen>())
    //                {
    //                    var view = new ViewLocator {Model = s,Tag=s.Id};
    //                    //var view = ViewModel.MvvmContext.GetView<ViewModeDefault>(s, typeof(IViewClassDefault));
    //                    ContentGrid.Children.Add((UIElement)view);
    //                    //if (view.Content is ScreenFrameView sfw)
    //                    //{
    //                    //    sfw.ViewModel.Presenter = ContentGrid;
    //                    //}
    //                }
    //            }
    //            break;
    //        case NotifyCollectionChangedAction.Remove:
    //            if (e.OldItems != null)
    //            {
    //                foreach (var s in e.OldItems.OfType<Screen>())
    //                {
    //                    foreach (var element in ContentGrid.Children.OfType<FrameworkElement>().ToList())
    //                    {
    //                        if (element.Tag.ToString() == s.Id)
    //                        {
    //                            ContentGrid.Children.Remove(element);
    //                        }
    //                    }
    //                }
    //            }
    //            else throw  new ArgumentException("OldItems should not be null for remove Action");
    //            break;
    //        case NotifyCollectionChangedAction.Replace:
    //        case NotifyCollectionChangedAction.Move:
    //        case NotifyCollectionChangedAction.Reset:
    //            throw new NotImplementedException();
    //        default:
    //            throw new ArgumentOutOfRangeException();
    //    }
    //}

    //protected override void OnRenderSizeChanged(SizeChangedInfo sizeInfo)
    //{
    //    base.OnRenderSizeChanged(sizeInfo);

    //    foreach (var frameView in Canvas.Children.OfType<ScreenFrameView>())
    //    {
    //        frameView.SetPosition();
    //    }
    //}

    public IMonitorsLayoutPresenterViewModel ViewModel => DataContext as MonitorsLayoutPresenterViewModel;

    IMonitorsLayout? Layout => ViewModel?.Model;

    public double GetRatio()
    {
        if (Layout == null) return 1;

        var all = Layout.PhysicalBounds;

        if (all.Width * all.Height > 0)
        {
            return Math.Min(
                ReferenceGrid.Bounds.Width / all.Width,
                ReferenceGrid.Bounds.Height / all.Height
            );
        }
        return 1;
    }

    public Panel MainPanel => ContentGrid;
    public Panel BackPanel => ContentGrid;

    void OnLayoutUpdated(object sender, EventArgs e)
    {
        var r = GetRatio();

        ViewModel.VisualRatio.X = r;
        ViewModel.VisualRatio.Y = r;
    }
}