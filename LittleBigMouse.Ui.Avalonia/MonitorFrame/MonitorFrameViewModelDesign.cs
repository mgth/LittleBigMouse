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

//#define uglyfix

using System.ComponentModel;
using System.Windows.Input;
using HLab.Mvvm.Annotations;
using LittleBigMouse.DisplayLayout.Dimensions;
using LittleBigMouse.DisplayLayout.Monitors;
using LittleBigMouse.Plugins;
using LittleBigMouse.Ui.Avalonia.Main;

namespace LittleBigMouse.Ui.Avalonia.MonitorFrame;

public class MonitorFrameViewModelDesign : MonitorFrameViewModel, IDesignViewModel
{
    class MonitorsPresenterDesign : IMonitorsLayoutPresenterViewModel
    {
        public IDisplayRatio VisualRatio => new DisplayRatioValue(1.0);
        public IMonitorsLayout Model { get; }

        public IMainPluginsViewModel MainViewModel => new MainViewModelDesign();

        public IMonitorFrameViewModel? SelectedMonitor { get ; set ; }
        public ICommand ResetLocationsFromSystem { get; }
        public ICommand ResetSizesFromSystem { get; }

        public event PropertyChangedEventHandler? PropertyChanged;
    }


    public MonitorFrameViewModelDesign()
    {
        Model = new PhysicalMonitor.Design();
        MonitorsPresenter = new MonitorsPresenterDesign();
    }

    //class DisplaySizeDesign : IDisplaySize
    //{
    //    public double Width { get => 160; set { } } 
    //    public double Height { get => 90; set { } } 
    //    public double X { get => 0; set { } } 
    //    public double Y { get => 0; set { } }

    //    public double TopBorder { get => 20; set { } } 
    //    public double BottomBorder { get => 20; set { } } 
    //    public double LeftBorder { get => 20; set { } } 
    //    public double RightBorder  { get => 20; set { } } 
    //    public Rect Bounds => new Rect(X, Y, Width, Height);
    //    public Point Center => new Point(X + Width / 2, Y + Height / 2);
    //    public Rect OutsideBounds => new Rect(X - LeftBorder, Y - TopBorder, Width + LeftBorder + RightBorder, Height + TopBorder + BottomBorder);
    //    public double OutsideWidth => Width + LeftBorder + RightBorder;
    //    public double OutsideHeight => Height + TopBorder + BottomBorder;
    //    public double OutsideX => X - LeftBorder;
    //    public double OutsideY => Y - TopBorder;
    //    public Point Location => new Point(X, Y);

    //    public bool Equals(IDisplaySize? other)
    //    {
    //        throw new NotImplementedException();
    //    }
    //}

    //public new IDisplaySize Unrotated => new DisplaySizeDesign(); 
}