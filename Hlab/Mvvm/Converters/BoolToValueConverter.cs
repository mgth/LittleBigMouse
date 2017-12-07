using System;
using System.Windows;
using System.Windows.Data;
using System.Windows.Media;

namespace Hlab.Mvvm.Converters
{
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
    public class BoolToStringConverter : BoolToValueConverter<String> { }
    public class BoolToBrushConverter : BoolToValueConverter<Brush> { }
    public class BoolToVisibilityConverter : BoolToValueConverter<Visibility> { }
    public class BoolToObjectConverter : BoolToValueConverter<Object> { }

    public class LengthRatioConverter : DependencyObject, IValueConverter
    {
        public static readonly DependencyProperty PhysicalRectProperty =
            DependencyProperty.Register(nameof(PhysicalRect), typeof(Rect),
                typeof(LengthRatioConverter), new FrameworkPropertyMetadata(new Rect()));

        public static readonly DependencyProperty FrameworkElementProperty =
            DependencyProperty.Register(nameof(FrameworkElement), typeof(FrameworkElement),
                typeof(LengthRatioConverter), new FrameworkPropertyMetadata(null));

        public Rect PhysicalRect //{ get; set; }
        {
            get => (Rect)GetValue(PhysicalRectProperty);
            set => SetValue(PhysicalRectProperty, value);
        }

        public FrameworkElement FrameworkElement //{ get; set; }
        {
            get => (FrameworkElement)GetValue(FrameworkElementProperty);
            set => SetValue(FrameworkElementProperty, value);
        }

        private double Ratio => Math.Min(
            FrameworkElement.ActualWidth / PhysicalRect.Width,
            FrameworkElement.ActualHeight / PhysicalRect.Height
        );

        public object Convert(object value, Type targetType, object parameter, System.Globalization.CultureInfo culture)
        {
            return new GridLength(Ratio * (double) value);
        }

        public object ConvertBack(object value, Type targetType, object parameter, System.Globalization.CultureInfo culture)
        {
            return ((GridLength)value).Value / Ratio;
        }

        //public override object ProvideValue(IServiceProvider serviceProvider)
        //{
        //    return this;
        //}
    }
    public class MarginRatioConverter : IValueConverter
    {
        public Rect PhysicalRect { get; set; }
        public FrameworkElement FrameworkElement { get; set; }

        private double Ratio => Math.Min(
            FrameworkElement.ActualWidth / PhysicalRect.Width,
            FrameworkElement.ActualHeight / PhysicalRect.Height
        );
            public double PhysicalToUiX(double x)
                => (x - PhysicalRect.Left) * Ratio
                   + (FrameworkElement.ActualWidth
                      - PhysicalRect.Width * Ratio) / 2;
            public double PhysicalToUiY(double y)
                => (y - PhysicalRect.Top) * Ratio
                   + (FrameworkElement.ActualHeight
                      - PhysicalRect.Height * Ratio) / 2;

        public object Convert(object value, Type targetType, object parameter, System.Globalization.CultureInfo culture)
        {
            var rect = (Rect) value;

            return new Thickness(PhysicalToUiX(rect.X), PhysicalToUiY(rect.Y),0,0);
        }

        public object ConvertBack(object value, Type targetType, object parameter, System.Globalization.CultureInfo culture)
        {
            return ((GridLength)value).Value / Ratio;
        }
    }

}