using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using System.Windows;
using NotifyChange;

namespace LittleBigMouse_Control
{
    abstract class PresenterViewModel : ViewModel
    {
        public static DependencyProperty MainViewModelProperty = DependencyProperty.Register(nameof(MainViewModel), typeof(MainViewModel), typeof(MultiScreensViewModel));
        public MainViewModel MainViewModel
        {
            get { return (MainViewModel)GetValue(MainViewModelProperty); }
            set { SetValue(MainViewModelProperty, value); }
        }

        public static DependencyProperty GetScreenControlViewModelPropery = DependencyProperty.Register(nameof(GetScreenControlViewModel), typeof(GetScreenControlViewModelDelegate), typeof(MainViewModel));
        public GetScreenControlViewModelDelegate GetScreenControlViewModel
        {
            get { return (GetScreenControlViewModelDelegate)GetValue(GetScreenControlViewModelPropery); }
            set { SetValue(GetScreenControlViewModelPropery, value); }
        }

        public static DependencyProperty RatioProperty = DependencyProperty.Register(nameof(Ratio), typeof(double), typeof(MultiScreensViewModel));
        public double Ratio
        {
            get { return (double)GetValue(RatioProperty); }
            protected set { SetValue(RatioProperty, value); }
        }
        public abstract double PhysicalToUiX(double x);
        public abstract double PhysicalToUiY(double y);
    }
}
