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
using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;
using System.Windows.Media;
using System.Windows.Shapes;
using LbmScreenConfig;
using System.ComponentModel;
using System.Windows.Interop;
using WinAPI_User32;

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
        public event PropertyChangedEventHandler PropertyChanged;
        private readonly PropertyChangeHandler _change;

        readonly Screen _screen;
        readonly Screen _drawOn;

        readonly RulerSide _side;


        private void SetSize()
        {
            switch (_side)
            {
                case RulerSide.Right:
                    var leftTop = new PhysicalPoint(_drawOn.Config, _drawOn,_screen.PhysicalX,_screen.PhysicalY);
                    var leftBottom = new PhysicalPoint(_drawOn.Config, _drawOn, _screen.PhysicalX, _screen.PhysicalBounds.Bottom);

                    if (leftBottom.Y <= _drawOn.PhysicalY || leftTop.Y >= _drawOn.PhysicalBounds.Bottom)
                    {
                        Hide();
                    }
                    else
                    {
                        if (Enabled) Show();
                    }
                    break;

                case RulerSide.Left:
                    var rightTop = new PhysicalPoint(_drawOn.Config, _drawOn, _screen.PhysicalBounds.Right, _screen.PhysicalBounds.Top);
                    var rightBottom = new PhysicalPoint(_drawOn.Config, _drawOn, _screen.PhysicalBounds.Right, _screen.PhysicalBounds.Bottom);

                    if (rightBottom.Y <= _drawOn.PhysicalY || rightTop.Y >= _drawOn.PhysicalBounds.Bottom)
                    {
                        Hide();
                    }
                    else
                    {
                        if (Enabled) Show();
                    }
                    break;

                case RulerSide.Bottom:
                    PhysicalPoint topLeft = _screen.Bounds.TopLeft.ToScreen(_drawOn);
                    PhysicalPoint topRight = _screen.Bounds.TopRight.ToScreen(_drawOn);

                    if (topRight.X <= _drawOn.PhysicalX || topLeft.X >= _drawOn.PhysicalBounds.Right)
                    {
                        Hide();
                    }
                    else
                    {
                        if (Enabled) Show();
                    }
                    break;
                case RulerSide.Top:
                    PhysicalPoint bottomLeft = _screen.Bounds.BottomLeft.ToScreen(_drawOn);
                    PhysicalPoint bottomRight = _screen.Bounds.BottomRight.ToScreen(_drawOn);

                    if (bottomRight.X <= _drawOn.PhysicalX || bottomLeft.X >= _drawOn.PhysicalBounds.Right)
                    {
                        Hide();
                    }
                    else
                    {
                        if(Enabled) Show();
                    }
                    break;
                default:
                    throw new ArgumentOutOfRangeException();
            }
        }

        private bool _enabled = false;

        public bool Enabled
        {
            get { return _enabled; }
            set
            {
                if (_enabled != value)
                {
                    _enabled = true;
                    SetSize();
                }
            }
        }


        public Thickness RulerThickness //rev
        {
            get
            {
                switch (_side)
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

        public double WindowTop
        {
            get
            {
                if (_side == RulerSide.Bottom) return _drawOn.Bounds.BottomRight.Wpf.Y - WindowHeight;

                return _drawOn.Bounds.TopLeft.Wpf.Y;
            }
            set { }
        }
        public double WindowLeft
        {
            get
            {
                if (_side == RulerSide.Right) return _drawOn.Bounds.BottomRight.Wpf.X - WindowWidth;

                return _drawOn.Bounds.TopLeft.Wpf.X;
            }
            set { }
        }
        public double RulerLeft //rev
        {
            get { return _screen.Bounds.TopLeft.ToScreen(_drawOn).Wpf.X; }
            set { }
        }

        public double RulerTop //rev
        {
            get { return _screen.Bounds.TopLeft.ToScreen(_drawOn).Wpf.Y; }
            set { }
        }
        public double WindowWidth //rev
        {
            get
            {
                if (Horizontal) return _drawOn.WpfWidth;

                return 30 * _drawOn.PhysicalToWpfRatioX ;
              
            }
            set { }
        }
        public double RulerWidth //rev
        {
            get
            {
                if (Horizontal) return _screen.Bounds.BottomRight.ToScreen(_drawOn).Wpf.X - RulerLeft;

                return WindowWidth;
            }
            set { }
        }


        public double WindowHeight
        {
            get
            {
                if (Vertical) return _drawOn.Bounds.BottomRight.Wpf.Y - WindowTop;

                return 30 * _drawOn.PhysicalToWpfRatioY; // 3cm
            }
            set { }
        }

        public double RulerHeight //rev
        {
            get
            {
                if (Vertical) return _screen.Bounds.BottomRight.ToScreen(_drawOn).Wpf.Y - RulerTop;

                return WindowHeight;               
            }
            set { }
        }



        public Thickness CanvasMargin //rev
        {
            get
            {
                if (Vertical)
                {
                    return new Thickness(
                        0,
                        _screen.Bounds.TopLeft.ToScreen(_drawOn).Wpf.Y - WindowTop,
                        0,
                        (WindowTop + WindowHeight) - (RulerTop + RulerHeight)
                        );
                }
                 else
                {
                    return new Thickness(
                        _screen.Bounds.TopLeft.ToScreen(_drawOn).Wpf.X - WindowLeft,
                        0,
                        (WindowLeft + WindowWidth) - (RulerLeft + RulerWidth),
                        0
                        );
                }
             }
        }

        public Point GradientStartPoint //rev
        {
            get
            {
                switch (_side)
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

        public Point GradientEndPoint //rev
        {
            get
            {
                switch (_side)
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
            _change = new PropertyChangeHandler(this);
            _change.PropertyChanged += delegate(object sender, PropertyChangedEventArgs args) { PropertyChanged?.Invoke(sender, args); };

            _screen = screen;
            _drawOn = drawOn;
            _side = side;

            _screen.PropertyChanged += _screen_PropertyChanged;
            _drawOn.PropertyChanged += _screen_PropertyChanged;

            DataContext = this;

            InitializeComponent();


            DrawRuler();
            SetSize();
        }

        private void _screen_PropertyChanged(object sender, PropertyChangedEventArgs e)
        {
            if (!_refresh) return;
            switch (e.PropertyName)
            {
                case "PhysicalX":
                    if (_side == RulerSide.Top || _side == RulerSide.Bottom)
                    {
                        _change.RaiseProperty("CanvasMargin");
                        //_change.RaiseProperty("RulerLeft");
                        //_change.RaiseProperty("RulerWidth");
                    }
                    break;
                case "PhysicalWidth":
                    if (_side == RulerSide.Top || _side == RulerSide.Bottom)
                    {
                        _change.RaiseProperty("CanvasMargin");
                        //_change.RaiseProperty("RulerLeft");
                        //_change.RaiseProperty("RulerWidth");
                    DrawRuler();
                    }
                    break;
                case "PhysicalY":
                    if (_side == RulerSide.Left || _side == RulerSide.Right)
                    {
                        _change.RaiseProperty("CanvasMargin");
                        //_change.RaiseProperty("ActualRulerTop");
                        //_change.RaiseProperty("RulerTop");
                        //_change.RaiseProperty("RulerHeight");
                    }
                    break;
                case "PhysicalHeight":
                    if (_side == RulerSide.Left || _side == RulerSide.Right)
                    {
                        _change.RaiseProperty("CanvasMargin");
                        //_change.RaiseProperty("RulerTop");
                        //_change.RaiseProperty("RulerHeight");
                        DrawRuler();
                    }
                     break;
            }
        }

        [@DependsOn("Side")]
        public bool Vertical => (_side == RulerSide.Left) || (_side == RulerSide.Right);

        public bool Horizontal => !Vertical;

        private void DrawRuler()
        {
            Canvas.Children.Clear();

            bool revert = (_side == RulerSide.Right) || (_side == RulerSide.Bottom);

            double sizeRatio = Vertical?((1/_drawOn.WpfToPixelRatioX)/_drawOn.PitchX):((1/_drawOn.WpfToPixelRatioY)/_drawOn.PitchY);
            double lenghtRatio = Vertical?((1/_drawOn.WpfToPixelRatioY)/_drawOn.PitchY):((1/_drawOn.WpfToPixelRatioX)/_drawOn.PitchX);

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

            Point newPoint = PointToScreen(e.GetPosition(this)); // Mouse.CursorPos;
            //newPoint.Offset(
            //    _screen.Config.PhysicalBounds.X/_drawOn.PitchX, 
            //    _screen.Config.PhysicalBounds.Y/_drawOn.PitchY
            //    );

            if (Vertical)
            {
                double offset = (newPoint.Y - _oldPoint.Y)*_drawOn.PitchY;

                double old = _drawOn.PhysicalY;

                _drawOn.PhysicalY = _dragStartPoint.Y - offset;

                if (_drawOn.Primary && _drawOn.PhysicalY == old) _oldPoint.Y += offset / _drawOn.PitchY;
            }
            else
            {
                double old = _drawOn.PhysicalY;

                double offset = (newPoint.X - _oldPoint.X)*_drawOn.PitchX;

                _drawOn.PhysicalX = _dragStartPoint.X - offset;

                if (_drawOn.Primary && _drawOn.PhysicalX == old) _oldPoint.X += offset / _drawOn.PitchX;
            }
        }

        private void Window_Closing(object sender, System.ComponentModel.CancelEventArgs e)
        {
            _screen.PropertyChanged -= _screen_PropertyChanged;
        }

        private void Window_MouseDown(object sender, MouseButtonEventArgs e)
        {
            _oldPoint = PointToScreen(e.GetPosition(this));
            //_oldPoint.Offset(_screen.Config.PhysicalBounds.X/_drawOn.PitchX, _screen.Config.PhysicalBounds.Y/_drawOn.PitchY);

            _dragStartPoint = InvertControl?_drawOn.PhysicalLocation:_screen.PhysicalLocation;
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

            PhysicalPoint pos = new WpfPoint(_drawOn.Config, _drawOn,p.X,p.Y).Physical.ToScreen(_drawOn);

            if (Vertical)
            {
                _drawOn.PhysicalRatioY *= ratio;

                PhysicalPoint pos2 =
                    new WpfPoint(_drawOn.Config, _drawOn, p.X, p.Y).Physical.ToScreen(_drawOn);

                _drawOn.PhysicalY += pos.Y - pos2.Y;
            }
            else
            {
                _drawOn.PhysicalRatioX *= ratio;

                PhysicalPoint pos2 =
                    new WpfPoint(_drawOn.Config, _drawOn, p.X, p.Y).Physical.ToScreen(_drawOn);

                _drawOn.PhysicalX += pos.X - pos2.X;
            }
         }

        public void SuspendDrawing()
        {
            HwndSource source = (HwndSource)HwndSource.FromVisual(this);
            User32.SendMessage(source.Handle, User32.WM_SETREDRAW, false, 0);
        }

        public void ResumeDrawing()
        {
            HwndSource source = (HwndSource)HwndSource.FromVisual(this);
            User32.SendMessage(source.Handle, User32.WM_SETREDRAW, true, 0);
            //Refresh();
        }

    }
}
