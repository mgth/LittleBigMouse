using System;
using System.ComponentModel;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Media;
using MonitorVcp;
using NotifyChange;
using Component = MonitorVcp.Component;

namespace LittleBigMouse_Control.Plugins.Vcp
{
    /// <summary>
    /// Logique d'interaction pour VcpSlider.xaml
    /// </summary>
    
    public partial class VcpSlider : UserControl, INotifyPropertyChanged
    {
        // PropertyChanged Handling
        private readonly NotifierHelper _change;
        public event PropertyChangedEventHandler PropertyChanged { add { _change.Add(value); } remove { _change.Remove(value); } }

        public VcpSlider()
        {
            _change = new NotifierHelper(this);
            InitializeComponent();
        }


        public static DependencyProperty MonitorLevelProperty = DependencyProperty.Register(
            "MonitorLevel",
            typeof(MonitorLevel),
            typeof(VcpSlider),
            new FrameworkPropertyMetadata(OnMonitorLevelProperty)
            );

        private static void OnMonitorLevelProperty(DependencyObject d, DependencyPropertyChangedEventArgs e)
        {
            var slider = (d as VcpSlider);
            if (slider == null) return;

            var old = (MonitorLevel) e.OldValue;
            if (old != null)
                old.PropertyChanged -= slider.ValueOnPropertyChanged;

            var newLevel = ((MonitorLevel)e.NewValue);
            if (newLevel!=null)
                newLevel.PropertyChanged += slider.ValueOnPropertyChanged;
        }

        public MonitorLevel MonitorLevel
        {
            get { return (MonitorLevel)GetValue(MonitorLevelProperty); }
            set { SetValue(MonitorLevelProperty,value); }
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
                case "Value":
                    Dispatcher.Invoke(delegate
                    {
                        Slider.Value = level.Value;
                        TextBox.Text = level.Value.ToString();
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
            if (MonitorLevel == null) return;

            MonitorLevel.ValueAsync = (uint)Slider.Value;
        }
    }
}
