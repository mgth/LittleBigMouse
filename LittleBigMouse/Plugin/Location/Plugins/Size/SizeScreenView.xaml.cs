using System;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;
using Hlab.Mvvm;
using LittleBigMouse.Control.Core;

namespace LittleBigMouse.LocationPlugin.Plugins.Size
{
    class SizeScreenContentView : UserControl, IView<ViewModeScreenSize, ScreenSizeViewModel>, IViewScreenFrameTopLayer 
    {
    }

    /// <summary>
    /// Logique d'interaction pour ScreenGuiBorders.xaml
    /// </summary>
    public partial class SizeScreenView : UserControl , IView<ViewModeScreenSize, ScreenSizeViewModel>, IViewScreenFrameContent
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

        ScreenSizeViewModel ViewModel => (DataContext as ScreenSizeViewModel);


        private void Height_MouseWheel(object sender, MouseWheelEventArgs e)
        {
            SetStaticPoint();
            ViewModel.OutsideHeight += WheelDelta(e);
        }
        private void Width_MouseWheel(object sender, MouseWheelEventArgs e)
        {
            SetStaticPoint();
            ViewModel.OutsideWidth += WheelDelta(e);
        }

        private void Bottom_MouseWheel(object sender, MouseWheelEventArgs e)
        {
            SetStaticPoint();
            ViewModel.Model.InMm.BottomBorder += WheelDelta(e);
        }

        private void Right_MouseWheel(object sender, MouseWheelEventArgs e)
        {
            SetStaticPoint();
            ViewModel.Model.InMm.RightBorder += WheelDelta(e);
        }

        private void Left_MouseWheel(object sender, MouseWheelEventArgs e)
        {
            SetStaticPoint();
            ViewModel.Model.InMm.LeftBorder += WheelDelta(e);
        }

        private void Top_MouseWheel(object sender, MouseWheelEventArgs e)
        {
            SetStaticPoint();
            ViewModel.Model.InMm.TopBorder += WheelDelta(e);
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
            ViewModel.Height += WheelDelta(e);
        }

        private void InsideWidth_MouseWheel(object sender, MouseWheelEventArgs e)
        {
            SetStaticPoint();
            ViewModel.Width += WheelDelta(e);
        }
    }
}
