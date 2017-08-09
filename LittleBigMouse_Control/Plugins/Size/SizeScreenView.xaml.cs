using System;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;

namespace LittleBigMouse_Control.Plugins.Size
{
    /// <summary>
    /// Logique d'interaction pour ScreenGuiBorders.xaml
    /// </summary>
    public partial class SizeScreenView : UserControl
    {
        public SizeScreenView()
        {
            InitializeComponent();

            SizeChanged += SizeScreenView_SizeChanged;
            LayoutUpdated += SizeScreenView_LayoutUpdated;
        }

        private void SizeScreenView_LayoutUpdated(object sender, EventArgs e)
        {
            if (_staticPoint != null)
            {
                 Point p2 = PointToScreen(
                     new Point(
                         ActualWidth * _staticPoint.Value.X, 
                         ActualHeight * _staticPoint.Value.Y
                         ));
                    WinAPI.NativeMethods.SetCursorPos((int)p2.X, (int)p2.Y);

                _staticPoint = null;
            }
        }

        private void SizeScreenView_SizeChanged(object sender, SizeChangedEventArgs e)
        {
        }

        SizeViewModel ViewModel => (DataContext as SizeViewModel);


        private void Height_MouseWheel(object sender, MouseWheelEventArgs e)
        {
            SetStaticPoint();
            ViewModel.PhysicalOutsideHeight += WheelDelta(e);
        }
        private void Width_MouseWheel(object sender, MouseWheelEventArgs e)
        {
            SetStaticPoint();
            ViewModel.PhysicalOutsideWidth += WheelDelta(e);
        }

        private void Bottom_MouseWheel(object sender, MouseWheelEventArgs e)
        {
            SetStaticPoint();
            ViewModel.Screen.BottomBorderInMm += WheelDelta(e);
        }

        private void Right_MouseWheel(object sender, MouseWheelEventArgs e)
        {
            SetStaticPoint();
            ViewModel.Screen.RightBorderInMm += WheelDelta(e);
        }

        private void Left_MouseWheel(object sender, MouseWheelEventArgs e)
        {
            SetStaticPoint();
            ViewModel.Screen.LeftBorderInMm += WheelDelta(e);
        }

        private void Top_MouseWheel(object sender, MouseWheelEventArgs e)
        {
            SetStaticPoint();
            ViewModel.Screen.TopBorderInMm += WheelDelta(e);
        }

        private void OnKeyEnterUpdate(object sender, KeyEventArgs e)
        {
            ViewHelper.OnKeyEnterUpdate(sender, e);
        }

        public double WheelDelta(MouseWheelEventArgs e)
        {
            double delta = (e.Delta > 0) ? 1 : -1;
            if ((Keyboard.Modifiers & ModifierKeys.Control) != 0) delta /= 10;
            return delta;
        }

        private Point? _staticPoint = null;

        private void SetStaticPoint()
        {
            Point p = Mouse.GetPosition(this);
            _staticPoint = new Point(
                p.X/ActualWidth,
                p.Y/ActualHeight);
        }

        private void InsideHeight_MouseWheel(object sender, MouseWheelEventArgs e)
        {
            SetStaticPoint();
            ViewModel.Screen.RealPhysicalHeight += WheelDelta(e);
        }

        private void InsideWidth_MouseWheel(object sender, MouseWheelEventArgs e)
        {
            SetStaticPoint();
            ViewModel.Screen.RealPhysicalWidth += WheelDelta(e);
        }
    }
}
