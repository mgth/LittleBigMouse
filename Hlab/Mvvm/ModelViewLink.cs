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
using System.Windows;

namespace Hlab.Mvvm
{
    public class MvvmLink
    {
        public string ViewMode { get; set; }
        /// <summary>
        /// Can be 
        /// - Model to link ViewModel 
        /// - ViewModel to link View / ViewModel
        /// </summary>
        public Type BaseType { get; set; }

        /// <summary>
        /// Can be View linked to ViewModel or ViewModel linked to Model/ViewModel
        /// </summary>
        public Type DerivedType { get; set; }

        public FrameworkElement GetView(INotifyPropertyChanged viewModel)
        {
            if (!typeof(FrameworkElement).IsAssignableFrom(DerivedType)) return null;

            var view = Activator.CreateInstance(DerivedType) as FrameworkElement;
            if(view!=null) view.DataContext = viewModel;
            return view;
        }
    }


    public class ModelViewLink
    {
        public bool IsList { get; set; }
        public string ViewMode { get; set; }
        public Type ViewType { get; set; }
        public Type ViewModelType { get; set; }
        public Type ModelType { get; set; }

        public FrameworkElement GetView()
        => (ViewType==null)
            ? new NotFoundView():
            Activator.CreateInstance(ViewType) as FrameworkElement;
        

        public INotifyPropertyChanged GetViewModel() 
            => Activator.CreateInstance(ViewModelType) as INotifyPropertyChanged;
    }
}