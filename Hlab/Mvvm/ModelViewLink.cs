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