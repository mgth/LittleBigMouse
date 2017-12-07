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
