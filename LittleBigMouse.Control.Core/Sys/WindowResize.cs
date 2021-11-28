/*
  LittleBigMouse.Control.Core
  Copyright (c) 2017 Mathieu GRENET.  All right reserved.

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
using System.Runtime.InteropServices;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;
using System.Windows.Interop;
using System.Windows.Media;

namespace LittleBigMouse.Control.Sys
{
    internal class WindowResizer
    {
        private const int WmSyscommand = 0x112;
        private HwndSource _hwndSource;
        readonly Window _activeWin;
        private readonly Grid _grid;


        private readonly Dictionary<UIElement, ResizeDirection> _resizers = new Dictionary<UIElement, ResizeDirection>();

        public WindowResizer(Window activeW, Grid grid)
        {
            _activeWin = activeW;
            _grid = grid;

            _activeWin.SourceInitialized += InitializeWindowSource;

            SetSizer(ResizeDirection.Top,0,1);
            SetSizer(ResizeDirection.Bottom, 2, 1);
            SetSizer(ResizeDirection.Left, 1, 0);
            SetSizer(ResizeDirection.Right, 1, 2);

            SetSizer(ResizeDirection.TopLeft, 0, 0);
            SetSizer(ResizeDirection.BottomLeft, 2, 0);
            SetSizer(ResizeDirection.TopRight, 0, 2);
            SetSizer(ResizeDirection.BottomRight, 2, 2);
        }

        private void SetSizer(ResizeDirection dir, int row, int column)
        {
            var sizer = new Border
            {
                Background = new SolidColorBrush(Colors.Black),
                Margin = new Thickness(-1)
            };

            sizer.SetValue(Grid.RowProperty, row);
            sizer.SetValue(Grid.ColumnProperty, column);
            _grid.Children.Add(sizer);

            sizer.MouseEnter += Sizer_MouseEnter;
            sizer.MouseLeave += Sizer_MouseLeave;
            sizer.PreviewMouseDown += Sizer_PreviewMouseDown;

            _resizers.Add(sizer, dir);           
        }

        private void Sizer_PreviewMouseDown(object sender, MouseButtonEventArgs e)
        {
            if (!(sender is UIElement clicked)) return;

            var dir = _resizers[clicked];

            Sizer_MouseEnter(sender, e);

            ResizeWindow(dir);
        }
        private void Sizer_MouseEnter(object sender, MouseEventArgs e)
        {
            if (!(sender is UIElement clicked)) return;

            var dir = _resizers[clicked];

            {
                switch (dir)
                {
                    case ResizeDirection.Top:
                    case ResizeDirection.Bottom:
                        _activeWin.Cursor = Cursors.SizeNS;
                        break;
                    case ResizeDirection.Left:
                    case ResizeDirection.Right:
                        _activeWin.Cursor = Cursors.SizeWE;
                        break;
                    case ResizeDirection.TopLeft:
                    case ResizeDirection.BottomRight:
                        _activeWin.Cursor = Cursors.SizeNWSE;
                        break;
                    case ResizeDirection.BottomLeft:
                    case ResizeDirection.TopRight:
                        _activeWin.Cursor = Cursors.SizeNESW;
                        break;
                }

            }
        }
        private void Sizer_MouseLeave(object sender, MouseEventArgs mouseEventArgs)
        {
            if (mouseEventArgs.LeftButton != MouseButtonState.Pressed)
            {
                _activeWin.Cursor = Cursors.Arrow;
            }
        }

        public void Sizer_DragWindow(object sender, MouseEventArgs mouseEventArgs)
        {
            if (mouseEventArgs.LeftButton == MouseButtonState.Released) return;
            _activeWin.DragMove();
        }

        private void InitializeWindowSource(object sender, EventArgs e)
        {
            _hwndSource = PresentationSource.FromVisual((Visual)sender) as HwndSource;
            _hwndSource?.AddHook(WndProc);
        }

        private IntPtr _retInt = IntPtr.Zero;

        private IntPtr WndProc(IntPtr hwnd, int msg, IntPtr wParam, IntPtr lParam, ref bool handled)
        {
            //Debug.WriteLine("WndProc messages: " + msg.ToString());
            //
            // Check incoming window system messages
            //
            if (msg == WmSyscommand)
            {
                //Debug.WriteLine("WndProc messages: " + msg.ToString());
            }

            return IntPtr.Zero;
        }

        public enum ResizeDirection
        {
            Left = 1,
            Right = 2,
            Top = 3,
            TopLeft = 4,
            TopRight = 5,
            Bottom = 6,
            BottomLeft = 7,
            BottomRight = 8,
        }

        [DllImport("user32.dll", CharSet = CharSet.Auto)]
        private static extern IntPtr SendMessage(IntPtr hWnd, uint msg, IntPtr wParam, IntPtr lParam);

        private void ResizeWindow(ResizeDirection direction)
        {
            SendMessage(_hwndSource.Handle, WmSyscommand, (IntPtr)(61440 + direction), IntPtr.Zero);
        }
    }
}