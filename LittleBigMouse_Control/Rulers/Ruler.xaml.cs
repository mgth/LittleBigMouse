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
using LittleBigMouse_Control.Rulers;
using WinAPI;

namespace LittleBigMouse_Control
{
    /// <summary>
    /// Logique d'interaction pour Sizer.xaml
    /// </summary>
    /// 


 
    public partial class Ruler : Window
    {
        private bool IsClosed
        {
            get
            {
                for (int i = 0; i < Application.Current.Windows.Count; i++)
                {
                    var window = Application.Current.Windows[i];
                    if(window != null && window.Equals(this))
                        return false;
                }
                return true;
            }
        }

        public bool IsClosing = false;












        public Ruler()
        {
            InitializeComponent();
            DataContextChanged += Ruler_DataContextChanged;
            Closing += Ruler_Closing; ;
        }

        private void Ruler_Closing(object sender, CancelEventArgs e)
        {
            IsClosing = true;
        }

        private void Ruler_DataContextChanged(object sender, DependencyPropertyChangedEventArgs e)
        {
            if (e.OldValue != null) (e.OldValue as ViewModel).PropertyChanged -= ViewModel_PropertyChanged;
            if (e.NewValue != null) (e.NewValue as ViewModel).PropertyChanged += ViewModel_PropertyChanged;
            SetLocation();
            DrawRuler();
        }


        private void SetLocation()
        {
            Hide();
            Width = 0;
            Height = 0;            
            Top = ViewModel.WindowTop;
            Left = ViewModel.WindowLeft;
            Width = ViewModel.WindowWidth;
            Height = ViewModel.WindowHeight;   
            Show();
        }

        private void ViewModel_PropertyChanged(object sender, PropertyChangedEventArgs e)
        {
            switch (e.PropertyName)
            {
                case "WindowTop":
                case "WindowLeft":
                case "WindowWidth":
                case "WindowHeight":
                    SetLocation();
                    break;
                case "Width":
                case "Length":
                case "LengthRatio":
                case "SizeRatio":
                case "Revert":
                case "Vertical":

                    DrawRuler();
                    break;
                default:
                    return;
            }
        }

        public RulerViewModel ViewModel => DataContext as RulerViewModel;


        private void SetSizes()
        {
            Top = ViewModel.WindowTop;
            Left = ViewModel.WindowLeft;
            Width = ViewModel.WindowWidth;
            Height = ViewModel.WindowHeight;
        }

        private void DrawRuler()
        {
            Canvas.Children.Clear();
            SetSizes();

            //if (double.IsInfinity(ViewModel.Width)) return;

            int mm = 0;
            while (true)
            {
                if (mm >= (int)ViewModel.Length + 1) break;

                double pos = mm * ViewModel.LenghtRatio;

                double size = ViewModel.SizeRatio;
                string text = null;
                Brush stroke = new SolidColorBrush(Colors.Black);

                if (mm >= ViewModel.Length)
                {
                    pos = ViewModel.Length * ViewModel.LenghtRatio;
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

                    t.SetValue(ViewModel.Vertical ? Canvas.TopProperty:Canvas.LeftProperty, pos);
                    t.SetValue(ViewModel.Vertical ? Canvas.LeftProperty:Canvas.TopProperty, (!ViewModel.Revert) ? (size - t.FontSize) : (ViewModel.Width - size));                 

                    Canvas.Children.Add(t);
                }

                Line l = new Line
                    {
                        X1 = ViewModel.Vertical ? ((ViewModel.Revert) ? ViewModel.Width - size : 0):pos,
                        X2 = ViewModel.Vertical ? ((ViewModel.Revert) ? ViewModel.Width : size):pos,
                        Y1 = ViewModel.Vertical ? pos:(ViewModel.Revert ? ViewModel.Width - size : 0),
                        Y2 = ViewModel.Vertical ? pos:((ViewModel.Revert) ? ViewModel.Width : size),
                        Stroke = stroke,
                        StrokeThickness = 0.1* ViewModel.LenghtRatio
                }; 
                      
                 Canvas.Children.Add(l);
                mm++;
            }
        }



        private Point _oldPoint;
        private Point _dragStartPoint;

        private void Window_MouseMove(object sender, MouseEventArgs e)
        {
            if (!Moving || _dragStartPoint == null) return;

            Point newPoint = PointToScreen(e.GetPosition(this)); // LbmMouse.CursorPos;
            //newPoint.Offset(
            //    _screen.Config.BoundsInMm.X/DrawOn.PitchX, 
            //    _screen.Config.BoundsInMm.Y/DrawOn.PitchY
            //    );

            if (ViewModel.Vertical)
            {
                double offset = (newPoint.Y - _oldPoint.Y)* ViewModel.DrawOn.PitchY;

                double old = ViewModel.DrawOn.YLocationInMm;

                ViewModel.DrawOn.YLocationInMm = _dragStartPoint.Y - offset;

                if (ViewModel.DrawOn.Primary && ViewModel.DrawOn.YLocationInMm == old) _oldPoint.Y += offset / ViewModel.DrawOn.PitchY;
            }
            else
            {
                double old = ViewModel.DrawOn.YLocationInMm;

                double offset = (newPoint.X - _oldPoint.X)* ViewModel.DrawOn.PitchX;

                ViewModel.DrawOn.XLocationInMm = _dragStartPoint.X - offset;

                if (ViewModel.DrawOn.Primary && ViewModel.DrawOn.XLocationInMm == old) _oldPoint.X += offset / ViewModel.DrawOn.PitchX;
            }
        }

        private void Window_MouseDown(object sender, MouseButtonEventArgs e)
        {
            _oldPoint = PointToScreen(e.GetPosition(this));
            //_oldPoint.Offset(_screen.Config.BoundsInMm.X/DrawOn.PitchX, _screen.Config.BoundsInMm.Y/DrawOn.PitchY);

            _dragStartPoint = InvertControl? ViewModel.DrawOn.LocationInMm: ViewModel.Screen.LocationInMm;
            Moving = true;
            CaptureMouse();
        }

        private void Window_MouseUp(object sender, MouseButtonEventArgs e)
        {
            if (Moving)
            {
                Moving = false;
                ReleaseMouseCapture();
            }
        }

        private bool Moving { get; set; } = false;
        private bool Refresh { get; set; } = true;
        public bool InvertControl { get; set; } = true;

        private void Ruler_OnMouseWheel(object sender, MouseWheelEventArgs e)
        {
            double ratio = (e.Delta > 0) ? 1.005 : 1/1.005;

            Point p = e.GetPosition(this);

            PhysicalPoint pos = new DipPoint(ViewModel.DrawOn.Config, ViewModel.DrawOn, p.X,p.Y).Mm.ToScreen(ViewModel.DrawOn);

            if (ViewModel.Vertical)
            {
                ViewModel.DrawOn.PhysicalRatioY *= ratio;

                PhysicalPoint pos2 =
                    new DipPoint(ViewModel.DrawOn.Config, ViewModel.DrawOn, p.X, p.Y).Mm.ToScreen(ViewModel.DrawOn);

                ViewModel.DrawOn.YLocationInMm += pos.Y - pos2.Y;
            }
            else
            {
                ViewModel.DrawOn.PhysicalRatioX *= ratio;

                PhysicalPoint pos2 =
                    new DipPoint(ViewModel.DrawOn.Config, ViewModel.DrawOn, p.X, p.Y).Mm.ToScreen(ViewModel.DrawOn);

                ViewModel.DrawOn.XLocationInMm += pos.X - pos2.X;
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
