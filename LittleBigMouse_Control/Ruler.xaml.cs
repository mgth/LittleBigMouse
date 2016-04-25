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
using System;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;
using System.Windows.Media;
using System.Windows.Shapes;
using LbmScreenConfig;
using System.ComponentModel;
using System.Windows.Interop;
using NotifyChange;
using WinAPI;

namespace LittleBigMouse_Control
{
    /// <summary>
    /// Logique d'interaction pour Sizer.xaml
    /// </summary>
    /// 

    public enum RulerSide
    {
        Top,
        Bottom,
        Left,
        Right
    }

 
    public partial class Ruler : Window, INotifyPropertyChanged
    {
        // PropertyChanged Handling
        private readonly NotifierHelper _notify;
        public event PropertyChangedEventHandler PropertyChanged { add { _notify.Add(value); } remove { _notify.Remove(value);} }

        private Screen _screen;
        public Screen Screen
        {
            get { return _screen; }
            set { _notify.SetAndWatch(ref _screen, value); }
        }

        private Screen _drawOn;
        public Screen DrawOn
        {
            get { return _drawOn; }
            set { _notify.SetAndWatch(ref _drawOn, value); }
        }

        private bool _enabled = false;
        public bool Enabled
        {
            get { return _enabled; }
            set { _notify.SetProperty(ref _enabled, value); }
        }

        private RulerSide _side;

        public RulerSide Side
        {
            get { return _side; }
            set { _notify.SetProperty(ref _side, value); }
        }


        [DependsOn("Enabled", "Screen.PhysicalBounds", "DrawOn.PhysicalBounds")]
        private void SetSize()
        {
            if (!Enabled || DrawOn == null || Screen == null)
            {
                Hide();
                return;
            }

            switch (Side)
            {
                case RulerSide.Right:
                    var leftTop = new PhysicalPoint(DrawOn.Config, DrawOn, Screen.PhysicalX, Screen.PhysicalY);
                    var leftBottom = new PhysicalPoint(DrawOn.Config, DrawOn, Screen.PhysicalX, Screen.PhysicalBounds.Bottom);

                    if (leftBottom.Y <= DrawOn.PhysicalY || leftTop.Y >= DrawOn.PhysicalBounds.Bottom)
                    {
                        Hide();
                    }
                    else
                    {
                        Show();
                    }
                    break;

                case RulerSide.Left:
                    var rightTop = new PhysicalPoint(DrawOn.Config, DrawOn, Screen.PhysicalBounds.Right, Screen.PhysicalBounds.Top);
                    var rightBottom = new PhysicalPoint(DrawOn.Config, DrawOn, Screen.PhysicalBounds.Right, Screen.PhysicalBounds.Bottom);

                    if (rightBottom.Y <= DrawOn.PhysicalY || rightTop.Y >= DrawOn.PhysicalBounds.Bottom)
                    {
                        Hide();
                    }
                    else
                    {
                        Show();
                    }
                    break;

                case RulerSide.Bottom:
                    PhysicalPoint topLeft = Screen.Bounds.TopLeft.ToScreen(DrawOn);
                    PhysicalPoint topRight = Screen.Bounds.TopRight.ToScreen(DrawOn);

                    if (topRight.X <= DrawOn.PhysicalX || topLeft.X >= DrawOn.PhysicalBounds.Right)
                    {
                        Hide();
                    }
                    else
                    {
                        Show();
                    }
                    break;
                case RulerSide.Top:
                    PhysicalPoint bottomLeft = Screen.Bounds.BottomLeft.ToScreen(DrawOn);
                    PhysicalPoint bottomRight = Screen.Bounds.BottomRight.ToScreen(DrawOn);

                    if (bottomRight.X <= DrawOn.PhysicalX || bottomLeft.X >= DrawOn.PhysicalBounds.Right)
                    {
                        Hide();
                    }
                    else
                    {
                        Show();
                    }
                    break;
                default:
                    throw new ArgumentOutOfRangeException();
            }
        }


        [DependsOn("Side")]
        public Thickness RulerThickness //rev
        {
            get
            {
                switch (Side)
                {
                    case RulerSide.Top:
                        return new Thickness(1, 0, 1, 0);
                    case RulerSide.Bottom:
                        return new Thickness(1, 0, 1, 0);
                    case RulerSide.Left:
                        return new Thickness(0, 1, 0, 1);
                    case RulerSide.Right:
                        return new Thickness(0, 1, 0, 1);
                    default:
                        throw new ArgumentOutOfRangeException();
                }
            }
        }

