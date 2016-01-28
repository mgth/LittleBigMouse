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
using System.Windows.Media.Effects;
using System.Windows.Media.Imaging;
using System.Windows.Navigation;
using System.Windows.Shapes;
using LbmScreenConfig;
using NotifyChange;

namespace LittleBigMouse_Control.BordersPlugin
{
    /// <summary>
    /// Logique d'interaction pour ScreenGuiBorders.xaml
    /// </summary>
    public partial class ScreenGuiBorders : ScreenGuiControl
    {


        public ScreenGuiBorders(Screen screen):base(screen)
        {
            InitializeComponent();

            DataContext = new ScreenViewModel(screen);



            ScreenGui.SizeChanged += (sender, args) => DrawLines();
            SizeChanged += (sender, args) => DrawLines();

            Unloaded += OnUnloaded;

            Loaded += OnLoaded;

            
        }

        private void OnLoaded(object sender, RoutedEventArgs routedEventArgs)
        {
            DrawLines();
        }

        private void OnUnloaded(object sender, RoutedEventArgs routedEventArgs)
        {
            ScreenGui.CoverGrid.Children.Clear();
        }


        private ScreenGui ScreenGui => (MainGui.Instance.ScreensPresenter as MultiScreensGui)?.GetScreenGui(Screen);

        public void DrawLines()
        {
            if (Visibility != Visibility.Visible) return;
            if (!IsLoaded) return;

            Canvas c = new Canvas();

           ScreenGui.CoverGrid.Children.Clear();

            double x = 3*ActualWidth/4 + ScreenGui.LeftBorder.Value;
            double y = 3*ActualHeight/4 + ScreenGui.TopBorder.Value;
            double h = ScreenGui.ActualHeight;
            double w = ScreenGui.ActualWidth;

            double arrow = 10;

            line(c.Children, x, 0, x, h);
            line(c.Children, x - arrow / 2, arrow, x, 0);
            line(c.Children, x + arrow / 2, arrow, x, 0);
            line(c.Children, x - arrow / 2, h - arrow, x, h);
            line(c.Children, x + arrow / 2, h - arrow, x, h);

            line(c.Children, 0, y, w, y);
            line(c.Children, arrow, y - arrow / 2, 0, y);
            line(c.Children, arrow, y + arrow / 2, 0, y);
            line(c.Children, w - arrow, y - arrow / 2, w, y);
            line(c.Children, w - arrow, y + arrow / 2, w, y);

            c.Effect = new DropShadowEffect
            {
                Color = Colors.DarkBlue,
            };

            ScreenGui.CoverGrid.Children.Add(c);
        }

        private void line(UIElementCollection ui, double x1, double y1, double x2, double y2)
        {

            Line l = new Line
            {
                Stroke = Brushes.CadetBlue,
                StrokeThickness = 2,
                X1 = x1,
                X2 = x2,
                Y1 = y1,
                Y2 = y2,
                IsHitTestVisible = false
            };

            ui.Add(l);
        }

        private ScreenViewModel ViewModel => DataContext as ScreenViewModel;

        private void Height_MouseWheel(object sender, MouseWheelEventArgs e)
        {
            ViewModel.PhysicalOutsideHeight += WheelDelta(e);
        }
        private void Width_MouseWheel(object sender, MouseWheelEventArgs e)
        {
            ViewModel.PhysicalOutsideWidth += WheelDelta(e);
        }

        private void Bottom_MouseWheel(object sender, MouseWheelEventArgs e)
        {
            Screen.RealBottomBorder += WheelDelta(e);
        }

        private void Right_MouseWheel(object sender, MouseWheelEventArgs e)
        {
            Screen.RealRightBorder += WheelDelta(e);
        }

        private void Left_MouseWheel(object sender, MouseWheelEventArgs e)
        {
            Screen.RealLeftBorder += WheelDelta(e);
        }

        private void Top_MouseWheel(object sender, MouseWheelEventArgs e)
        {
            Screen.RealTopBorder += WheelDelta(e);
        }
    }
}
