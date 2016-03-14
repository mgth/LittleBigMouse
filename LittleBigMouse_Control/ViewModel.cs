using System;
using System.Windows;
using NotifyChange;

namespace LittleBigMouse_Control
{
    public class ViewModel : Notifier
    {
        //public static DependencyProperty ViewProperty = DependencyProperty.Register(nameof(View),typeof(FrameworkElement),typeof(ViewModel));
        //public FrameworkElement View { get { return (FrameworkElement)GetValue(ViewProperty); } set {SetValue(ViewProperty,value);} }

        private FrameworkElement _view = null;

        public FrameworkElement View
        {
            get
            {
                if (_view == null) _view = GetNewView();

                return _view; 
            }
            set { SetProperty(ref _view, value); }
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
