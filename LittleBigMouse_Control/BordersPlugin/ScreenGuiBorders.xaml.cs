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

namespace LittleBigMouse_Control.BordersPlugin
{
    /// <summary>
    /// Logique d'interaction pour ScreenGuiBorders.xaml
    /// </summary>
    public partial class ScreenGuiBorders : ScreenGuiControl<ScreenGuiBorders>
    {


        public ScreenGuiBorders(Screen screen):base(screen)
        {
            InitializeComponent();

            ScreenGui.SizeChanged += (sender, args) => Change.RaiseProperty("Size");
            SizeChanged += (sender, args) => Change.RaiseProperty("Size");

            Change.Watch(Screen,"Screen");

            Unloaded += OnUnloaded;

            DrawLines();
        }

        private void OnUnloaded(object sender, RoutedEventArgs routedEventArgs)
        {
            ScreenGui.CoverGrid.Children.Clear();
        }


        private ScreenGui ScreenGui => (MainGui.Instance.ScreensPresenter as MultiScreensGui)?.GetScreenGui(Screen);

        [DependsOn("Size")]
        public void DrawLines()
        {
            if (this.Visibility != System.Windows.Visibility.Visible) return;

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

        [DependsOn("Screen.RealPhysicalWidth", "Screen.RealLeftBorder", "Screen.RealRightBorder")]
        public double PhysicalOutsideWidth
        {
            get { return Screen.RealPhysicalWidth + Screen.RealLeftBorder + Screen.RealRightBorder; }
            set
            {
                double offset = (value - PhysicalOutsideWidth)/2;
                Screen.RealLeftBorder += offset;
                Screen.RealRightBorder += offset;
            }
        }

        [DependsOn("Screen.RealPhysicalHeight", "Screen.RealTopBorder", "Screen.RealBottomBorder")]
        public double PhysicalOutsideHeight
        {
            get { return Screen.RealPhysicalHeight + Screen.RealTopBorder + Screen.RealBottomBorder; }
            set
            {
                double offset = value - PhysicalOutsideHeight;
                Screen.RealBottomBorder += offset;
            }
        }
        private void Height_MouseWheel(object sender, MouseWheelEventArgs e)
        {
            double delta = (e.Delta > 0) ? 1 : -1;
            PhysicalOutsideHeight += delta;
        }
        private void Width_MouseWheel(object sender, MouseWheelEventArgs e)
        {
            double delta = (e.Delta > 0) ? 1 : -1;
            PhysicalOutsideWidth += delta;
        }

        private void Bottom_MouseWheel(object sender, MouseWheelEventArgs e)
        {
            double delta = (e.Delta > 0) ? 1 : -1;
            Screen.RealBottomBorder += delta;
        }

        private void Right_MouseWheel(object sender, MouseWheelEventArgs e)
        {
            double delta = (e.Delta > 0) ? 1 : -1;
            Screen.RealRightBorder += delta;
        }

        private void Left_MouseWheel(object sender, MouseWheelEventArgs e)
        {
            double delta = (e.Delta > 0) ? 1 : -1;
            Screen.RealLeftBorder += delta;
        }

        private void Top_MouseWheel(object sender, MouseWheelEventArgs e)
        {
            double delta = (e.Delta > 0) ? 1 : -1;
            Screen.RealTopBorder += delta;
        }
    }
}
