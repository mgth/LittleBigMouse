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
using System.ComponentModel;
using System.Globalization;
using System.Windows;
using System.Windows.Data;

namespace HLab.Mvvm
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
