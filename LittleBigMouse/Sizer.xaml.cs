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
using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;
using System.Windows.Media;
using System.Windows.Shapes;

namespace LittleBigMouse
{
    /// <summary>
    /// Logique d'interaction pour Sizer.xaml
    /// </summary>
    /// 

    public enum SizerSide
    {
        Top,
        Bottom,
        Left,
        Right
    }

    public partial class Sizer : Window
    {
        Screen _screen;
        Screen _drawOn;

        SizerSide _side;

        public static Sizer getSizer(Screen screen, SizerSide side)
        {
            Screen drawOn = null;
            switch (side)
            {
                case SizerSide.Left:
                    drawOn = screen.Config.FromPhysicalPoint(
                        new Point(
                            screen.PhysicalBounds.Left - 1.0, // TODO : +1 is just a ugly hack to be sure to be in side screen
                            screen.PhysicalBounds.Top + 1.0
                        )
                        );
                    if (drawOn == null)
                        drawOn = screen.Config.FromPhysicalPoint(
                            new Point(
                            screen.PhysicalBounds.Left - 1.0,
                            screen.PhysicalBounds.Bottom - 1.0
                            )
                            );
                break;
                case SizerSide.Right:
                    drawOn = screen.Config.FromPhysicalPoint(
                        new Point(
                            screen.PhysicalBounds.Right + 1.0, // TODO : +1 is just a ugly hack to be sure to be in side screen
                            screen.PhysicalBounds.Top + 1.0
                        )
                        );
                    if (drawOn == null)
                        drawOn = screen.Config.FromPhysicalPoint(
                            new Point(
                            screen.PhysicalBounds.Right + 1.0,
                            screen.PhysicalBounds.Bottom - 1.0
                            )
                            );
                    break;
                case SizerSide.Top:
                    drawOn = screen.Config.FromPhysicalPoint(
                        new Point(
                            screen.PhysicalBounds.Left + 1.0, // TODO : +1 is just a ugly hack to be sure to be in side screen
                            screen.PhysicalBounds.Top - 1.0
                        )
                        );
                    if (drawOn == null)
                        drawOn = screen.Config.FromPhysicalPoint(
                            new Point(
                            screen.PhysicalBounds.Right - 1.0,
                            screen.PhysicalBounds.Top - 1.0
                            )
                            );
                    break;
                case SizerSide.Bottom:
                    drawOn = screen.Config.FromPhysicalPoint(
                        new Point(
                            screen.PhysicalBounds.Left + 1.0, // TODO : +1 is just a ugly hack to be sure to be in side screen
                            screen.PhysicalBounds.Bottom + 1.0
                        )
                        );
                    if (drawOn == null)
                        drawOn = screen.Config.FromPhysicalPoint(
                            new Point(
                            screen.PhysicalBounds.Right - 1.0,
                            screen.PhysicalBounds.Bottom + 1.0
                            )
                            );
                    break;
            }

            if (drawOn != null) return new Sizer(screen, drawOn, side);
            else return null;
        }

        public Sizer(Screen screen, Screen drawOn, SizerSide side)
        {
            InitializeComponent();
            _screen = screen;
            _drawOn = drawOn;
            _side = side;

            switch(_side)
            {
                case SizerSide.Left:
                    Point left_top = _drawOn.PhysicalToWpf(_screen.PhysicalBounds.TopLeft);
                    Point left_bottom = _drawOn.PhysicalToWpf(_screen.PhysicalBounds.BottomLeft);

                    Width = (_drawOn.Bounds.Width*_drawOn.PixelToWpfRatioX) / 16;   

                    Left = left_top.X - Width;
                    Top = left_top.Y;
                    Height = left_bottom.Y - left_top.Y;

                    gradient.StartPoint = new Point(1,0.5);
                    gradient.EndPoint = new Point(0, 0.5);

                    border.BorderThickness = new Thickness(0, 1, 0, 1);
                    drawVerticalRuler(SizerSide.Right);
                    break;

                case SizerSide.Right:
                    Point right_top = _drawOn.PhysicalToWpf(_screen.PhysicalBounds.TopRight);
                    Point right_bottom = _drawOn.PhysicalToWpf(_screen.PhysicalBounds.BottomRight);

                    Width = (_drawOn.Bounds.Width*_drawOn.PixelToWpfRatioX) / 16;   

                    Left = right_top.X;
                    Top = right_top.Y;
                    Height = right_bottom.Y - right_top.Y;

                    gradient.StartPoint = new Point(0, 0.5);
                    gradient.EndPoint = new Point(1, 0.5);

                    border.BorderThickness = new Thickness(0, 1, 0, 1);
                    drawVerticalRuler(SizerSide.Left);
                    break;

                case SizerSide.Top:
                    Point top_left = _drawOn.PhysicalToWpf(_screen.PhysicalBounds.TopLeft);
                    Point top_right = _drawOn.PhysicalToWpf(_screen.PhysicalBounds.TopRight);

                    Height = (_drawOn.Bounds.Height * _drawOn.PixelToWpfRatioY) / 8;

                    Left = top_left.X;
                    Top = top_left.Y - Height;
                    Width = top_right.X - top_left.X;

                    gradient.StartPoint = new Point(0.5, 1);
                    gradient.EndPoint = new Point(0.5, 0);

                    border.BorderThickness = new Thickness(1, 0, 1, 0);

                    drawHorizontalRuler(SizerSide.Bottom);
                    break;


                case SizerSide.Bottom:
                    Point bottom_left = _drawOn.PhysicalToWpf(_screen.PhysicalBounds.BottomLeft);
                    Point bottom_right = _drawOn.PhysicalToWpf(_screen.PhysicalBounds.BottomRight);

                    Height = (_drawOn.Bounds.Height * _drawOn.PixelToWpfRatioY) / 8;

                    Left = bottom_left.X;
                    Top = bottom_left.Y;
                    Width = bottom_right.X - bottom_left.X;

                    gradient.StartPoint = new Point(0.5, 0);
                    gradient.EndPoint = new Point(0.5, 1);

                    border.BorderThickness = new Thickness(1, 0, 1, 0);

                    drawHorizontalRuler(SizerSide.Top);
                    break;
            }
        }

