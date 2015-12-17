using System;
using System.CodeDom;
using System.Collections.Generic;
using System.ComponentModel;
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
using System.Windows.Threading;
using LbmScreenConfig;
using NotifyChange;
using WinAPI_Dxva2;
using Component = LbmScreenConfig.Component;

namespace LittleBigMouse_Control.VcpPlugin
{
    /// <summary>
    /// Logique d'interaction pour VcpSlider.xaml
    /// </summary>
    
    public partial class VcpSlider : UserControl, INotifyPropertyChanged
    {
        // PropertyChanged Handling
        private readonly PropertyChangedHelper _change;
        public event PropertyChangedEventHandler PropertyChanged { add { _change.Add(this, value); } remove { _change.Remove(value); } }

        public VcpSlider()
        {
            _change = new PropertyChangedHelper(this);
            InitializeComponent();
            DataContext = this;
        }


        public static DependencyProperty MonitorLevelProperty = DependencyProperty.Register(
            "MonitorLevel",
            typeof(MonitorLevel),
            typeof(VcpSlider)
            );

        public MonitorLevel MonitorLevel
        {
            get { return (MonitorLevel)GetValue(MonitorLevelProperty); }
            set
            {
                SetValue(MonitorLevelProperty,value);
                value.PropertyChanged += ValueOnPropertyChanged;
            }
        }

        private void ValueOnPropertyChanged(object sender, PropertyChangedEventArgs propertyChangedEventArgs)
        {
            MonitorLevel level = sender as MonitorLevel;
            if (level == null) return;

            switch (propertyChangedEventArgs.PropertyName)
            {
                case "Min":
                    Dispatcher.Invoke(delegate { Slider.Minimum = level.Min; });
                    break;
                case "Max":
                    Dispatcher.Invoke( delegate { Slider.Maximum = level.Max; });
                    break;
//                case "Value":
                case "ValueAsync":
                    Dispatcher.Invoke(delegate
                    {
                        Slider.Value = level.ValueAsync;
                        TextBox.Text = level.ValueAsync.ToString();
                    });
                    break;
            }
            
        }

        public Color Color
        {
            get
            {
                switch (Component)
                {
                    case Component.Red:
                        return Colors.Red;
                    case Component.Green:
                        return Colors.Lime;
                    case Component.Blue:
                        return Colors.Blue;
                    case Component.Brightness:
                        return Colors.White;
                    case Component.Contrast:
                        return Colors.Gray;
                     default:
                        throw new ArgumentOutOfRangeException(nameof(Component), Component, null);
                }

            }
        }

        public Component Component { get; set; }

        private void Slider_OnValueChanged(object sender, RoutedPropertyChangedEventArgs<double> e)
        {
            MonitorLevel.Value = (uint)Slider.Value;
        }
    }
}
