/*
  HLab.Mvvm
  Copyright (c) 2017 Mathieu GRENET.  All right reserved.

  This file is part of HLab.Mvvm.

    HLab.Mvvm is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    HLab.Mvvm is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with MouseControl.  If not, see <http://www.gnu.org/licenses/>.

	  mailto:mathieu@mgth.fr
	  http://www.mgth.fr
*/
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