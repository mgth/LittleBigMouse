using System;
using System.Globalization;
using System.Linq;
using System.Windows.Data;

namespace Hlab.Mvvm.Converters
{
    public class MultiScaleConverter : IMultiValueConverter
    {
        public object Convert(object[] values, Type targetType, object parameter, CultureInfo culture)
        {
            var v = values.OfType<double>().Min();

            var scale = double.Parse((string)parameter, CultureInfo.InvariantCulture);
            var result = v * scale;

            if (result < 0.1) return 0.1;
            if (result > 35791) return 35791;

            if (double.IsNaN(result) || double.IsInfinity(result)) return 0.1;

            return result;
        }

        public object[] ConvertBack(object value, Type[] targetTypes, object parameter, CultureInfo culture)
        {
            throw new NotImplementedException("MultiScaleConverter : ConvertBack");
        }
    }
}