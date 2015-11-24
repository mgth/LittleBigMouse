/*
  MouseControl - Mouse Managment in multi DPI monitors environment
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

using System;
using System.Collections.Generic;
using System.ComponentModel;
using System.Linq;
using System.Windows;
using LbmScreenConfig;
using System.Windows.Controls;
using System.Windows.Input;
using NativeHelpers;

namespace LittleBigMouse_Control
{
    /// <summary>
    /// Interaction logic for Config.xaml
    /// </summary>
    public partial class ConfigGui : PerMonitorDPIWindow, INotifyPropertyChanged
    {
        public event PropertyChangedEventHandler PropertyChanged;
        private readonly PropertyChangeHandler _change;

        public ScreenConfig Config { get; }

        public ScreenGui Selected
        {
            get
            { return AllScreenGuis.FirstOrDefault(gui => gui.Selected); }
            set
            {
                value.Selected = true;
            }
        }

        public IEnumerable<ScreenGui> AllScreenGuis
            => ScreensGrid.Children.Cast<UIElement>().OfType<ScreenGui>();

        public ScreenGui GetScreenGui(Screen s)
        {
            return AllScreenGuis.FirstOrDefault(gui => gui.Screen == s);
        } 

        private readonly WindowResizer _resizer;

        public ConfigGui()
        {
            _change = new PropertyChangeHandler(this);
            _change.PropertyChanged += delegate (object sender, PropertyChangedEventArgs args) { PropertyChanged?.Invoke(sender, args); };

            Config = new ScreenConfig();

            InitializeComponent();

            _resizer = new WindowResizer(this,ResizeGrid);

            DataContext = this;
            UpdatePhysicalOutsideBounds();

            LoadLocation(Config.PrimaryScreen);

            foreach (ScreenGui sgui in Config.AllScreens.Select(s => new ScreenGui(s,this)))
            {
                ScreensGrid.Children.Add(sgui);
                sgui.SelectedChanged += _gui_SelectedChanged;

                //sgui.PropertyChanged += delegate(object sender, PropertyChangedEventArgs args) { _change.RaiseProperty("ScreenGui."+args.PropertyName); };
            }

            SizeChanged += delegate(object sender, SizeChangedEventArgs args) { _change.RaiseProperty("Size"); };
            MainGrid.SizeChanged += delegate (object sender, SizeChangedEventArgs args) { _change.RaiseProperty("Size"); };

            LocationChanged += delegate(object sender, EventArgs args) { _change.RaiseProperty("Location"); };

            StateChanged += delegate(object sender, EventArgs args)
            {
                _change.RaiseProperty("State");
            };

            _change.Watch(Config,"Config");

            Config.PropertyChanged += Config_PropertyChanged;
            foreach (Screen screen in Config.AllScreens)
            {
                screen.PropertyChanged += Config_PropertyChanged;
            }
        }

        public bool LiveUpdate
        {
            get { return _liveUpdate; }
            set { _liveUpdate = value;
                if (_liveUpdate)
                    ActivateConfig();
                else
                {
                    LittleBigMouseClient.Client.LoadConfig();
                }
            }
        }

        private void Config_PropertyChanged(object sender, PropertyChangedEventArgs e)
        {
            ActivateConfig();
        }

        public void ActivateConfig()
        {
            if (LiveUpdate)
            {
                Save();
                LittleBigMouseClient.Client.LoadConfig();                         
            }
        }

    private Screen _locationScreen;
        public Screen LocationScreen
        {
            get
            {
                return _locationScreen??Config.PrimaryScreen;
            }
            set
            {
                if (value == null)
                    return;

                Screen old = _locationScreen;
                if (!_change.SetProperty(ref _locationScreen, value)) return;

                if (old!=null) old.PropertyChanged -= LocationScreenOnPropertyChanged;
                _locationScreen.PropertyChanged += LocationScreenOnPropertyChanged;
            }
        }

        private void LocationScreenOnPropertyChanged(object sender, PropertyChangedEventArgs propertyChangedEventArgs)
        {
            _change.RaiseProperty("LocationScreen." + propertyChangedEventArgs.PropertyName);
        }




        private bool _moving = false;
        public bool Moving
        {
            get { return _moving; }
            set { _change.SetProperty(ref _moving, value); }
        }

        
        private Rect _physicalOutsideBounds;
        public Rect PhysicalOutsideBounds => _physicalOutsideBounds;

        [DependsOn("Moving","Config.PhysicalOutsideBounds")]
        public void UpdatePhysicalOutsideBounds()
        {
            if (Moving) return;
            _change.SetProperty(ref _physicalOutsideBounds, Config.PhysicalOutsideBounds, "PhysicalOutsideBounds");
         }

        private double _ratio = 0;

        public double Ratio => _ratio;

        [DependsOn("Size", "Moving", "PhysicalOutsideBounds")]
        public void UpdateRatio()
        {
            Rect all = PhysicalOutsideBounds;

            double ratio = 0;

            if (all.Width*all.Height > 0)
            {
                ratio = Math.Min(
                    ScreensGrid.ActualWidth/all.Width,
                    ScreensGrid.ActualHeight/all.Height
                    );
            }
            _change.SetProperty(ref _ratio, ratio, "Ratio");
        }

        public double PhysicalToUIX(double x) => (x - PhysicalOutsideBounds.Left) * Ratio + (ScreensGrid.ActualWidth - PhysicalOutsideBounds.Width * Ratio) / 2 ;
        public double PhysicalToUIY(double y) => (y - PhysicalOutsideBounds.Top) * Ratio + (ScreensGrid.ActualHeight - PhysicalOutsideBounds.Height *Ratio) / 2;

        public Point PhysicalToUi(Point p)
        {
            return new Point(
                PhysicalToUIX(p.X),
                PhysicalToUIY(p.Y)
                );
        }

        public Point UiToPhysical(Point p)
        {
            Rect all = PhysicalOutsideBounds;

            return new Point(
                (p.X / Ratio) + all.Left,
                (p.Y / Ratio) + all.Top
                );
        }
        public Vector UiToPhysical(Vector V)
        {
            return new Vector(
                (V.X / Ratio),
                (V.Y / Ratio)
                );
        }

        private void _gui_SelectedChanged(object s, bool selected)
        {
            if (!selected) return;

            if (ShowRulers)
            {
                ShowRulers = false;
                ShowRulers = true;
            }
        }


        public void SaveLocation()
        {
                Point topLeft = LocationScreen.AbsoluteWorkingArea.TopLeft.Wpf.Point; ;
                Point bottomRight = LocationScreen.AbsoluteWorkingArea.BottomRight.Wpf.Point;

                LocationScreen.GuiLocation = 
                new Rect(
                    new Point(
                        (Left - topLeft.X) / (bottomRight.X - topLeft.X),
                        (Top - topLeft.Y) / (bottomRight.Y - topLeft.Y)
                        ),
                    new Size(
                        Width / (bottomRight.X - topLeft.X),
                        Height / (bottomRight.Y - topLeft.Y)
                       )
                    );
         }


        private bool _doNotSaveLocation = false;
        public void LoadLocation(Screen s)
        {
            _doNotSaveLocation = true;

            LocationScreen = s;

            //Point topLeft = LocationScreen.Bounds.TopLeft.Wpf.Point;
            //Point bottomRight = LocationScreen.Bounds.BottomRight.Wpf.Point;

            Point topLeft = LocationScreen.AbsoluteWorkingArea.TopLeft.Wpf.Point;//Bounds.TopLeft.Wpf.Point;
            Point bottomRight = LocationScreen.AbsoluteWorkingArea.BottomRight.Wpf.Point;//Bounds.BottomRight.Wpf.Point;

            Top = topLeft.Y + (bottomRight.Y - topLeft.Y) * LocationScreen.GuiLocation.Top;
            Left =  topLeft.X + (bottomRight.X - topLeft.X) * LocationScreen.GuiLocation.Left;
            Width = (bottomRight.X - topLeft.X) * LocationScreen.GuiLocation.Width;
            Height = (bottomRight.Y - topLeft.Y) * LocationScreen.GuiLocation.Height;

            _doNotSaveLocation = false;
        }        

        private void Save()
        {
            Config.Save();
        }

        private void cmdOk_Click(object sender, RoutedEventArgs e)
        {
            cmdApply_Click(sender,e);
            Close();
        }

        private void cmdApply_Click(object sender, RoutedEventArgs e)
        {
            Save();
            LittleBigMouseClient.Client.LoadAtStartup(Config.LoadAtStartup);
            LittleBigMouseClient.Client.LoadConfig();
            LittleBigMouseClient.Client.Start();
        }

        private void cmdCancel_Click(object sender, RoutedEventArgs e)
        {
            Close();
        }

        private void Window_Closing(object sender, CancelEventArgs e)
        {
            ShowRulers = false;
        }

        private void cmdUnload_Click(object sender, RoutedEventArgs e)
        {
            LittleBigMouseClient.Client.Quit();
        }

        private bool _showRulers = false;

        public bool ShowRulers
        {
            get { return _showRulers; }
            set
            {
                if (_change.SetProperty(ref _showRulers, value))
                {
                    if (_showRulers)
                    {
                        AddRuler(RulerSide.Left);
                        AddRuler(RulerSide.Right);
                        AddRuler(RulerSide.Top);
                        AddRuler(RulerSide.Bottom);

                        foreach (Ruler ruler in _rulers) ruler.Enabled = true;
                    
                    }
                    else    
                    {
                        foreach (Ruler sz in _rulers)
                        {
                            sz.Close();
                        }
                        _rulers.Clear();                  
                    }
                }
            }
        }

        private readonly List<Ruler> _rulers = new List<Ruler>();
        private bool _liveUpdate = false;

        private void AddRuler(RulerSide side)
        {
            if (Selected == null) return;

            foreach (var sz in Config.AllScreens.Select(s => new Ruler(Selected.Screen, s, side)))
            {
                _rulers.Add(sz);
            }
        }

        public void ShiftPhysicalBounds(Vector shift)
        {
            Rect r = new Rect(
                new Point(
                    PhysicalOutsideBounds.X+ shift.X, 
                    PhysicalOutsideBounds.Y+ shift.Y
                    )
                    , PhysicalOutsideBounds.Size
                    );
            _change.SetProperty(ref _physicalOutsideBounds, r, "PhysicalOutsideBounds");
        }


        private void MainGrid_OnMouseDown(object sender, MouseButtonEventArgs e)
        {
            if (e.ClickCount == 2)
            {
                //SetFullScreen(Config.PrimaryScreen);



                //WindowState = WindowState==WindowState.Maximized?WindowState.Normal : WindowState.Maximized;
            }
            else
                _resizer.Sizer_DragWindow(sender,e);
        }

        public void SetFullScreen(Screen s)
        {
            if (s != LocationScreen)
            {
                LoadLocation(s);
            }
            else 
                WindowState = (WindowState == WindowState.Maximized) ? WindowState.Normal : WindowState.Maximized;
        }

        [DependsOn("Location")]
        public Screen DrawnOnScreen
        {
            get
            {
                var p1 = new WpfPoint(Config, Selected.Screen, Left, Top);
                return p1.TargetScreen;
            }
        }

        [DependsOn("DrawnOnScreen","State", "ShowRulers")]
        public GridLength HorizontalResizerSize 
            => WindowState == WindowState.Maximized && ShowRulers ? 
            new GridLength(30 * LocationScreen.PhysicalToWpfRatioX / ScaleFactor) : new GridLength(10);

        [DependsOn("DrawnOnScreen", "State", "ShowRulers")]
        public GridLength VerticalResizerSize 
            => WindowState ==  WindowState.Maximized && ShowRulers ?
            new GridLength(30 * LocationScreen.PhysicalToWpfRatioY / ScaleFactor) : new GridLength(10);

        private void ConfigGui_OnLocationChanged(object sender, EventArgs e)
        {
            if (_doNotSaveLocation) return;

            AbsolutePoint p = new WpfPoint(Config,null,Left + Width/2,Top + Height/2);
            if (p.TargetScreen != LocationScreen)
            {
                double width = Width / LocationScreen.WpfToPixelRatioX;
                double height = Height / LocationScreen.WpfToPixelRatioY;

                LocationScreen = p.TargetScreen;

                Width = width * LocationScreen.WpfToPixelRatioX;
                Height = height * LocationScreen.WpfToPixelRatioY;

                GetScreenGui(LocationScreen).Selected = true;
            }
                
            SaveLocation();
        }
    }

    public static class FrameworkElementExt
    {
        public static void BringToFront(this FrameworkElement element)
        {
            Panel parent = element?.Parent as Panel;
            if (parent == null) return;

            var maxZ = parent.Children.OfType<UIElement>()
              .Where(x => x != element)
              .Select(Panel.GetZIndex)
              .Max();
            Panel.SetZIndex(element, maxZ + 1);
        }
    }


}
