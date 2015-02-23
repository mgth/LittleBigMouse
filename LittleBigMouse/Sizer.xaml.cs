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
using System.Windows.Input;

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

                    Width = (_drawOn.Bounds.Width*_drawOn.WpfRatioX) / 16;   

                    Left = left_top.X - Width;
                    Top = left_top.Y;
                    Height = left_bottom.Y - left_top.Y;

                    gradient.StartPoint = new Point(1,0.5);
                    gradient.EndPoint = new Point(0, 0.5);

                    border.BorderThickness = new Thickness(0, 1, 0, 1);

                    break;

                case SizerSide.Right:
                    Point right_top = _drawOn.PhysicalToWpf(_screen.PhysicalBounds.TopRight);
                    Point right_bottom = _drawOn.PhysicalToWpf(_screen.PhysicalBounds.BottomRight);

                    Width = (_drawOn.Bounds.Width*_drawOn.WpfRatioX) / 16;   

                    Left = right_top.X;
                    Top = right_top.Y;
                    Height = right_bottom.Y - right_top.Y;

                    gradient.StartPoint = new Point(0, 0.5);
                    gradient.EndPoint = new Point(1, 0.5);

                    border.BorderThickness = new Thickness(0, 1, 0, 1);

                    break;

                case SizerSide.Top:
                    Point top_left = _drawOn.PhysicalToWpf(_screen.PhysicalBounds.TopLeft);
                    Point top_right = _drawOn.PhysicalToWpf(_screen.PhysicalBounds.TopRight);

                    Height = (_drawOn.Bounds.Height * _drawOn.WpfRatioY) / 8;

                    Left = top_left.X;
                    Top = top_left.Y - Height;
                    Width = top_right.X - top_left.X;

                    gradient.StartPoint = new Point(0.5, 1);
                    gradient.EndPoint = new Point(0.5, 0);

                    border.BorderThickness = new Thickness(1, 0, 1, 0);

                    break;


                case SizerSide.Bottom:
                    Point bottom_left = _drawOn.PhysicalToWpf(_screen.PhysicalBounds.BottomLeft);
                    Point bottom_right = _drawOn.PhysicalToWpf(_screen.PhysicalBounds.BottomRight);

                    Height = (_drawOn.Bounds.Height * _drawOn.WpfRatioY) / 8;

                    Left = bottom_left.X;
                    Top = bottom_left.Y;
                    Width = bottom_right.X - bottom_left.X;

                    gradient.StartPoint = new Point(0.5, 0);
                    gradient.EndPoint = new Point(0.5, 1);

                    border.BorderThickness = new Thickness(1, 0, 1, 0);

                    break;
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
                            Top += (newPoint.Y - _oldPoint.Y) * _drawOn.WpfRatioY;
                            p = _drawOn.WpfToPhysical(new Point(Left,Top));
                             _screen.PhysicalLocation = new Point(_screen.PhysicalBounds.X, p.Y);
                           break;
                        case SizerSide.Top:
                        case SizerSide.Bottom:
                            Left += (newPoint.X - _oldPoint.X) * _drawOn.WpfRatioY;
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
