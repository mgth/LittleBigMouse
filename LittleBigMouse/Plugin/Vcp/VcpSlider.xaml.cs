/*
  LittleBigMouse.Plugin.Vcp
  Copyright (c) 2017 Mathieu GRENET.  All right reserved.

  This file is part of LittleBigMouse.Plugin.Vcp.

    LittleBigMouse.Plugin.Vcp is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    LittleBigMouse.Plugin.Vcp is distributed in the hope that it will be useful,
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
using System.Windows.Controls;
using System.Windows.Media;
using Hlab.Notify;
using Hlab.Windows.MonitorVcp;

namespace LittleBigMouse.Plugin.Vcp
{
    /// <summary>
    /// Logique d'interaction pour VcpSlider.xaml
    /// </summary>
    
    public partial class VcpSlider : UserControl, INotifyPropertyChanged
    {
        // PropertyChanged Handling
        public event PropertyChangedEventHandler PropertyChanged
        {
            add => this.Add(value);
            remove => this.Remove(value);
        }

        public VcpSlider()
        {
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
            get => (MonitorLevel)GetValue(MonitorLevelProperty); set => SetValue(MonitorLevelProperty, value);
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
                    case VcpComponent.Red:
                        return Colors.Red;
                    case VcpComponent.Green:
                        return Colors.Lime;
                    case VcpComponent.Blue:
                        return Colors.Blue;
                    case VcpComponent.Brightness:
                        return Colors.White;
                    case VcpComponent.Contrast:
                        return Colors.Gray;
                     default:
                        throw new ArgumentOutOfRangeException(nameof(Component), Component, null);
                }

            }
        }

        public VcpComponent Component { get; set; }

        private void Slider_OnValueChanged(object sender, RoutedPropertyChangedEventArgs<double> e)
        {
            if (MonitorLevel == null) return;

            MonitorLevel.ValueAsync = (uint)Slider.Value;
        }
    }
}
