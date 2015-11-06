using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Runtime.InteropServices;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;
using System.Windows.Interop;
using System.Windows.Media;
using System.Windows.Shapes;

namespace LittleBigMouse_Control
{
    class WindowResizer
    {
        private const int WM_SYSCOMMAND = 0x112;
        private HwndSource hwndSource;
        Window _activeWin;
        private Grid _grid;


        private Dictionary<UIElement, ResizeDirection> _sizers = new Dictionary<UIElement, ResizeDirection>();

        public WindowResizer(Window activeW, Grid grid)
        {
            _activeWin = activeW as Window;
            _grid = grid;

            _activeWin.SourceInitialized += new EventHandler(InitializeWindowSource);

            SetSizer(ResizeDirection.Top,0,1);
            SetSizer(ResizeDirection.Bottom, 2, 1);
            SetSizer(ResizeDirection.Left, 1, 0);
            SetSizer(ResizeDirection.Right, 1, 2);

            SetSizer(ResizeDirection.TopLeft, 0, 0);
            SetSizer(ResizeDirection.BottomLeft, 2, 0);
            SetSizer(ResizeDirection.TopRight, 0, 2);
            SetSizer(ResizeDirection.BottomRight, 2, 2);
        }

        void SetSizer(ResizeDirection dir, int row, int column)
        {
            Border sizer = new Border();

            sizer.Background = new SolidColorBrush(Colors.Black);

            sizer.SetValue(Grid.RowProperty, row);
            sizer.SetValue(Grid.ColumnProperty, column);
            _grid.Children.Add(sizer);

            sizer.MouseEnter += Sizer_MouseEnter;
            sizer.MouseLeave += Sizer_MouseLeave;
            sizer.PreviewMouseDown += Sizer_PreviewMouseDown;

            _sizers.Add(sizer, dir);
            
        }

        private void Sizer_PreviewMouseDown(object sender, MouseButtonEventArgs e)
        {
            UIElement clicked = sender as UIElement;

            if (clicked == null) return;

            ResizeDirection dir = _sizers[clicked];

            Sizer_MouseEnter(sender, e);

            ResizeWindow(dir);
        }
        private void Sizer_MouseEnter(object sender, MouseEventArgs e)
        {

            UIElement clicked = sender as UIElement;

            if (clicked == null) return;

            ResizeDirection dir = _sizers[clicked];

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
            hwndSource = PresentationSource.FromVisual((Visual)sender) as HwndSource;
            hwndSource.AddHook(new HwndSourceHook(WndProc));
        }

        IntPtr retInt = IntPtr.Zero;

        private IntPtr WndProc(IntPtr hwnd, int msg, IntPtr wParam, IntPtr lParam, ref bool handled)
        {
            //Debug.WriteLine("WndProc messages: " + msg.ToString());
            //
            // Check incoming window system messages
            //
            if (msg == WM_SYSCOMMAND)
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
        private static extern IntPtr SendMessage(IntPtr hWnd, uint Msg, IntPtr wParam, IntPtr lParam);


        private void ResizeWindow(ResizeDirection direction)
        {
            SendMessage(hwndSource.Handle, WM_SYSCOMMAND, (IntPtr)(61440 + direction), IntPtr.Zero);
        }





    }
}