        [DependsOn("Side", "DrawOn.PhysicalBounds", nameof(WindowHeight))]
        public double WindowTop
        {
            get
            {
                if (Side == RulerSide.Bottom) return DrawOn.Bounds.BottomRight.Wpf.Y - WindowHeight;

                return DrawOn.Bounds.TopLeft.Wpf.Y;
            }
            set { }
        }

        [DependsOn("Side", "DrawOn.PhysicalBounds", nameof(WindowWidth))]
        public double WindowLeft
        {
            get
            {
                if (Side == RulerSide.Right) return DrawOn.Bounds.BottomRight.Wpf.X - WindowWidth;

                return DrawOn.Bounds.TopLeft.Wpf.X;
            }
            set { }
        }

        [DependsOn("DrawOn.PhysicalBounds", "Screen.PhysicalBounds")]
        public double RulerLeft //rev
        {
            get { return Screen.Bounds.TopLeft.ToScreen(DrawOn).Wpf.X; }
            set { }
        }

        [DependsOn("Side", "DrawOn.PhysicalBounds", "Screen.PhysicalBounds")]
        public double RulerTop //rev
        {
            get { return Screen.Bounds.TopLeft.ToScreen(DrawOn).Wpf.Y; }
            set { }
        }

        [DependsOn("Horizontal", "DrawOn.WpfWidth", "DrawOn.PhysicalToWpfRatioX")]
        public double WindowWidth //rev
        {
            get
            {
                if (Horizontal) return DrawOn.WpfWidth;

                return 30 * DrawOn.PhysicalToWpfRatioX ;
              
            }
            set { }
        }
        [DependsOn("Horizontal", "Screen.PhysicalBounds", nameof(RulerLeft), nameof(WindowWidth))]
        public double RulerWidth //rev
        {
            get
            {
                if (Horizontal) return Screen.Bounds.BottomRight.ToScreen(DrawOn).Wpf.X - RulerLeft;

                return WindowWidth;
            }
            set { }
        }


        [DependsOn("DrawOn.Bounds", nameof(WindowTop), "DrawOn.PhysicalToWpfRatioY")]
        public double WindowHeight
        {
            get
            {
                if (Vertical) return DrawOn.Bounds.BottomRight.Wpf.Y - WindowTop;

                return 30 * DrawOn.PhysicalToWpfRatioY; // 3cm
            }
            set { }
        }

        [DependsOn("Screen.Bounds", "DrawOn.Bounds", nameof(RulerTop), nameof(WindowHeight))]
        public double RulerHeight //rev
        {
            get
            {
                if (Vertical) return Screen.Bounds.BottomRight.ToScreen(DrawOn).Wpf.Y - RulerTop;

                return WindowHeight;               
            }
            set { }
        }


        private Thickness _canvasMargin;

        public Thickness CanvasMargin //rev
        {
            get { return _canvasMargin; }
            private set
            {
                _notify.SetProperty(ref _canvasMargin, value);
            }
        }

        [DependsOn("Screen.Bounds", "Screen.PhysicalBounds", "DrawOn.Bounds", "DrawOn.PhysicalBounds", "WindowTop", "WindowLeft", "WindowHeight", "RulerTop", "RulerLeft",  "RulerHeight", "RulerWidth")]
        public void UpdateCanvasMargin()
        {
            if (DrawOn == null) return;
            if (Screen == null) return;

                if (Vertical)
                {
                CanvasMargin = new Thickness(
                        0,
                        Screen.Bounds.TopLeft.ToScreen(DrawOn).Wpf.Y - WindowTop,
                        0,
                        (WindowTop + WindowHeight) - (RulerTop + RulerHeight)
                        );
                }
                 else
                {
                CanvasMargin = new Thickness(
                        Screen.Bounds.TopLeft.ToScreen(DrawOn).Wpf.X - WindowLeft,
                        0,
                        (WindowLeft + WindowWidth) - (RulerLeft + RulerWidth),
                        0
                        );
                }
        }