        private void drawVerticalRuler(SizerSide side)
        {
            double ratioX = _drawOn.PixelToWpfRatioX / _drawOn.PitchX;
            double ratioY = _drawOn.PixelToWpfRatioY / _drawOn.PitchY;
            int i = 0;
            while (true)
            {
                double y = i * ratioY;
                if (y > Height) break;

                double x; string text = null;
                if (i % 100 == 0)
                {
                    x = 20.0 * ratioX;
                    if (i > 0) text = (i / 100).ToString();
                }
                else if (i % 50 == 0)
                {
                    x = 15.0 * ratioX;
                    text = "5";
                }
                else if (i % 10 == 0)
                {
                    x = 10.0 * ratioX;
                    text = ((i % 100) / 10).ToString();
                }
                else if (i % 5 == 0)
                {
                    x = 5.0 * ratioX;
                }
                else
                {
                    x = 2.5 * ratioX;
                }

                if (text != null)
                {
                    TextBlock t = new TextBlock
                    {
                        Text = text,
                        FontSize = x / 3,
                    };
                    t.SetValue(Canvas.TopProperty, y);
                    t.SetValue(Canvas.LeftProperty, (side == SizerSide.Left)?(x - t.FontSize) : Width - x);
                    canvas.Children.Add(t);
                }

                Line l = new Line
                {
                    X1 = (side == SizerSide.Left) ? 0 : Width - x,
                    X2 = (side == SizerSide.Left) ? x : Width,
                    Y1 = y,
                    Y2 = y,
                    Stroke = new SolidColorBrush(Colors.Black),
                };
                canvas.Children.Add(l);
                i++;
            }

        }

        private void drawHorizontalRuler(SizerSide side)
        {
            double ratioX = _drawOn.PixelToWpfRatioX / _drawOn.PitchX;
            double ratioY = _drawOn.PixelToWpfRatioY / _drawOn.PitchY;

            int i = 0;
            while (true)
            {
                double x = i * ratioX;
                if (x > Width) break;

                double y; string text = null;
                if (i % 100 == 0)
                {
                    y = 20.0 * ratioY;
                    if (i > 0) text = (i / 100).ToString();
                }
                else if (i % 50 == 0)
                {
                    y = 15.0 * ratioY;
                    text = "5";
                }
                else if (i % 10 == 0)
                {
                    y = 10.0 * ratioY;
                    text = ((i % 100) / 10).ToString();
                }
                else if (i % 5 == 0)
                {
                    y = 5.0 * ratioY;
                }
                else
                {
                    y = 2.5 * ratioY;
                }

                if (text != null)
                {
                    TextBlock t = new TextBlock
                    {
                        Text = text,
                        FontSize = y / 3,
                    };
                    t.SetValue(Canvas.LeftProperty, x);
                    t.SetValue(Canvas.TopProperty, (side==SizerSide.Top)?(y - t.FontSize) : (Height - y));
                    canvas.Children.Add(t);
                }

                Line l = new Line
                {
                    X1 = x,
                    X2 = x,
                    Y1 = (side==SizerSide.Top)?0 :(Height - y),
                    Y2 = (side==SizerSide.Top)?y :Height,
                    Stroke = new SolidColorBrush(Colors.Black),
                };
                canvas.Children.Add(l);

                i++;
            }

        }


        private bool _moving = false;
        Point _oldPoint = new Point();


        private void Window_MouseMove(object sender, MouseEventArgs e)
        {
            Point newPoint = Mouse.CursorPos;

            if (e.LeftButton == MouseButtonState.Pressed)
            {
                if(_moving)
                {
                    Point p;
                    switch(_side)
                    {
                        case SizerSide.Left:
                        case SizerSide.Right:
                            Top += (newPoint.Y - _oldPoint.Y) * _drawOn.PixelToWpfRatioY;
                            p = _drawOn.WpfToPhysical(new Point(Left,Top));
                             _screen.PhysicalLocation = new Point(_screen.PhysicalBounds.X, p.Y);
                           break;
                        case SizerSide.Top:
                        case SizerSide.Bottom:
                            Left += (newPoint.X - _oldPoint.X) * _drawOn.PixelToWpfRatioY;
                            p = _drawOn.WpfToPhysical(new Point(Left,Top));
                            _screen.PhysicalLocation = new Point(p.X, _screen.PhysicalBounds.Y);
                            break;
                    }
                    _oldPoint = newPoint;
                }
                else
                {
                    _oldPoint = newPoint;
                    _moving = true;
                }
            }
            else
            {
                _moving = false;
            }
        }
    }
}
