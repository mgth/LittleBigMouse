/*
  MouseControl - LbmMouse Managment in multi DPI monitors environment
  Copyright (c) 2015 Mathieu GRENET.  All right reserved.

  This file is part of MouseControl.

    ArduixPL is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    ArduixPL is distributed in the hope that it will be useful,
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
using System.Windows.Input;
using WindowsMonitors;
using LbmScreenConfig;
using Erp.Mvvm;

namespace LittleBigMouse.ControlCore
{
    /// <summary>
    /// Interaction logic for Config.xaml
    /// </summary>
    public partial class MainView : IView<ViewModeDefault,MainViewModel>, IViewClassDefault
    {

        private readonly WindowResizer _resizer;

        public MainView()
        {
            MonitorsService.D.UpdateDevices();
            InitializeComponent();
            _resizer = new WindowResizer(this,ResizeGrid);
        }

        private void Window_Loaded(object sender, RoutedEventArgs e)
        {
            this.EnableBlur();
        }


        //private Screen _locationScreen;
        //public Screen LocationScreen
        //{
        //    get
        //    {
        //        return _locationScreen??Config.PrimaryScreen;
        //    }
        //    set
        //    {
        //        if (value == null)
        //            return;

        //        Screen old = _locationScreen;
        //        if (!_change.Set(ref _locationScreen, value)) return;

        //        if (old!=null) old.PropertyChanged -= LocationScreenOnPropertyChanged;
        //        _locationScreen.PropertyChanged += LocationScreenOnPropertyChanged;
        //    }
        //}

        //private void LocationScreenOnPropertyChanged(object sender, PropertyChangedEventArgs propertyChangedEventArgs)
        //{
        //    _change.RaiseProperty("LocationScreen." + propertyChangedEventArgs.PropertyName);
        //}








        //public void SaveLocation()
        //{
        //        Point topLeft = LocationScreen.AbsoluteWorkingArea.TopLeft.Dip.Point; ;
        //        Point bottomRight = LocationScreen.AbsoluteWorkingArea.BottomRight.Dip.Point;

        //        LocationScreen.GuiLocation = 
        //        new Rect(
        //            new Point(
        //                (Left - topLeft.X) / (bottomRight.X - topLeft.X),
        //                (Top - topLeft.Y) / (bottomRight.Y - topLeft.Y)
        //                ),
        //            new Size(
        //                Width / (bottomRight.X - topLeft.X),
        //                Height / (bottomRight.Y - topLeft.Y)
        //               )
        //            );
        // }


        //private bool _doNotSaveLocation = false;
        //public void LoadLocation(Screen s)
        //{
        //    _doNotSaveLocation = true;

        //    LocationScreen = s;

        //    //Point topLeft = LocationScreen.Bounds.TopLeft.Dip.Point;
        //    //Point bottomRight = LocationScreen.Bounds.BottomRight.Dip.Point;

        //    Point topLeft = LocationScreen.AbsoluteWorkingArea.TopLeft.Dip.Point;//Bounds.TopLeft.Dip.Point;
        //    Point bottomRight = LocationScreen.AbsoluteWorkingArea.BottomRight.Dip.Point;//Bounds.BottomRight.Dip.Point;

        //    Top = topLeft.Y + (bottomRight.Y - topLeft.Y) * LocationScreen.GuiLocation.Top;
        //    Left =  topLeft.X + (bottomRight.X - topLeft.X) * LocationScreen.GuiLocation.Left;
        //    Width = (bottomRight.X - topLeft.X) * LocationScreen.GuiLocation.Width;
        //    Height = (bottomRight.Y - topLeft.Y) * LocationScreen.GuiLocation.Height;

        //    _doNotSaveLocation = false;
        //}        






        public void OnMouseDown(object sender, MouseButtonEventArgs e)
        {
            if (e.ClickCount == 2)
            {
                //SetFullScreen(Config.PrimaryScreen);



                WindowState = WindowState==WindowState.Maximized?WindowState.Normal : WindowState.Maximized;
            }
            else
                _resizer.Sizer_DragWindow(sender,e);
        }



        //[TriggedOn("Location")]
        //public Screen DrawnOnScreen
        //{
        //    get
        //    {
        //        var p1 = new DipPoint(Config, Config.Selected, Left, Top);
        //        return p1.TargetScreen;
        //    }
        //}

        //[TriggedOn("DrawnOnScreen","State", "ShowRulers")]
        //public GridLength HorizontalResizerSize 
        //    => WindowState == WindowState.Maximized /*&& ShowRulers*/ ? 
        //    new GridLength(30 * LocationScreen.MmToDipRatioX / ScaleFactor) : new GridLength(10);

        //[TriggedOn("DrawnOnScreen", "State", "ShowRulers")]
        //public GridLength VerticalResizerSize 
        //    => WindowState ==  WindowState.Maximized /*&& ShowRulers*/ ?
        //    new GridLength(30 * LocationScreen.MmToDipRatioY / ScaleFactor) : new GridLength(10);

        //private void ConfigGui_OnLocationChanged(object sender, EventArgs e)
        //{
        //    if (_doNotSaveLocation) return;

        //    AbsolutePoint p = new DipPoint(Config,null,Left + Width/2,Top + Height/2);
        //    if (p.TargetScreen != LocationScreen)
        //    {
        //        double width = Width / LocationScreen.WpfToPixelRatioX;
        //        double height = Height / LocationScreen.WpfToPixelRatioY;

        //        LocationScreen = p.TargetScreen;

        //        Width = width * LocationScreen.WpfToPixelRatioX;
        //        Height = height * LocationScreen.WpfToPixelRatioY;

        //        //GetScreenGui(LocationScreen).Selected = true;
        //    }
                
        //    SaveLocation();
        //}
    }

    public static class FrameworkElementExt
    {
        public static void BringToFront(this FrameworkElement element)
        {
            Panel parent = element?.Parent as Panel;
            // bring element to front so we can move it over the others
            if (parent == null) return;
            parent.Children.Remove(element);
            parent.Children.Add(element);



            //var maxZ = parent.Children.OfType<UIElement>()
            //  .Where(x => x != element)
            //  .Select(Panel.GetZIndex)
            //  .Max();
            //Panel.SetZIndex(element, maxZ + 1);
        }
    }


}
