/*
  HLab.Windows.MonitorVcp
  Copyright (c) 2021 Mathieu GRENET.  All right reserved.

  This file is part of HLab.Windows.MonitorVcp.

    HLab.Windows.MonitorVcp is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    HLab.Windows.MonitorVcp is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with MouseControl.  If not, see <http://www.gnu.org/licenses/>.

	  mailto:mathieu@mgth.fr
	  http://www.mgth.fr
*/


//using System.Drawing;

using System.Reactive.Linq;
using Avalonia;
using Avalonia.Controls;
using Avalonia.Data;
using HLab.Base.Avalonia;
using HLab.Notify.PropertyChanged;
using HLab.Sys.Windows.Monitors;


namespace HLab.Sys.Windows.MonitorVcp.Avalonia
{
using H = DependencyHelper<Luminance>;
    /// <summary>
    /// Logique d'interaction pour Luminance.xaml
    /// </summary>
    public partial class Luminance : UserControl
    {
        public Luminance()
        {
            InitializeComponent();

            DataContext = new LuminanceViewModel();

            // TODO : Avalonia

            //SetBinding(MonitorsProperty, new Binding("Config") { Mode = BindingMode.TwoWay });

            //var source = this.GetObservable(UserControl.DataContextProperty)
            //    ?.OfType<LuminanceViewModel>().Select(x => x?.Con);


            //this.Bind(MonitorsProperty, );


        }

        LuminanceViewModel ViewModel => DataContext as LuminanceViewModel;

        public static readonly AvaloniaProperty MonitorsProperty = H.Property<IMonitorsService>().Register();
            
            //AvaloniaProperty.Register("Config", typeof(IMonitorsService), typeof(Luminance), new PropertyMetadata(null));

        public IMonitorsService Monitors
        {
            get => (IMonitorsService)GetValue(MonitorsProperty); set
            {
                ViewModel.Monitors = value;
                SetValue(MonitorsProperty, value);
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

        void Control_OnMouseDoubleClick(object sender, MouseButtonEventArgs e)
        {
            ViewModel.Value = ViewModel.MaxAll;
        }

        void UIElement_OnMouseLeftButtonDown(object sender, MouseButtonEventArgs e)
        {
            if (e.ClickCount == 2) ViewModel.Value = ViewModel.MinAll;
        }
    }

}

