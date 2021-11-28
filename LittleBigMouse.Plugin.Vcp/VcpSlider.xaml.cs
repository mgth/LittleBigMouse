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
using HLab.Base;
using HLab.Sys.Windows.MonitorVcp;

namespace LittleBigMouse.Plugin.Vcp
{
    /// <summary>
    /// Logique d'interaction pour VcpSlider.xaml
    /// </summary>

    public partial class VcpSlider : UserControl
    {
        class H : DependencyHelper<VcpSlider> { }

        public VcpSlider()
        {
            InitializeComponent();

            SetEnabled(false);
        }


        public static DependencyProperty MonitorLevelProperty = H.Property<MonitorLevel>()
            .OnChange((c, e) =>
            {
                if(e.OldValue!=null)
                    e.OldValue.PropertyChanged -= c.ValueOnPropertyChanged;

                if (e.NewValue != null)
                {
                    e.NewValue.PropertyChanged += c.ValueOnPropertyChanged;

                    c.SetMinimum(e.NewValue.Min);
                    c.SetMaximum(e.NewValue.Max);
                    c.SetValue(e.NewValue.Value);
                    c.SetEnabled(e.NewValue.Enabled);
                    c.SetMoving(e.NewValue.Moving);
                }
            }).Register();


        public MonitorLevel MonitorLevel
        {
            get => (MonitorLevel)GetValue(MonitorLevelProperty);
            set => SetValue(MonitorLevelProperty, value);
        }

        private void ValueOnPropertyChanged(object sender, PropertyChangedEventArgs propertyChangedEventArgs)
        {
            MonitorLevel level = sender as MonitorLevel;
            if (level == null) return;

            switch (propertyChangedEventArgs.PropertyName)
            {
                case "Min":
                    SetMinimum(level.Min);
                    break;
                case "Max":
                    SetMaximum(level.Max);
                    break;
                case "Value":
                    SetValue(level.Value);
                    break;
                case "Enabled":
                    SetEnabled(level.Enabled);
                    break;
                case "Moving":
                     SetMoving(level.Moving);
                   break;
            }
        }

        private void SetMaximum(double max) => Dispatcher.Invoke(()=>Slider.Maximum = max);
        private void SetMinimum(double min) => Dispatcher.Invoke(()=>Slider.Minimum = min);
        private void SetValue(double value) => Dispatcher.Invoke(()=>
        {
            Slider.Value = value;
            TextBox.Text = value.ToString();
        });
        private void SetMoving(bool moving) => Dispatcher.Invoke(()=>
        {
            TextBox.Background = new SolidColorBrush(moving?Colors.Orange:Colors.LightGreen);
        });
        private void SetEnabled(bool enabled) => Dispatcher.Invoke(()=>
        {
            Slider.IsEnabled = enabled;
            TextBox.IsEnabled = enabled;
        });
        //        private void SetValue(MonitorLevel level) => Dispatcher.Invoke(()=>Slider.Value = level.Value);

        public static DependencyProperty ComponentProperty = H.Property<VcpComponent>()
            .Default(VcpComponent.Brightness)
            .OnChange((c, e) =>
            {
                switch (e.NewValue)
                {
                    case VcpComponent.Red:
                        c.SetColor( Color.FromArgb(255,255,0,0) );
                        break;
                    case VcpComponent.Green:
                        c.SetColor(Color.FromArgb(255, 0, 255, 0));
                        break;
                    case VcpComponent.Blue:
                        c.SetColor(Color.FromArgb(255,0, 0, 255));
                        break;
                    case VcpComponent.Brightness:
                        c.SetColor(Color.FromArgb(255, 255, 255, 255));
                        break;
                    case VcpComponent.Contrast:
                        c.SetColor(Color.FromArgb(255, 30, 30, 30));
                        break;
                    default:
                        throw new ArgumentOutOfRangeException(nameof(Component), e.NewValue, null);
                }

            })
            .Register();

        private void SetColor(Color c)
        {
            Slider.Foreground = new SolidColorBrush(
                Color.FromScRgb(c.ScA * 0.7f, c.ScR* 0.8f , c.ScG* 0.8f, c.ScB * 0.8f)
            );
            Slider.Background = new SolidColorBrush(
                Color.FromScRgb(c.ScA * 0.6f, c.ScR* 0.2f , c.ScG* 0.2f, c.ScB * 0.2f)
            );
            Slider.BorderBrush = new SolidColorBrush(
                Color.FromScRgb(c.ScA * 0.9f, c.ScR * 0.5f+0.5f, c.ScG * 0.5f + 0.5f, c.ScB * 0.5f + 0.5f)
            );

        }

        public VcpComponent Component
        {
            get => (VcpComponent)GetValue(ComponentProperty);
            set => SetValue(ComponentProperty,value);
        }

        private void Slider_OnValueChanged(object sender, RoutedPropertyChangedEventArgs<double> e)
        {
            if (MonitorLevel == null) return;

            MonitorLevel.Value = (uint)Slider.Value;
        }
    }
}
