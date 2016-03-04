using System;
using System.Globalization;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Data;
using System.Windows.Input;
using System.Windows.Media;
using LittleBigMouse_Control.PluginLocation;
using Microsoft.Shell;

namespace LittleBigMouse_Control.Plugins.Location
{
    /// <summary>
    /// Logique d'interaction pour LocationScreenView.xaml
    /// </summary>
    partial class LocationScreenView : UserControl
    {

        public LocationScreenView() 
        {
            InitializeComponent();
            //LayoutUpdated += View_LayoutUpdated;
            SizeChanged += View_LayoutUpdated;
        }

        //        private ScreenConfig Config => MainViewModel.Instance.Config;
        //        private SizerPlugin.SizerPlugin Plugin => SizerPlugin.SizerPlugin.Instance;
        private Point? _staticPoint = null;

        private void SetStaticPoint()
        {
            Point p = Mouse.GetPosition(this);
            _staticPoint = new Point(
                p.X / ActualWidth,
                p.Y / ActualHeight);
        }
        private void View_LayoutUpdated(object sender, EventArgs e)
        {
            if (_staticPoint != null)
            {
                Point p = PointToScreen(
                    new Point(
                        ActualWidth * _staticPoint.Value.X,
                        ActualHeight * _staticPoint.Value.Y
                        ));

                WinAPI.NativeMethods.SetCursorPos((int)p.X, (int)p.Y);

                _staticPoint = null;
            }
        }

         private LocationScreenViewModel ViewModel => (DataContext as LocationScreenViewModel);


        private void PhysicalWidth_MouseWheel(object sender, MouseWheelEventArgs e)
        {
            if (!ViewModel.Screen.Selected) return;

            SetStaticPoint();
            double ratio = (e.Delta > 0) ? 1.005 : 1 / 1.005;
            ViewModel.Screen.PhysicalRatioX *= ratio;
            ViewModel.Screen.Config.Compact();
        }

        private void PhysicalHeight_MouseWheel(object sender, MouseWheelEventArgs e)
        {
            if (!ViewModel.Screen.Selected) return;

            SetStaticPoint();
            double ratio = (e.Delta > 0) ? 1.005 : 1 / 1.005;
            ViewModel.Screen.PhysicalRatioY *= ratio;
            ViewModel.Screen.Config.Compact();
        }

        private void OnMouseLeftButtonDown(object sender, MouseButtonEventArgs e)
        {
            CaptureMouse();
            ViewModel.StartMove(e.GetPosition(ViewModel.Frame.Presenter.ScreensCanvas));
            //Gui.BringToFront(); // Todo
            e.Handled = true;
        }
        private void OnMouseLeftButtonUp(object sender, MouseButtonEventArgs e)
        {
            ViewModel.EndMove();
            ReleaseMouseCapture();
        }

        private void OnMouseMove(object sender, MouseEventArgs e)
        {
            if (!IsMouseCaptured) return;

            if (ViewModel?.Frame.Presenter == null)
            {
                return;
            }

            if (e.LeftButton != MouseButtonState.Pressed)
            {
                ViewModel.EndMove();
                ReleaseMouseCapture();
                return;
            }

            ViewModel.Move(e.GetPosition(ViewModel.Frame.Presenter.ScreensCanvas));
        }


        private void OnKeyEnterUpdate(object sender, KeyEventArgs e)
        {
            if (e.Key == Key.Enter)
            {
                TextBox tBox = (TextBox)sender;
                DependencyProperty prop = TextBox.TextProperty;

                BindingExpression binding = BindingOperations.GetBindingExpression(tBox, prop);
                if (binding != null) { binding.UpdateSource(); }
            }
        }
    }

}
