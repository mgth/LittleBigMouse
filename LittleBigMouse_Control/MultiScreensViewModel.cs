using System;
using System.Collections;
using System.Collections.Generic;
using System.Collections.ObjectModel;
using System.Linq;
using System.Reflection;
using System.Text;
using System.Threading.Tasks;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Media;
using LbmScreenConfig;
using NotifyChange;

namespace LittleBigMouse_Control
{
    class MultiScreensViewModel : PresenterViewModel
    {
        public override Type ViewType => typeof(MultiScreensView);
        public MultiScreensViewModel()
        {
            ScreensCanvas.SizeChanged += (sender, args) => RaiseProperty("Size");
            ScreenFrames.CollectionChanged += ScreenFrames_CollectionChanged;
        }

        private void ScreenFrames_CollectionChanged(object sender, System.Collections.Specialized.NotifyCollectionChangedEventArgs e)
        {
            if (e.OldItems != null)
                foreach (ScreenFrameViewModel frame in e.OldItems)
                {
                    ScreensCanvas.Children.Remove(frame.View);
                }

            if (e.NewItems != null)
                foreach (ScreenFrameViewModel frame in e.NewItems)
                {
                    ScreensCanvas.Children.Add(frame.View);
                }
        }

        public ScreenConfig Config
        {
            get { return (ScreenConfig)GetValue(ConfigProperty); }
            set { SetValue(ConfigProperty, value); }
        }

        public Canvas ScreensCanvas { get; } = new Canvas();



        public ObservableCollection<ScreenFrameViewModel> ScreenFrames = new ObservableCollection<ScreenFrameViewModel>();

        [DependsOn("Config.Selected")]
        private void BringSelectedToFront()
        {
            ScreenFrameViewModel frame = ScreenFrames.FirstOrDefault(vm => vm.Screen.Selected);
            if (frame != null)
            {
                ScreensCanvas.Children.Remove(frame.View);
                ScreensCanvas.Children.Add(frame.View);               
            }
        }


        [DependsOn(nameof(Config))]
        private void UpdateScreensControlViews()
        {
            IList<ScreenFrameViewModel> old = ScreenFrames.ToList();

            foreach (Screen s in Config.AllScreens)
            {
                ScreenFrameViewModel vm = old.FirstOrDefault(v => v.Screen.Equals(s) );

                if (vm != null) { old.Remove(vm); }
                else
                {
                    vm = new ScreenFrameViewModel
                    {
                        Screen = s,
                        Presenter = this
                    };
                    ScreenFrames.Add(vm);
                }
            }

            foreach (ScreenFrameViewModel frame in old)
            {
                ScreenFrames.Remove(frame);
            }
        }
        public static DependencyProperty ConfigProperty = DependencyProperty.Register(nameof(Config), typeof(ScreenConfig), typeof(MultiScreensViewModel), WatchNotifier());


        [DependsOn("Size", "Config.MovingPhysicalOutsideBounds")]
        private void UpdateRatio(string s)
        {
            Rect all = Config.MovingPhysicalOutsideBounds;

            double ratio = 0;

            if (all.Width * all.Height > 0)
            {
                ratio = Math.Min(
                    ScreensCanvas.ActualWidth / all.Width,
                    ScreensCanvas.ActualHeight / all.Height
                    );
            }
            Ratio = ratio;
        }
        public override double PhysicalToUiX(double x) 
            => (x - Config.MovingPhysicalOutsideBounds.Left) * Ratio 
            + (ScreensCanvas.ActualWidth 
            - Config.MovingPhysicalOutsideBounds.Width * Ratio) / 2;
        public override double PhysicalToUiY(double y) 
            => (y - Config.MovingPhysicalOutsideBounds.Top) * Ratio 
            + (ScreensCanvas.ActualHeight 
            - Config.MovingPhysicalOutsideBounds.Height * Ratio) / 2;

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
    }
}
