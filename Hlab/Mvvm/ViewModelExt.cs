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
using System.ComponentModel;
using System.Linq;
using System.Windows;
using Hlab.Base;

namespace Hlab.Mvvm
{
    public static class ViewModelCollectionExt
    {
        public static FrameworkElement GetActualView(this INotifyPropertyChanged vm, FrameworkElement element = null)
        {
            if (element == null) element = Application.Current.MainWindow;

            return element.FindChildren<FrameworkElement>().FirstOrDefault(fe => ReferenceEquals(fe.DataContext, vm));
        }
    }
}
