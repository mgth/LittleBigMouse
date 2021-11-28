using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Data;
using System.Windows.Documents;
using System.Windows.Input;
using System.Windows.Media;
using System.Windows.Media.Imaging;
using System.Windows.Navigation;
using System.Windows.Shapes;
using HLab.Base;
using HLab.Notify.PropertyChanged;

namespace LittleBigMouse.Plugin.Vcp
{
    /// <summary>
    /// Logique d'interaction pour ComponentSlider.xaml
    /// </summary>
    public partial class ComponentSlider : UserControl
    {
        private class H : DependencyHelper<ComponentSlider> { }

        public ComponentSlider()
        {
            InitializeComponent();
        }

        public static DependencyProperty ValueProperty = H.Property<double>()
            .OnChange((c,e) => c.Slider.Value = e.NewValue ).Register();

        public double Value {
            get => (double)GetValue(ValueProperty) ;
            set => SetValue(ValueProperty,value);
        }

        public static DependencyProperty MaximumProperty = H.Property<double>()
            .OnChange((c, e) => c.Slider.Maximum = e.NewValue).Register();

        public double Maximum {
            get => (double)GetValue(MaximumProperty) ;
            set => SetValue(MaximumProperty, value);
        }

        public static DependencyProperty MinimumProperty = H.Property<double>()
            .OnChange((c, e) => c.Slider.Minimum = e.NewValue)
            .Register();

        public double Minimum
        {
            get => (double)GetValue(MinimumProperty) ;
            set => SetValue(MinimumProperty, value);
        }

        private void Slider_OnValueChanged(object sender, RoutedPropertyChangedEventArgs<double> e)
        {
            Value = Slider.Value;
        }
    }
}
