/*
  LittleBigMouse.Daemon
  Copyright (c) 2017 Mathieu GRENET.  All right reserved.

  This file is part of LittleBigMouse.Daemon.

    LittleBigMouse.Daemon is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    LittleBigMouse.Daemon is distributed in the hope that it will be useful,
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
using System.Runtime.CompilerServices;
using System.Windows;
using System.Windows.Input;
using System.Windows.Media;
using HLab.Base.Wpf;

namespace BrightnessSync
{
    /// <summary>
    /// Logique d'interaction pour Luminance.xaml
    /// </summary>
    public partial class BrightnessWindow : Window, INotifyPropertyChanged
    {
        public static RoutedEvent AnimShowEvent = EventManager.RegisterRoutedEvent("AnimShow",RoutingStrategy.Bubble, typeof(RoutedEventHandler), typeof(BrightnessWindow));
        public static RoutedEvent AnimHideEvent = EventManager.RegisterRoutedEvent("AnimHide", RoutingStrategy.Bubble, typeof(RoutedEventHandler), typeof(BrightnessWindow));
        public event RoutedEventHandler AnimShow
        {
            add => AddHandler(AnimShowEvent, value);
            remove => RemoveHandler(AnimShowEvent, value);
        }
        public event RoutedEventHandler AnimHide
        {
            add => AddHandler(AnimHideEvent, value);
            remove => RemoveHandler(AnimHideEvent, value);
        }

        public static double AutoHeight => 50;
        public static double AutoWidth => 360;
        public double AutoLeft => 
            SystemParameters.WorkArea.Width - AutoWidth;
        public double AutoTop => 
            SystemParameters.WorkArea.Height - AutoHeight;
        public double AutoBottom =>
            SystemParameters.WorkArea.Height;

        public Brush BackgroundBrush => new SolidColorBrush(AccentColorSet.ActiveSet["HardwareFlipViewFillPressed"]); //);
        public Brush BorderBrushColor => new SolidColorBrush(AccentColorSet.ActiveSet["ControlScrollbarThumbBorderPressed"]);//; //);

        public BrightnessWindow()
        {
            
            InitializeComponent();
            DataContext = this;

            Left = AutoLeft;
            Top = AutoTop;
            Loaded += (sender, args) => this.EnableBlur();
            Loaded += OnLoaded;
            SizeChanged += (sender, args) => OnPropertyChanged(nameof(AutoTop));
            SizeChanged += (sender, args) => OnPropertyChanged(nameof(AutoLeft));

        }

        public MouseDevice Dev { get; set; } = null;

        private void HookOnMouseWheel(object sender, MouseEventArgs mouseEventArgs)
        {
            //TODO :
            //MouseWheelEventArgs e = new MouseWheelEventArgs(
            //    Mouse.PrimaryDevice, 0,
            //    mouseEventArgs.Delta);
            //Luminance.OnMouseWheel(this, e);
        }

        private void OnLoaded(object sender, RoutedEventArgs routedEventArgs)
        {
            //CaptureMouse();
            //this.Focus();
        }

        public event PropertyChangedEventHandler PropertyChanged;

        protected virtual void OnPropertyChanged([CallerMemberName] string propertyName = null)
        {
            PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(propertyName));
        }

        protected override void OnMouseWheel(MouseWheelEventArgs e)
        {
            //TODO Luminance.OnMouseWheel(this,e);
            base.OnMouseWheel(e);
        }

        protected override void OnDeactivated(EventArgs e)
        {
            Hide();
            base.OnDeactivated(e);
        }

        private void LuminanceWindow_OnStateChanged(object sender, EventArgs e)
        {
        }


        // TODO public IKeyboardMouseEvents Hook = null;

        private void LuminanceWindow_OnIsVisibleChanged(object sender, DependencyPropertyChangedEventArgs e)
        {
            if (Visibility == Visibility.Visible)
            {
                //TODO : Hook.MouseWheel += HookOnMouseWheel;
                RaiseEvent(new RoutedEventArgs(AnimShowEvent));
            }
            else
            {
                //TODO : Hook.MouseWheel -= HookOnMouseWheel;
                //this.Focus();
                RaiseEvent(new RoutedEventArgs(AnimHideEvent));
            }
        }
    }
}
