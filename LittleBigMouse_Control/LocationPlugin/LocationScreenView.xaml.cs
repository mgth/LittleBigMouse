using System;
using System.Collections.Generic;
using System.Collections.ObjectModel;
using System.ComponentModel;
using System.Diagnostics;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Data;
using System.Windows.Documents;
using System.Windows.Input;
using System.Windows.Media;
using System.Windows.Media.Imaging;
using System.Windows.Navigation;
using System.Windows.Shapes;
using LbmScreenConfig;
using LittleBigMouse_Control.LocationPlugin;
using NotifyChange;
using WinAPI_Dxva2;
using System.Globalization;
using LittleBigMouse_Control.Properties;

namespace LittleBigMouse_Control
{
    /// <summary>
    /// Logique d'interaction pour LocationScreenView.xaml
    /// </summary>
    partial class LocationScreenView : UserControl
    {

        public LocationScreenView() 
        {
            InitializeComponent();
            SizeChanged += OnSizeChanged1;
            LayoutUpdated += View_LayoutUpdated;
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
                Point p2 = PointToScreen(
                    new Point(
                        ActualWidth * _staticPoint.Value.X,
                        ActualHeight * _staticPoint.Value.Y
                        ));
                WinAPI_User32.User32.SetCursorPos((int)p2.X, (int)p2.Y);

                _staticPoint = null;
            }
        }

        private void OnSizeChanged1(object sender, SizeChangedEventArgs sizeChangedEventArgs)
        {
            ((ScreenViewModel)DataContext).RaiseProperty("Size");
        }

        private LocationScreenViewModel ViewModel => (DataContext as LocationScreenViewModel);

        /// <summary>
        /// 
        /// </summary>
        /// <param name="sender"></param>
        /// <param name="e"></param>
        private void OnSizeChanged(object sender, SizeChangedEventArgs e)
        {
            double size = Math.Min(grid.ActualHeight, grid.ActualWidth) / 3;
            center.Height = size;
            center.Width = size;
        }


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
    public class BoolToStringConverter : BoolToValueConverter<String> { }
    public class BoolToBrushConverter : BoolToValueConverter<Brush> { }
    public class BoolToVisibilityConverter : BoolToValueConverter<Visibility> { }
    public class BoolToObjectConverter : BoolToValueConverter<Object> { }
    public class BoolToValueConverter<T> : IValueConverter
    {
        public T FalseValue { get; set; }
        public T TrueValue { get; set; }

        public object Convert(object value, Type targetType, object parameter, System.Globalization.CultureInfo culture)
        {
            if (value == null)
                return FalseValue;
            else
                return (bool)value ? TrueValue : FalseValue;
        }

        public object ConvertBack(object value, Type targetType, object parameter, System.Globalization.CultureInfo culture)
        {
            return value?.Equals(TrueValue) ?? false;
        }
    }

    public class ScaleConverter : IValueConverter
    {
        public object Convert(object value, Type targetType, object parameter, CultureInfo culture)
        {
            double v = 12;
            if (value is double)
                v = (double)value;
            else if (value is FrameworkElement)
                v = Math.Min(((FrameworkElement) value).ActualHeight, ((FrameworkElement) value).ActualWidth);

            double scale = double.Parse((string)parameter,CultureInfo.InvariantCulture);

            return Math.Max(v * scale, 0.1);
        }

        public object ConvertBack(object value, Type targetType, object parameter, System.Globalization.CultureInfo culture)
        {
            throw new NotImplementedException();
        }

    }

}
