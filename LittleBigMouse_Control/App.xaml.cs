using System;
using System.Globalization;
using System.Windows;
using System.Windows.Data;
using System.Windows.Media;

namespace LittleBigMouse_Control
{
    /// <summary>
    /// Logique d'interaction pour App.xaml
    /// </summary>
    public partial class App : Application
    {
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
                v = Math.Min(((FrameworkElement)value).ActualHeight, ((FrameworkElement)value).ActualWidth);

            double scale = double.Parse((string)parameter, CultureInfo.InvariantCulture);

            double result = Math.Min(Math.Max(v * scale, 0.1), 35791);

            if (double.IsNaN(result) || double.IsInfinity(result)) return 0.1;

            return result;
        }

        public object ConvertBack(object value, Type targetType, object parameter, System.Globalization.CultureInfo culture)
        {
            throw new NotImplementedException("ScaleConverter : ConvertBack");
        }

    }

    public class MultiScaleConverter : IMultiValueConverter
    {
        public object Convert(object[] values, Type targetType, object parameter, CultureInfo culture)
        {
            double v = double.MaxValue;
            foreach (object value in values)
            {
                if (value is double && (double)value < v) v = (double)value;
            }

            double scale = double.Parse((string)parameter, CultureInfo.InvariantCulture);

            double result = Math.Min(Math.Max(v * scale, 0.1), 35791);

            if (double.IsNaN(result) || double.IsInfinity(result)) return 0.1;

            return result;
        }

        public object[] ConvertBack(object value, Type[] targetTypes, object parameter, CultureInfo culture)
        {
            throw new NotImplementedException("MultiScaleConverter : ConvertBack");
        }
    }
}

