using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Media;
using NotifyChange;

namespace LittleBigMouse_Control
{
    class ScreenControlViewModel : ScreenViewModel
    {
        public static DependencyProperty FrameProperty = DependencyProperty.Register
            (nameof(Frame), typeof(ScreenFrameViewModel), typeof(ScreenControlViewModel), WatchNotifier());

        public ScreenFrameViewModel Frame
        {
            get { return (ScreenFrameViewModel)GetValue(FrameProperty); }
            set { SetValue(FrameProperty, value); }
        }

        public Grid CoverControl { get; } = new Grid();
    }
}
