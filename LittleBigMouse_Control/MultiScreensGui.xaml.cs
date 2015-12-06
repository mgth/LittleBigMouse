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
using System.Windows.Media.Imaging;
using System.Windows.Navigation;
using System.Windows.Shapes;
using LbmScreenConfig;

namespace LittleBigMouse_Control
{
    /// <summary>
    /// Logique d'interaction pour MultiScreensGui.xaml
    /// </summary>
    public partial class MultiScreensGui : ScreensPresenter
    {
        public MultiScreensGui()
        {
            InitializeComponent();
            SizeChanged += delegate { Change.RaiseProperty("Size"); };

            Config.UpdatePhysicalOutsideBounds();
            UpdateScreensGui();
        }


        public IEnumerable<ScreenGui> AllScreenGuis
            => ScreensGrid.Children.Cast<UIElement>().OfType<ScreenGui>();

        public ScreenGui GetScreenGui(Screen s)
        {
            return AllScreenGuis.FirstOrDefault(gui => gui.Screen == s);
        }


        [DependsOn("Config")]
        public void UpdateScreensGui()
        {
            foreach (ScreenGui sgui in Config.AllScreens.Select(s => new ScreenGui(s, this)))
            {
                ScreensGrid.Children.Add(sgui);
            }

        }

        private double _ratio = 0;
        public double Ratio => _ratio;

        [DependsOn("Size", "Screen.Moving", "Screen.MovingPhysicalOutsideBounds")]
        public void UpdateRatio()
        {
            Rect all = Config.MovingPhysicalOutsideBounds;

            double ratio = 0;

            if (all.Width * all.Height > 0)
            {
                ratio = Math.Min(
                    ScreensGrid.ActualWidth / all.Width,
                    ScreensGrid.ActualHeight / all.Height
                    );
            }
            Change.SetProperty(ref _ratio, ratio, "Ratio");
        }

        public double PhysicalToUiX(double x) => (x - Config.MovingPhysicalOutsideBounds.Left) * Ratio + (ScreensGrid.ActualWidth - Config.MovingPhysicalOutsideBounds.Width * Ratio) / 2;
        public double PhysicalToUiY(double y) => (y - Config.MovingPhysicalOutsideBounds.Top) * Ratio + (ScreensGrid.ActualHeight - Config.MovingPhysicalOutsideBounds.Height * Ratio) / 2;

        public Point PhysicalToUi(Point p)
        {
            return new Point(
                PhysicalToUiX(p.X),
                PhysicalToUiY(p.Y)
                );
        }
        public Point UiToPhysical(Point p)
        {
            Rect all = Config.MovingPhysicalOutsideBounds;

            return new Point(
                (p.X / Ratio) + all.Left,
                (p.Y / Ratio) + all.Top
                );
        }
        public Vector UiToPhysical(Vector V)
        {
            return new Vector(
                (V.X / Ratio),
                (V.Y / Ratio)
                );
        }

        public override IEnumerable<ScreenGuiControl> AllControlGuis
        {
            get {
                foreach (ScreenGui gui in AllScreenGuis)
                {
                    yield return gui.ScreenGuiControl;
                }
            }
        }

        private void ScreensPresenter_MouseDown(object sender, MouseButtonEventArgs e)
        {
            MainGui.Instance.OnMouseDown(sender,e);
        }
    }
}
