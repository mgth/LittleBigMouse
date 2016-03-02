using System;
using System.Collections.Generic;
using System.Linq;
//using System.Drawing;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Data;
using System.Windows.Input;
using System.Windows.Media;
using NotifyChange;

namespace LbmScreenConfig
{
    /// <summary>
    /// Logique d'interaction pour Luminance.xaml
    /// </summary>
    public partial class Luminance : UserControl
    {
        public Luminance()
        {
            InitializeComponent();

            DataContext = new LuminanceViewModel();

            SetBinding(ConfigProperty, new Binding("Config") { Mode = BindingMode.TwoWay });
        }

        private LuminanceViewModel ViewModel => DataContext as LuminanceViewModel;

        public static readonly DependencyProperty ConfigProperty = DependencyProperty.Register("Config", typeof(ScreenConfig), typeof(Luminance), new PropertyMetadata(new ScreenConfig()));

        public ScreenConfig Config
        {
            get { return (ScreenConfig)GetValue(ConfigProperty); }
            set
            {
                ViewModel.Config = value;
                SetValue(ConfigProperty, value);
            }
        }

        public double WheelDelta(MouseWheelEventArgs e)
        {
            double delta = (e.Delta > 0) ? 1 : -1;
            if ((Keyboard.Modifiers & ModifierKeys.Control) != 0) delta /= 10;
            return delta;
        }

        public void OnMouseWheel(object sender, MouseWheelEventArgs e)
        {
            ViewModel.Value += WheelDelta(e);
        }

        private void Control_OnMouseDoubleClick(object sender, MouseButtonEventArgs e)
        {
            ViewModel.Value = ViewModel.MaxAll;
        }

        private void UIElement_OnMouseLeftButtonDown(object sender, MouseButtonEventArgs e)
        {
            if (e.ClickCount == 2) ViewModel.Value = ViewModel.MinAll;
        }
    }

}

