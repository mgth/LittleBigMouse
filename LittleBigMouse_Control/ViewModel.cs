using System;
using System.ComponentModel;
using System.Windows;
using Erp.Notify;

namespace LittleBigMouse_Control
{
    public class ViewModel : INotifyPropertyChanged
    {
        //public static DependencyProperty ViewProperty = DependencyProperty.Register(nameof(View),typeof(FrameworkElement),typeof(ViewModel));
        //public FrameworkElement View { get { return (FrameworkElement)GetValue(ViewProperty); } set {SetValue(ViewProperty,value);} }
        public event PropertyChangedEventHandler PropertyChanged
        {
            add => this.Add(value);
            remove => this.Remove(value);
        }

        public FrameworkElement View
        {
            get => this.Get(GetNewView); set => this.Set(value);
        }

        public virtual Type ViewType => null;

        public virtual FrameworkElement GetNewView()
        {
                 if (ViewType == null) return null;

                var fe = (FrameworkElement)Activator.CreateInstance(ViewType);
                fe.DataContext = this;
                return fe;
        }
    }
}
