using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Media;
using System.Windows.Media.Animation;
using LbmScreenConfig;
using NotifyChange;

namespace LittleBigMouse_Control
{
    public class ScreenViewModel : ViewModel
    {


        private ViewModel _control = null;
        public ViewModel Control => _control??(_control=NewControl);
        public virtual ViewModel NewControl => null;

        public Screen Screen
        {
            get { return (Screen)GetValue(ScreenProperty); }
            set { SetValue(ScreenProperty, value); }
        }

        [DependsOn("Screen")]
        private void WatchConfig()
        {
            Watch(Screen.Config,"Config");
        }

        [DependsOn("Screen.RealPhysicalHeight", "Screen.RealTopBorder", "Screen.RealBottomBorder")]
        public double PhysicalOutsideHeight
        {
            get { return Screen.RealPhysicalHeight + Screen.RealTopBorder + Screen.RealBottomBorder; }
            set
            {
                double offset = value - PhysicalOutsideHeight;
                Screen.RealBottomBorder += offset;
                Screen.Config.Compact();
            }
        }

        [DependsOn("Screen.RealPhysicalWidth", "Screen.RealLeftBorder", "Screen.RealRightBorder")]
        public double PhysicalOutsideWidth
        {
            get { return Screen.RealPhysicalWidth + Screen.RealLeftBorder + Screen.RealRightBorder; }
            set
            {
                double offset = (value - PhysicalOutsideWidth) / 2;
                Screen.RealLeftBorder += offset;
                Screen.RealRightBorder += offset;
                Screen.Config.Compact();
            }
        }

        private bool _power = true;
        public Viewbox PowerButton => _power
            ? (Viewbox)Application.Current.FindResource("LogoPowerOn")
            : (Viewbox)Application.Current.FindResource("LogoPowerOff");




        protected const double MinFontSize = 0.1;

        private double _fontSize = MinFontSize;
        public static DependencyProperty ScreenProperty = DependencyProperty.Register(nameof(Screen), typeof(Screen), typeof(ScreenViewModel), WatchNotifier());

        public double FontSize
        {
            get { return _fontSize; }
            private set { SetProperty(ref _fontSize, value); }
        }

        public void OnSizeChanged(object sender, SizeChangedEventArgs sizeChangedEventArgs)
        {
            var obj = sender as UserControl;
            if (obj != null)
            {
                double s = Math.Min(obj.ActualHeight, obj.ActualWidth) / 7;
                FontSize = Math.Max(MinFontSize, Math.Pow(s, 1 / 1.3));              
            }
        }
    }
}