        [DependsOn(nameof(Side))]
        public Point GradientStartPoint //rev
        {
            get
            {
                switch (Side)
                {
                    case RulerSide.Top: return new Point(0.5, 0);
                    case RulerSide.Bottom: return new Point(0.5, 1);
                    case RulerSide.Left: return new Point(0, 0.5);
                    case RulerSide.Right: return new Point(1, 0.5);
                    default:
                        throw new ArgumentOutOfRangeException();
                }
            }
        }

        [DependsOn(nameof(Side))]
        public Point GradientEndPoint //rev
        {
            get
            {
                switch (Side)
                {
                    case RulerSide.Top: return new Point(0.5, 1);
                    case RulerSide.Bottom: return new Point(0.5, 0);
                    case RulerSide.Left: return new Point(1, 0.5);
                    case RulerSide.Right: return new Point(0, 0.5);
                    default:
                        throw new ArgumentOutOfRangeException();
                }
            }
        }

        public Ruler(Screen screen, Screen drawOn, RulerSide side)
        {
            _notify = new NotifierHelper(this);

            Side = side;

            DataContext = this;

            InitializeComponent();

            Screen = screen;
            DrawOn = drawOn;

            DrawRuler();

            _notify.InitNotifier();
        }


        //private void _screen_PropertyChanged(object sender, PropertyChangedEventArgs e)
        //{
        //    if (!_refresh) return;
        //    switch (e.PropertyName)
        //    {
        //        case "PhysicalX":
        //            if (_side == RulerSide.Top || _side == RulerSide.Bottom)
        //            {
        //                _notify.RaiseProperty("CanvasMargin");
        //            }
        //            break;
        //        case "PhysicalWidth":
        //            if (_side == RulerSide.Top || _side == RulerSide.Bottom)
        //            {
        //                _notify.RaiseProperty("CanvasMargin");
        //                DrawRuler();
        //            }
        //            break;
        //        case "PhysicalY":
        //            if (_side == RulerSide.Left || _side == RulerSide.Right)
        //            {
        //                _notify.RaiseProperty("CanvasMargin");
        //            }
        //            break;
        //        case "PhysicalHeight":
        //            if (_side == RulerSide.Left || _side == RulerSide.Right)
        //            {
        //                _notify.RaiseProperty("CanvasMargin");
        //                DrawRuler();
        //            }
        //             break;
        //    }
        //}

        [DependsOn(nameof(Side))]
        public bool Vertical => (_side == RulerSide.Left) || (_side == RulerSide.Right);

        [DependsOn(nameof(Side))]
        public bool Horizontal => !Vertical;

        private void DrawRuler()
        {
            Canvas.Children.Clear();

            bool revert = (_side == RulerSide.Right) || (_side == RulerSide.Bottom);

            double sizeRatio = Vertical?((1/ DrawOn.WpfToPixelRatioX)/ DrawOn.PitchX):((1/ DrawOn.WpfToPixelRatioY)/ DrawOn.PitchY);
            double lenghtRatio = Vertical?((1/ DrawOn.WpfToPixelRatioY)/ DrawOn.PitchY):((1/ DrawOn.WpfToPixelRatioX)/ DrawOn.PitchX);

            double length = Vertical?_screen.PhysicalHeight:_screen.PhysicalWidth;
            double width = Vertical ? WindowWidth : WindowHeight;

            int mm = 0;
            while (true)
            {
                if (mm >= (int)length + 1) break;

                double pos = mm * lenghtRatio;

                double size = sizeRatio;
                string text = null;
                Brush stroke = new SolidColorBrush(Colors.Black);

                if (mm >= length)
                {
                    pos = length * lenghtRatio;
                    size *= 30.0;
                    stroke = new SolidColorBrush(Colors.Red);
                }
                else if (mm == 0)
                {
                    size *= 30.0;
                    stroke = new SolidColorBrush(Colors.Red);
                }
                else if (mm%100 == 0)
                {
                    size *= 20.0;
                    text = (mm/100).ToString();
                }
                else if (mm%50 == 0)
                {
                    size *= 15.0;
                    text = "5";
                }
                else if (mm%10 == 0)
                {
                    size *= 10.0;
                    text = ((mm%100)/10).ToString();
                }
                else if (mm%5 == 0)
                {
                    size *= 5.0;
                }
                else
                {
                    size *= 2.5;
                }

                if (text != null)
                {
                    TextBlock t = new TextBlock
                    {
                        Text = text, FontSize = size/3,
                    };

                    t.SetValue(Vertical?Canvas.TopProperty:Canvas.LeftProperty, pos);
                    t.SetValue(Vertical?Canvas.LeftProperty:Canvas.TopProperty, (!revert) ? (size - t.FontSize) : (width - size));                 

                    Canvas.Children.Add(t);
                }

                Line l = new Line
                    {
                        X1 = Vertical?((revert) ? width - size : 0):pos,
                        X2 = Vertical ? ((revert) ? width : size):pos,
                        Y1 = Vertical ? pos:(revert ? width - size : 0),
                        Y2 = Vertical ? pos:((revert) ? width : size),
                        Stroke = stroke,
                        StrokeThickness = 0.1* lenghtRatio
                }; 
                      
                 Canvas.Children.Add(l);
                mm++;
            }
        }



