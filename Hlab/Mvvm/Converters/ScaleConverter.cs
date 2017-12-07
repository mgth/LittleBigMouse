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
using System.Globalization;
using System.Windows;
using System.Windows.Data;

namespace Hlab.Mvvm.Converters
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