using System;
using System.ComponentModel;
using System.Globalization;
using System.Windows;
using System.Windows.Data;

namespace Hlab.Mvvm
{
    public class ModelToViewModelConverter : IValueConverter
    {
        public object Convert(object value, Type targetType,
            object parameter, CultureInfo culture)
        {
            var fe = parameter as FrameworkElement;
            var p = (ViewModeContext)fe?.GetValue(ViewLocator.ViewModeContextProperty);
            var viewMode = (Type) fe?.GetValue(ViewLocator.ViewModeProperty);
            var viewClass = (Type)fe?.GetValue(ViewLocator.ViewClassProperty);

            return p?.GetLinked(value, viewMode, viewClass);
        }

        public object ConvertBack(object value, Type targetType,
            object parameter, CultureInfo culture)
        {
            var vm = value as INotifyPropertyChanged;
            return vm?.GetModel();
        }
    }
}
