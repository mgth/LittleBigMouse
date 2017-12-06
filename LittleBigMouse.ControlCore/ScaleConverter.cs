using System;
using System.Globalization;
using System.Windows;
using System.Windows.Data;

namespace LittleBigMouse.ControlCore
{
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
}