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
using Avalonia.Controls;

using HLab.Mvvm.Annotations;

using LittleBigMouse.DisplayLayout.Monitors;
using LittleBigMouse.Plugins;
using LittleBigMouse.Plugins.Avalonia;
using LittleBigMouse.Ui.Avalonia.Plugins.Debug;
using HLab.Mvvm.Avalonia;

namespace LittleBigMouse.Ui.Avalonia;

/// <summary>
/// Logique d'interaction pour MultiScreensGui.xaml
/// </summary>
public partial class MultiMonitorsLayoutPresenterView : UserControl
    , IView<DefaultViewMode,IMonitorsLayoutPresenterViewModel>
    , IView<MonitorDebugViewMode,IMonitorsLayoutPresenterViewModel>
    , IMonitorsLayoutPresenterViewClass
    , IMonitorsLayoutPresenterView
{
    public MultiMonitorsLayoutPresenterView()
    {
        InitializeComponent();
    }


    //    foreach (var frameView in Canvas.Children.OfType<ScreenFrameView>())
    //    {
    //        frameView.SetPosition();
    //    }
    //}

    public IMonitorsLayout? Layout => ViewModel?.Model;

    public IMonitorsLayoutPresenterViewModel? ViewModel  => this.GetViewModel<IMonitorsLayoutPresenterViewModel>(DataContext);

    public double GetRatio()
    {
        if (Layout == null) return 1.0;

        var all = Layout.PhysicalBounds;

        if (all.Width * all.Height > 0.0)
        {
            return Math.Min(
                ReferenceGrid.Bounds.Width / all.Width,
                ReferenceGrid.Bounds.Height / all.Height
            );
        }
        return 1.0;
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