        private Point _oldPoint;
        private Point _dragStartPoint;

        private void Window_MouseMove(object sender, MouseEventArgs e)
        {
            if (!_moving || _dragStartPoint == null) return;

            Point newPoint = PointToScreen(e.GetPosition(this)); // LbmMouse.CursorPos;
            //newPoint.Offset(
            //    _screen.Config.PhysicalBounds.X/DrawOn.PitchX, 
            //    _screen.Config.PhysicalBounds.Y/DrawOn.PitchY
            //    );

            if (Vertical)
            {
                double offset = (newPoint.Y - _oldPoint.Y)* DrawOn.PitchY;

                double old = DrawOn.PhysicalY;

                DrawOn.PhysicalY = _dragStartPoint.Y - offset;

                if (DrawOn.Primary && DrawOn.PhysicalY == old) _oldPoint.Y += offset / DrawOn.PitchY;
            }
            else
            {
                double old = DrawOn.PhysicalY;

                double offset = (newPoint.X - _oldPoint.X)* DrawOn.PitchX;

                DrawOn.PhysicalX = _dragStartPoint.X - offset;

                if (DrawOn.Primary && DrawOn.PhysicalX == old) _oldPoint.X += offset / DrawOn.PitchX;
            }
        }

        private void Window_MouseDown(object sender, MouseButtonEventArgs e)
        {
            _oldPoint = PointToScreen(e.GetPosition(this));
            //_oldPoint.Offset(_screen.Config.PhysicalBounds.X/DrawOn.PitchX, _screen.Config.PhysicalBounds.Y/DrawOn.PitchY);

            _dragStartPoint = InvertControl? DrawOn.PhysicalLocation:_screen.PhysicalLocation;
            _moving = true;
            CaptureMouse();
        }

        private void Window_MouseUp(object sender, MouseButtonEventArgs e)
        {
            if (_moving)
            {
                _moving = false;
                ReleaseMouseCapture();
            }
        }

        private bool _moving { get; set; } = false;
        private bool _refresh { get; set; } = true;
        public bool InvertControl { get; set; } = true;

        private void Ruler_OnMouseWheel(object sender, MouseWheelEventArgs e)
        {
            double ratio = (e.Delta > 0) ? 1.005 : 1/1.005;

            Point p = e.GetPosition(this);

            PhysicalPoint pos = new WpfPoint(DrawOn.Config, DrawOn, p.X,p.Y).Physical.ToScreen(DrawOn);

            if (Vertical)
            {
                DrawOn.PhysicalRatioY *= ratio;

                PhysicalPoint pos2 =
                    new WpfPoint(DrawOn.Config, DrawOn, p.X, p.Y).Physical.ToScreen(DrawOn);

                DrawOn.PhysicalY += pos.Y - pos2.Y;
            }
            else
            {
                DrawOn.PhysicalRatioX *= ratio;

                PhysicalPoint pos2 =
                    new WpfPoint(DrawOn.Config, DrawOn, p.X, p.Y).Physical.ToScreen(DrawOn);

                DrawOn.PhysicalX += pos.X - pos2.X;
            }
         }

        public void SuspendDrawing()
        {
            HwndSource source = (HwndSource)HwndSource.FromVisual(this);
            NativeMethods.SendMessage(source.Handle, NativeMethods.WM_SETREDRAW, false, 0);
        }

        public void ResumeDrawing()
        {
            HwndSource source = (HwndSource)HwndSource.FromVisual(this);
            NativeMethods.SendMessage(source.Handle, NativeMethods.WM_SETREDRAW, true, 0);
            //Refresh();
        }

    }
}
